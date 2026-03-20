//! Worker pool implementations for concurrent message processing.
//!
//! This module provides two types of worker pools:
//!
//! - [`WorkerPoolUnbounded`]: Uses an unbounded channel for message submission.
//!   Best for scenarios where you want to submit messages without blocking and
//!   let the pool handle scaling based on queue length.
//!
//! - [`WorkerPoolBounded`]: Uses a bounded channel with a fixed capacity.
//!   Best for scenarios where you want backpressure - submissions will block
//!   when the queue is full, preventing memory exhaustion.
//!
//! # Common Features
//!
//! Both pool types share these features:
//!
//! * Use provide their [Worker] object as template, new spawn Worker is produced via Cloned.
//!
//! * **Async and Blocking Workers**: Support for both [WorkerAsync] and
//!   [WorkerBlocking] implementations.
//!
//! * **Dynamic Scaling**: When `max_workers > min_workers`, the pool automatically
//!   scales up workers based on workload and scales down idle workers after a timeout.
//!
//! * **Builder Pattern**: Use [`WorkerPoolBuilder`] to configure pool parameters
//!   like worker counts, timeouts, and spawn intervals.
//!
//! * **init and exit hook**: you may custom actions hook when worker being spawn or exit.
//!
//! # Quick Start
//!
//! ## Using Builder (Recommended)
//!
//! ```rust
//! use orb::worker_pool::{Worker, WorkerBlocking, WorkerPoolBounded};
//! use std::time::Duration;
//!
//! #[derive(Clone)]
//! struct MyWorker;
//!
//! #[derive(Clone)]
//! struct MyMsg { value: u32 }
//!
//! impl Worker for MyWorker {
//!     type Msg = MyMsg;
//! }
//!
//! impl WorkerBlocking for MyWorker {
//!     fn run(&self, msg: Self::Msg) {
//!         println!("Processing: {}", msg.value);
//!     }
//! }
//!
//! // Create a bounded pool with 2-8 workers
//! let pool = WorkerPoolBounded::builder(MyWorker, 2)
//!     .max_workers(8)
//!     .timeout(Duration::from_secs(5))
//!     .new_blocking(100); // channel capacity of 100
//!
//! for i in 0..100 {
//!     pool.submit(MyMsg { value: i });
//! }
//! ```
//!
//! ## Worker Traits
//!
//! Implement [`WorkerAsync`] for async/await-based processing:
//!
//! ```rust
//! use orb::worker_pool::{Worker, WorkerAsync};
//!
//! #[derive(Clone)]
//! struct AsyncWorker;
//!
//! #[derive(Clone)]
//! struct Msg { data: String }
//!
//! impl Worker for AsyncWorker {
//!     type Msg = Msg;
//! }
//!
//! impl WorkerAsync for AsyncWorker {
//!     async fn run(&self, msg: Self::Msg) {
//!         // Async processing logic
//!         println!("Processing: {}", msg.data);
//!     }
//! }
//! ```
//!
//! Implement [`WorkerBlocking`] for CPU-bound or blocking operations:
//!
//! ```rust
//! use orb::worker_pool::{Worker, WorkerBlocking};
//!
//! #[derive(Clone)]
//! struct BlockingWorker;
//!
//! #[derive(Clone)]
//! struct Msg { data: Vec<u8> }
//!
//! impl Worker for BlockingWorker {
//!     type Msg = Msg;
//! }
//!
//! impl WorkerBlocking for BlockingWorker {
//!     fn run(&self, msg: Self::Msg) {
//!         // Blocking processing logic
//!         println!("Processing {} bytes", msg.data.len());
//!     }
//! }
//! ```

mod bounded;
mod unbounded;
use crate::prelude::*;
use std::marker::PhantomData;

pub use bounded::WorkerPoolBounded;
pub use unbounded::WorkerPoolUnbounded;

use crossfire::{AsyncRxTrait, BlockingRxTrait};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

const ZERO_DUR: Duration = Duration::from_secs(0);

