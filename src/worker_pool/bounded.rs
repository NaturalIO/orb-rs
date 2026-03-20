use super::*;
use crate::prelude::*;
use crossfire::{
    AsyncRx, MAsyncRx, MAsyncTx, MRx, MTx, RecvTimeoutError, Rx, TrySendError, mpmc, mpsc,
};
use std::{sync::Arc, thread, time::Duration};

/// A bounded worker pool that uses a fixed-size channel for message submission.
///
/// Unlike [`WorkerPoolUnbounded`](super::WorkerPoolUnbounded), this pool uses a bounded channel
/// with a fixed capacity. When the channel is full, subsequent submissions will block
/// until space becomes available. This provides backpressure to prevent memory exhaustion
/// when the producer is faster than the consumer.
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
/// use orb::worker_pool::{Worker, WorkerBlocking, WorkerPoolBounded};
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
/// let pool = WorkerPoolBounded::builder(worker, 2)    // min_workers = 2
///     .max_workers(8)                                 // max_workers = 8 (enable auto-scaling)
///     .timeout(Duration::from_secs(5))               // idle timeout = 5s
///     .new_blocking(100);                            // channel capacity = 100
///
/// for i in 0..100 {
///     pool.submit(MyMsg { value: i });
/// }
///
/// // Try to submit without blocking
/// if let Err(msg) = pool.try_submit(MyMsg { value: 101 }) {
///     println!("Queue full, message dropped");
/// }
/// ```
///
/// ## Async Worker
///
/// Use async workers for I/O-bound operations within an async runtime:
///
/// ```rust
/// use orb::worker_pool::{Worker, WorkerAsync, WorkerPoolBounded};
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
/// let pool = WorkerPoolBounded::builder(worker, 2)    // min_workers = 2
///     .max_workers(8)                                 // max_workers = 8 (enable auto-scaling)
///     .timeout(Duration::from_secs(5))               // idle timeout = 5s
///     .new_async::<RT>(100, None);                   // channel capacity = 100, no explicit runtime
///
/// for i in 0..100 {
///     pool.submit(MyMsg { value: i });
/// }
/// # }
/// ```
#[derive(Clone)]
pub struct WorkerPoolBounded<M>
where
    M: Send + Sized + Unpin + 'static,
{
    tx: MTx<mpmc::Array<M>>,
    tx_async: MAsyncTx<mpmc::Array<M>>,
    inner: Arc<WorkerPoolInner>,
    noti_tx: MTx<mpsc::One<()>>,
    auto: bool,
}

impl<W: WorkerAsync> WorkerPoolBuilder<WorkerPoolBounded<W::Msg>, W> {
    /// Creates a new async worker pool.
    ///
    /// # Arguments
    ///
    /// * `bound`: the channel buffer size,
    /// * `rt`: Optional executor handle. If `None`, uses the thread-local runtime, When you call
    ///    from thread context, must provide the AsyncExec handle to spawning worker.
    ///
    /// # Panics
    ///
    /// Panics if `min_workers` is 0 or if `max_workers < min_workers`.
    pub fn new_async<RT>(mut self, bound: usize, rt: Option<RT::Exec>) -> WorkerPoolBounded<W::Msg>
    where
        RT: AsyncRuntime,
    {
        let worker = self.worker.take().expect("worker must been set");
        let inner = self.build();
        let auto = inner.min_workers < inner.max_workers;
        let (tx_async, rx) = mpmc::bounded_async::<W::Msg>(bound);
        let tx = tx_async.clone().into_blocking();
        let (noti_tx, noti_rx) = mpsc::new();
        WorkerPoolInner::init_async::<W, RT, _>(&inner, rt.as_ref(), &worker, &rx);
        if inner.max_workers > inner.min_workers {
            let f = WorkerPoolBounded::<W::Msg>::watcher_async::<W, RT>(
                inner.clone(),
                worker,
                rx,
                noti_rx,
            );
            if let Some(_rt) = rt.as_ref() {
                _rt.spawn_detach(f);
            } else {
                RT::spawn_detach(f);
            }
        }
        WorkerPoolBounded::<W::Msg> { tx, tx_async, inner, noti_tx, auto }
    }
}

