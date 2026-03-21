use super::*;
use crate::prelude::*;
use crossfire::{
    AsyncRx, MAsyncRx, MAsyncTx, MRx, MTx, RecvTimeoutError, Rx, SendTimeoutError, TrySendError,
    mpmc, mpsc,
};
use std::{sync::Arc, thread, time::Duration};

/// A bounded worker pool that uses a fixed-size channel for message submission.
///
/// Unlike [`WorkerPoolUnbounded`](super::WorkerPoolUnbounded), this pool uses a bounded channel
/// with a fixed capacity. When the channel is full, subsequent submissions will block (for async)
/// or timeout and trigger dynamic scaling (when min_workers < max_workers).
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
/// use orb::worker_pool::{WorkerBlocking, WorkerPoolBounded};
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
/// impl WorkerBlocking for MyBlockingWorker {
///     type Msg = MyMsg;
///
///     fn run(&self, msg: Self::Msg) {
///         println!("Processing: {}", msg.value);
///     }
/// }
///
/// let worker = MyBlockingWorker;
/// // Create a pool with channel bound of 100, 2-8 workers, 5s timeout
/// let pool = WorkerPoolBounded::new_blocking(100, worker, 2, 8, Duration::from_secs(5));
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
/// use orb::worker_pool::{WorkerAsync, WorkerPoolBounded};
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
/// impl WorkerAsync for MyAsyncWorker {
///     type Msg = MyMsg;
///
///     async fn run(&self, msg: Self::Msg) {
///         println!("Processing: {}", msg.value);
///     }
/// }
///
/// # fn example<RT: AsyncRuntime>() {
/// let worker = MyAsyncWorker;
/// // Create a pool with channel bound of 100, 2-8 workers, 5s timeout
/// let pool = WorkerPoolBounded::new_async::<_, RT>(100, worker, None, 2, 8, Duration::from_secs(5));
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