pub trait Worker: Send + Sync + Unpin + 'static + Clone {
    type Msg: Send + Sized + Unpin + 'static;
}

pub trait WorkerAsync: Worker {
    #[inline]
    fn init(&self, _count: usize) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn run(&self, msg: Self::Msg) -> impl Future<Output = ()> + Send;

    fn on_exit(&self, _count: usize) -> impl Future<Output = ()> + Send {
        async {}
    }
}

pub trait WorkerBlocking: Worker {
    #[inline]
    fn init(&self, _count: usize) {}

    fn run(&self, msg: Self::Msg);

    fn on_exit(&self, _count: usize) {}
}

/// A builder for configuring and creating worker pools.
///
/// This builder provides a fluent API for configuring worker pool parameters
/// before creating the actual pool. Use [`WorkerPoolBounded::builder()`] or
/// [`WorkerPoolUnbounded::builder()`] to obtain a builder instance.
///
/// When you have done setting, call [`Self::new_async()`] or [`Self::new_blocking()`] to create the
/// worker pool.
///
/// # Configuration Options
///
/// * `min_workers` - The minimum number of workers to maintain (required, set in `builder()`)
/// * `max_workers` - The maximum number of workers allowed (default: same as min_workers)
/// * `timeout` - Idle timeout for dynamic workers (default: 2 seconds)
/// * `spawn_interval` - Minimum interval between spawning new workers (default: 500ms)
pub struct WorkerPoolBuilder<P, W: Worker> {
    min_workers: usize,
    max_workers: usize,
    timeout: Duration,
    spawn_inv: Duration,
    worker: Option<W>,
    _phan: PhantomData<fn(&P)>,
}

impl<P, W: Worker> WorkerPoolBuilder<P, W> {
    /// Sets the maximum number of workers.
    ///
    /// When `max_workers > min_workers`, the pool will automatically scale up
    /// workers based on workload and scale down idle workers after the timeout.
    ///
    /// # Default
    ///
    /// Defaults to `min_workers` (no scaling).
    ///
    /// # Panics
    ///
    /// Panics if `max_workers < min_workers` when building the pool.
    #[inline]
    pub fn max_workers(mut self, max_workers: usize) -> Self {
        self.max_workers = max_workers;
        self
    }

    /// Sets the idle timeout for dynamic workers.
    ///
    /// Timeout has two meanings:
    /// - When worker count > min_workers, extra workers will exit after being idle for this duration.
    /// - The watcher thread (coroutine) will check conditions to spawn workers every `timeout` duration.
    ///
    /// # Default
    ///
    /// Defaults to 2 seconds.
    #[inline]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the minimum interval between spawning new workers.
    ///
    /// This prevents workers from being spawned too quickly during
    /// sudden load spikes.
    ///
    /// # Default
    ///
    /// Defaults to 500 milliseconds.
    #[inline]
    pub fn spawn_interval(mut self, inv: Duration) -> Self {
        self.spawn_inv = inv;
        self
    }

    #[inline]
    fn build(mut self) -> Arc<WorkerPoolInner> {
        assert!(self.min_workers > 0);
        assert!(self.max_workers >= self.min_workers);
        if self.timeout == ZERO_DUR {
            self.timeout = Duration::from_secs(2);
        }
        if self.spawn_inv == ZERO_DUR {
            self.spawn_inv = Duration::from_millis(500);
        }
        Arc::new(WorkerPoolInner {
            count: AtomicUsize::new(0),
            min_workers: self.min_workers,
            max_workers: self.max_workers,
            timeout: self.timeout,
            spawn_inv: self.spawn_inv,
            // TODO bind_cpu param
            max_cpu: 0,
            bind_cpu: AtomicUsize::new(0),
        })
    }
}

struct WorkerPoolInner {
    count: AtomicUsize,
    min_workers: usize,
    max_workers: usize,
    timeout: Duration,
    spawn_inv: Duration,
    max_cpu: usize,
    bind_cpu: AtomicUsize,
}

