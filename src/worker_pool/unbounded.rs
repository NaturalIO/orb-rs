use std::{thread, time::Duration};

use super::*;
use crate::prelude::*;
use crossfire::{AsyncRx, MAsyncRx, MRx, MTx, RecvTimeoutError, Rx, mpmc, mpsc, null::CloseHandle};

/// A worker pool with an unbounded channel for message submission.
///
/// Unlike [`WorkerPoolBounded`](super::WorkerPoolBounded), this pool uses an unbounded channel
/// that can grow without limit. Messages are never rejected and submissions never block.
/// This is useful when you want to accept all incoming work and let the pool handle
/// scaling based on queue length.
///
/// **Note**: Be careful with memory usage. If the producer is much faster than consumers,
/// the queue can grow indefinitely. Use [`WorkerPoolBounded`](super::WorkerPoolBounded) if you
/// need backpressure.
///
/// This pool supports both async and blocking workers. It maintains a minimum number of
/// workers and can scale up to a maximum when needed. Workers that are idle for the
/// specified timeout will automatically exit until only the minimum remains.
///
/// # Type Parameters
///
/// * `M` - The message type that workers will process. Must be `Send + Sized + Unpin + 'static`.
///
/// # Examples
///
/// ## Blocking Worker
///
/// Use blocking workers for CPU-bound or blocking operations:
///
/// ```rust
/// use orb::worker_pool::{Worker, WorkerBlocking, WorkerPoolUnbounded};
/// use std::time::Duration;
///
/// #[derive(Clone)]
/// struct MyBlockingWorker;
///
/// #[derive(Clone)]
/// struct MyMsg {
///     value: u32,
/// }
///
/// impl Worker for MyBlockingWorker {
///     type Msg = MyMsg;
/// }
///
/// impl WorkerBlocking for MyBlockingWorker {
///     fn run(&self, msg: Self::Msg) {
///         println!("Processing: {}", msg.value);
///     }
/// }
///
/// let worker = MyBlockingWorker;
/// let pool = WorkerPoolUnbounded::builder(worker, 2)  // min_workers = 2
///     .max_workers(8)                                 // max_workers = 8 (enable auto-scaling)
///     .timeout(Duration::from_secs(5))               // idle timeout = 5s
///     .new_blocking();                               // no channel capacity limit for unbounded
///
/// for i in 0..100 {
///     pool.submit(MyMsg { value: i });
/// }
/// ```
///
/// ## Async Worker
///
/// Use async workers for I/O-bound operations within an async runtime:
///
/// ```rust
/// use orb::worker_pool::{Worker, WorkerAsync, WorkerPoolUnbounded};
/// use orb::AsyncRuntime;
/// use std::time::Duration;
///
/// #[derive(Clone)]
/// struct MyAsyncWorker;
///
/// #[derive(Clone)]
/// struct MyMsg {
///     value: u32,
/// }
///
/// impl Worker for MyAsyncWorker {
///     type Msg = MyMsg;
/// }
///
/// impl WorkerAsync for MyAsyncWorker {
///     async fn run(&self, msg: Self::Msg) {
///         println!("Processing: {}", msg.value);
///     }
/// }
///
/// # fn example<RT: AsyncRuntime>() {
/// let worker = MyAsyncWorker;
/// let pool = WorkerPoolUnbounded::builder(worker, 2)  // min_workers = 2
///     .max_workers(8)                                 // max_workers = 8 (enable auto-scaling)
///     .timeout(Duration::from_secs(5))               // idle timeout = 5s
///     .new_async::<RT>(None);                        // no explicit runtime, no channel capacity limit
///
/// for i in 0..100 {
///     pool.submit(MyMsg { value: i });
/// }
/// # }
/// ```
#[derive(Clone)]
pub struct WorkerPoolUnbounded<M>
where
    M: Send + Sized + Unpin + 'static,
{
    tx: MTx<mpmc::List<M>>,
    inner: Arc<WorkerPoolInner>,
    _close_h: CloseHandle<mpsc::Null>,
}

impl<W: WorkerAsync> WorkerPoolBuilder<WorkerPoolUnbounded<W::Msg>, W> {
    /// Creates a new async worker pool.
    ///
    /// # Arguments
    ///
    /// * `rt` - Optional executor handle. If `None`, uses the thread-local runtime. When you call
    ///    from thread context, must provide the AsyncExec handle to spawning worker.
    pub fn new_async<RT>(mut self, rt: Option<RT::Exec>) -> WorkerPoolUnbounded<W::Msg>
    where
        RT: AsyncRuntime,
    {
        let worker = self.worker.take().expect("worker must been set");
        let inner = self.build();
        let (tx, rx) = mpmc::unbounded_async::<W::Msg>();
        let (close_h, close_rx) = mpsc::new();
        WorkerPoolInner::init_async::<W, RT, _>(&inner, rt.as_ref(), &worker, &rx);
        if inner.max_workers > inner.min_workers {
            let f = WorkerPoolUnbounded::<W::Msg>::watcher_async::<W, RT>(
                inner.clone(),
                worker,
                rx,
                close_rx,
            );
            if let Some(_rt) = rt.as_ref() {
                _rt.spawn_detach(f);
            } else {
                RT::spawn_detach(f);
            }
        }
        WorkerPoolUnbounded::<W::Msg> { tx, inner, _close_h: close_h }
    }
}