impl<M> WorkerPoolBounded<M>
where
    M: Send + Sized + Unpin + 'static,
{
    /// Submits a message to the worker pool for processing.
    ///
    /// This method will queue the message for the next available worker.
    /// If the channel is full and `min_workers < max_workers`, this method will:
    /// 1. Wait up to 1 second for space to become available
    /// 2. If timed out, notify the watcher to spawn additional workers
    /// 3. Block until the message can be sent
    ///
    /// For non-blocking submission, use [`try_submit`](Self::try_submit).
    /// For async submission, use [`submit_async`](Self::submit_async).
    #[inline]
    pub fn submit(&self, msg: M) {
        if self.auto {
            if let Err(SendTimeoutError::Timeout(_msg)) =
                self.tx.send_timeout(msg, Duration::from_secs(1))
            {
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
    /// If the channel is full and `min_workers < max_workers`, this method will:
    /// 1. Wait up to 1 second for space to become available
    /// 2. If timed out, notify the watcher to spawn additional workers
    /// 3. Await until the message can be sent
    ///
    /// # Type Parameters
    ///
    /// * `RT` - The async runtime type implementing [`AsyncRuntime`].
    #[inline]
    pub async fn submit_async<RT: AsyncRuntime>(&self, msg: M) {
        if self.auto {
            if let Err(TrySendError::Full(msg)) = self.tx.try_send(msg) {
                if let Err(SendTimeoutError::Timeout(msg)) =
                    self.tx_async.send_with_timer(msg, RT::sleep(Duration::from_secs(1))).await
                {
                    let _ = self.noti_tx.try_send(());
                    self.tx_async.send(msg).await.expect("send");
                }
            }
        } else {
            self.tx_async.send(msg).await.expect("send");
        }
    }

    /// Attempts to submit a message without blocking.
    ///
    /// Returns `Ok(())` if the message was successfully queued,
    /// or `Err(msg)` if the channel is full.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orb::worker_pool::{WorkerBlocking, WorkerPoolBounded};
    /// use std::time::Duration;
    ///
    /// # #[derive(Clone)]
    /// # struct MyWorker;
    /// # #[derive(Clone)]
    /// # struct MyMsg;
    /// # impl WorkerBlocking for MyWorker {
    /// #     type Msg = MyMsg;
    /// #     fn run(&self, _msg: Self::Msg) {}
    /// # }
    /// # let worker = MyWorker;
    /// # let pool = WorkerPoolBounded::new_blocking(10, worker, 1, 1, Duration::from_secs(1));
    /// // Try to submit without blocking
    /// match pool.try_submit(MyMsg) {
    ///     Ok(()) => println!("Message queued successfully"),
    ///     Err(_) => println!("Queue full, try again later"),
    /// }
    /// ```
    #[inline]
    pub fn try_submit(&self, msg: M) -> Result<(), M> {
        match self.tx.try_send(msg) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_msg)) => return Err(_msg),
            _ => unreachable!(),
        }
    }

    /// Creates a new async worker pool.
    ///
    /// # Arguments
    ///
    /// * `worker` - The worker implementation that will process messages. Must implement [`WorkerAsync`].
    /// * `rt` - Optional executor handle. If `None`, uses the thread-local runtime via [`AsyncRuntime::spawn_detach`].
    /// * `min_workers` - The minimum number of workers to maintain. Must be greater than 0.
    /// * `max_workers` - The maximum number of workers allowed. Must be >= `min_workers`.
    /// * `timeout` - Idle timeout for dynamic workers. If set to 0 when `max_workers > min_workers`,
    ///   defaults to 2 seconds. Workers idle for this duration will exit.
    ///
    /// # Type Parameters
    ///
    /// * `W` - The worker type implementing [`WorkerAsync`].
    /// * `RT` - The async runtime type implementing [`AsyncRuntime`].
    ///
    /// # Panics
    ///
    /// Panics if `min_workers` is 0 or if `max_workers < min_workers`.
    pub fn new_async<W, RT>(
        bound: usize, worker: W, rt: Option<RT::Exec>, min_workers: usize, max_workers: usize,
        timeout: Duration,
    ) -> Self
    where
        W: WorkerAsync<Msg = M>,
        RT: AsyncRuntime,
    {
        let inner = WorkerPoolInner::new(timeout, min_workers, max_workers);
        let auto = min_workers < max_workers;
        let (tx_async, rx) = mpmc::bounded_async::<M>(bound);
        let tx = tx_async.clone().into_blocking();
        let (noti_tx, noti_rx) = mpsc::new();
        WorkerPoolInner::init_async::<W, RT, _>(&inner, rt.as_ref(), &worker, &rx);
        if max_workers > min_workers {
            let f = Self::watcher_async::<W, RT>(inner.clone(), worker, rx, noti_rx);
            if let Some(_rt) = rt.as_ref() {
                _rt.spawn_detach(f);
            } else {
                RT::spawn_detach(f);
            }
        }
        Self { tx, tx_async, inner, noti_tx, auto }
    }

    /// Creates a new blocking worker pool.
    ///
    /// Unlike [`new_async`](Self::new_async), this creates OS threads instead of async tasks,
    /// making it suitable for CPU-bound or blocking operations.
    ///
    /// # Arguments
    ///
    /// * `worker` - The worker implementation that will process messages. Must implement [`WorkerBlocking`].
    /// * `min_workers` - The minimum number of workers to maintain. Must be greater than 0.
    /// * `max_workers` - The maximum number of workers allowed. Must be >= `min_workers`.
    /// * `timeout` - Idle timeout for dynamic workers. If set to 0 when `max_workers > min_workers`,
    ///   defaults to 2 seconds. Workers idle for this duration will exit.
    ///
    /// # Type Parameters
    ///
    /// * `W` - The worker type implementing [`WorkerBlocking`].
    ///
    /// # Panics
    ///
    /// Panics if `min_workers` is 0 or if `max_workers < min_workers`.
    pub fn new_blocking<W>(
        bound: usize, worker: W, min_workers: usize, max_workers: usize, timeout: Duration,
    ) -> Self
    where
        W: WorkerBlocking<Msg = M>,
    {
        let auto = min_workers < max_workers;
        let inner = WorkerPoolInner::new(timeout, min_workers, max_workers);
        let (tx, rx) = mpmc::bounded_blocking::<M>(bound);
        let tx_async = tx.clone().into_async();
        let (noti_tx, noti_rx) = mpsc::new();
        WorkerPoolInner::init_blocking::<W, _>(&inner, &worker, &rx);
        if max_workers > min_workers {
            let _inner = inner.clone();
            thread::spawn(move || {
                Self::watcher_blocking::<W>(_inner, worker, rx, noti_rx);
            });
        }
        Self { tx, tx_async, inner, noti_tx, auto }
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
            match noti_rx.recv_with_timer(RT::sleep(Duration::from_millis(500))).await {
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(_) => {
                    inner.clone().spawn_async_worker::<W, RT, _>(
                        None,
                        worker.clone(),
                        true,
                        rx.clone(),
                    );
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
                    inner.clone().run_blocking_worker::<W, _>(worker.clone(), true, rx.clone());
                }
            }
        }
    }
}