impl WorkerPoolInner {
    #[inline(always)]
    fn inc(&self) -> usize {
        self.count.fetch_add(1, Ordering::Acquire) + 1
    }

    #[inline(always)]
    fn dec(&self) -> usize {
        self.count.fetch_sub(1, Ordering::Release) - 1
    }

    #[inline(always)]
    fn worker_count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    fn init_async<W, RT, RX>(this: &Arc<Self>, rt: Option<&RT::Exec>, worker: &W, rx: &RX)
    where
        W: WorkerAsync,
        RT: AsyncRuntime,
        RX: AsyncRxTrait<W::Msg> + Clone,
    {
        for _ in 0..this.min_workers {
            this.clone().spawn_async_worker::<W, RT, RX>(rt, worker.clone(), false, rx.clone());
        }
    }

    #[inline]
    fn spawn_async_worker<W, RT, RX>(
        self: Arc<Self>, rt: Option<&RT::Exec>, worker: W, auto: bool, rx: RX,
    ) where
        W: WorkerAsync,
        RT: AsyncRuntime,
        RX: AsyncRxTrait<W::Msg> + Clone,
    {
        let f = self.run_async_worker::<W, RT, RX>(worker, auto, rx);
        if let Some(_rt) = rt.as_ref() {
            _rt.spawn_detach(f);
        } else {
            RT::spawn_detach(f);
        }
    }

    async fn run_async_worker<W, RT, RX>(self: Arc<Self>, worker: W, auto: bool, rx: RX)
    where
        W: WorkerAsync,
        RT: AsyncRuntime,
        RX: AsyncRxTrait<W::Msg>,
    {
        worker.init(self.inc()).await;
        if auto {
            while let Ok(msg) = rx.recv_with_timer(RT::sleep(self.timeout)).await {
                worker.run(msg).await;
                while let Ok(msg) = rx.try_recv() {
                    worker.run(msg).await;
                }
            }
        } else {
            while let Ok(msg) = rx.recv().await {
                worker.run(msg).await;
                while let Ok(msg) = rx.try_recv() {
                    worker.run(msg).await;
                }
            }
        }
        worker.on_exit(self.dec()).await;
    }

    fn init_blocking<W, RX>(this: &Arc<Self>, worker: &W, rx: &RX)
    where
        W: WorkerBlocking,
        RX: BlockingRxTrait<W::Msg> + Clone,
    {
        for _ in 0..this.min_workers {
            this.clone().spawn_blocking_worker::<W, RX>(worker.clone(), false, rx.clone());
        }
    }

    #[inline(always)]
    fn spawn_blocking_worker<W, RX>(self: Arc<Self>, worker: W, auto: bool, rx: RX)
    where
        W: WorkerBlocking,
        RX: BlockingRxTrait<W::Msg> + Clone,
    {
        let _bind_cpu: Option<usize> = if self.max_cpu > 0 {
            let cpu = self.bind_cpu.fetch_add(1, Ordering::Relaxed);
            Some(cpu % self.max_cpu)
        } else {
            None
        };
        std::thread::spawn(move || {
            //            if let Some(cpu) = bind_cpu {
            //                core_affinity::set_for_current(core_affinity::CoreId { id: cpu });
            //            }
            self.run_blocking_worker(worker, auto, rx);
        });
    }

    fn run_blocking_worker<W, RX>(self: Arc<Self>, worker: W, auto: bool, rx: RX)
    where
        W: WorkerBlocking,
        RX: BlockingRxTrait<W::Msg> + Clone,
    {
        worker.init(self.inc());
        if auto {
            while let Ok(msg) = rx.recv_timeout(self.timeout) {
                worker.run(msg);
                while let Ok(msg) = rx.try_recv() {
                    worker.run(msg);
                }
            }
        } else {
            while let Ok(msg) = rx.recv() {
                worker.run(msg);
                while let Ok(msg) = rx.try_recv() {
                    worker.run(msg);
                }
            }
        }
        worker.on_exit(self.dec());
    }
}