impl<W: WorkerBlocking> WorkerPoolBuilder<WorkerPoolBounded<W::Msg>, W> {
    /// Creates a new blocking worker pool.
    ///
    /// this creates OS threads instead of async tasks,
    /// making it suitable for CPU-bound or blocking operations.
    ///
    /// # Arguments
    ///
    /// * `bound`: the channel buffer size
    ///
    /// # Panics
    ///
    /// Panics if `min_workers` is 0 or if `max_workers < min_workers`.
    pub fn new_blocking(mut self, bound: usize) -> WorkerPoolBounded<W::Msg> {
        let worker = self.worker.take().expect("worker must been set");
        let inner = self.build();
        let auto = inner.min_workers < inner.max_workers;
        let (tx, rx) = mpmc::bounded_blocking::<W::Msg>(bound);
        let tx_async = tx.clone().into_async();
        let (noti_tx, noti_rx) = mpsc::new();
        WorkerPoolInner::init_blocking::<W, _>(&inner, &worker, &rx);
        if inner.max_workers > inner.min_workers {
            let _inner = inner.clone();
            thread::spawn(move || {
                WorkerPoolBounded::<W::Msg>::watcher_blocking::<W>(_inner, worker, rx, noti_rx);
            });
        }
        WorkerPoolBounded::<W::Msg> { tx, tx_async, inner, noti_tx, auto }
    }
}

impl<M> WorkerPoolBounded<M>
where
    M: Send + Sized + Unpin + 'static,
{
    /// Creates a builder for configuring a new bounded worker pool.
    #[inline]
    pub fn builder<W>(worker: W, min_workers: usize) -> WorkerPoolBuilder<WorkerPoolBounded<M>, W>
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
    /// This method might block the thread when the queue is full.
    /// If the channel is full and `min_workers < max_workers`, new worker might spawn
    #[inline]
    pub fn submit(&self, msg: M) {
        if self.auto {
            if let Err(TrySendError::Full(_msg)) = self.tx.try_send(msg) {
                let _ = self.noti_tx.try_send(());
                self.tx.send(_msg).expect("send");
            }
        } else {
            self.tx.send(msg).expect("send");
        }
    }

    /// Submits a message asynchronously to the worker pool for processing.
    ///
    /// This method is similar to [`submit`](Self::submit) but for use in async contexts.
    /// If the channel is full and `min_workers < max_workers`, new worker might spawn
    #[inline]
    pub async fn submit_async(&self, msg: M) {
        if self.auto {
            if let Err(TrySendError::Full(msg)) = self.tx.try_send(msg) {
                let _ = self.noti_tx.try_send(());
                self.tx_async.send(msg).await.expect("send");
            }
        } else {
            self.tx_async.send(msg).await.expect("send");
        }
    }

    /// Attempts to submit a message without blocking.
    ///
    /// Returns `Ok(())` if the message was successfully queued,
    /// or `Err(msg)` if the channel is full.
    #[inline]
    pub fn try_submit(&self, msg: M) -> Result<(), M> {
        match self.tx.try_send(msg) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_msg)) => return Err(_msg),
            _ => unreachable!(),
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

    async fn watcher_async<W, RT>(
        inner: Arc<WorkerPoolInner>, worker: W, rx: MAsyncRx<mpmc::Array<M>>,
        noti_rx: AsyncRx<mpsc::One<()>>,
    ) where
        W: WorkerAsync<Msg = M>,
        RT: AsyncRuntime,
    {
        loop {
            match noti_rx.recv_with_timer(RT::sleep(inner.timeout)).await {
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(_) => {
                    if inner.worker_count() < inner.max_workers {
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
            }
        }
    }

    fn watcher_blocking<W>(
        inner: Arc<WorkerPoolInner>, worker: W, rx: MRx<mpmc::Array<M>>, noti_rx: Rx<mpsc::One<()>>,
    ) where
        W: WorkerBlocking<Msg = M>,
    {
        loop {
            match noti_rx.recv_timeout(Duration::from_millis(500)) {
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(_) => {
                    if inner.worker_count() < inner.max_workers {
                        inner.clone().spawn_blocking_worker::<W, _>(
                            worker.clone(),
                            true,
                            rx.clone(),
                        );
                        // don't spawn too quick
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        }
    }
}