impl<W: WorkerBlocking> WorkerPoolBuilder<WorkerPoolUnbounded<W::Msg>, W> {
    ///
    /// # Arguments
    ///
    ///   If queue size > worker_count, will spawn new worker.
    ///
    /// # Type Parameters
    ///
    /// * `W` - The worker type implementing [`WorkerAsync`].
    /// * `RT` - The async runtime type implementing [`AsyncRuntime`].
    ///
    /// # Panics
    ///
    /// Panics if `min_workers` is 0 or if `max_workers < min_workers`.
    /// Creates a new blocking worker pool.
    ///
    /// Unlike [`new_async`](Self::new_async), this creates OS threads instead of async tasks,
    /// making it suitable for CPU-bound or blocking operations.
    ///
    /// # Type Parameters
    ///
    /// * `W` - The worker type implementing [`WorkerBlocking`].
    ///
    /// # Panics
    ///
    /// Panics if `min_workers` is 0 or if `max_workers < min_workers`.
    pub fn new_blocking(mut self) -> WorkerPoolUnbounded<W::Msg> {
        let worker = self.worker.take().expect("worker must been set");
        let inner = self.build();
        let (tx, rx) = mpmc::unbounded_blocking::<W::Msg>();
        let (close_h, close_rx) = mpsc::new();
        WorkerPoolInner::init_blocking::<W, _>(&inner, &worker, &rx);
        if inner.max_workers > inner.min_workers {
            let _inner = inner.clone();
            thread::spawn(move || {
                WorkerPoolUnbounded::<W::Msg>::watcher_blocking::<W>(_inner, worker, rx, close_rx);
            });
        }
        WorkerPoolUnbounded::<W::Msg> { tx, inner, _close_h: close_h }
    }
}

impl<M> WorkerPoolUnbounded<M>
where
    M: Send + Sized + Unpin + 'static,
{
    /// Creates a builder for configuring a new unbounded worker pool.
    #[inline]
    pub fn builder<W>(worker: W, min_workers: usize) -> WorkerPoolBuilder<WorkerPoolUnbounded<M>, W>
    where
        W: Worker<Msg = M>,
    {
        WorkerPoolBuilder {
            worker: Some(worker),
            min_workers,
            max_workers: min_workers,
            timeout: ZERO_DUR,
            spawn_inv: ZERO_DUR,
            _phan: Default::default(),
        }
    }

    /// Submits a message to the worker pool for processing.
    ///
    /// This method is non-blocking and will queue the message for the next available worker.
    #[inline(always)]
    pub fn submit(&self, msg: M) {
        self.tx.send(msg).expect("send");
    }

    async fn watcher_async<W, RT>(
        inner: Arc<WorkerPoolInner>, worker: W, rx: MAsyncRx<mpmc::List<M>>,
        close_rx: AsyncRx<mpsc::Null>,
    ) where
        W: WorkerAsync<Msg = M>,
        RT: AsyncRuntime,
    {
        let mut inv = inner.timeout;
        if inv > Duration::from_secs(1) {
            inv = Duration::from_secs(1);
        }
        loop {
            match close_rx.recv_with_timer(RT::sleep(inv)).await {
                Err(RecvTimeoutError::Timeout) => {
                    let worker_count = inner.worker_count();
                    if worker_count > inner.max_workers {
                        continue;
                    }
                    if rx.len() > worker_count {
                        inner.clone().spawn_async_worker::<W, RT, _>(
                            None,
                            worker.clone(),
                            true,
                            rx.clone(),
                        );
                        // don't spawn too quick
                        RT::sleep(inner.spawn_inv).await;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return;
                }
                Ok(_) => unreachable!(),
            }
        }
    }

    fn watcher_blocking<W>(
        inner: Arc<WorkerPoolInner>, worker: W, rx: MRx<mpmc::List<M>>, close_rx: Rx<mpsc::Null>,
    ) where
        W: WorkerBlocking<Msg = M>,
    {
        let mut inv = inner.timeout;
        if inv > Duration::from_secs(1) {
            inv = Duration::from_secs(1);
        }
        loop {
            match close_rx.recv_timeout(inv) {
                Err(RecvTimeoutError::Timeout) => {
                    let worker_count = inner.worker_count();
                    if worker_count > inner.max_workers {
                        continue;
                    }
                    if rx.len() > worker_count {
                        inner.clone().spawn_blocking_worker::<W, _>(
                            worker.clone(),
                            true,
                            rx.clone(),
                        );
                        // don't spawn too quick
                        thread::sleep(inner.spawn_inv);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return;
                }
                Ok(_) => unreachable!(),
            }
        }
    }

    /// Returns the current number of active workers.
    ///
    /// This count includes both minimum workers and any dynamically spawned workers
    /// that are still active (not yet timed out).
    #[inline]
    pub fn worker_count(&self) -> usize {
        self.inner.worker_count()
    }
}
