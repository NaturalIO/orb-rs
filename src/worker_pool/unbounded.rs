use std::{thread, time::Duration};

use super::*;
use crate::prelude::*;
use crossfire::{AsyncRx, MAsyncRx, MRx, MTx, RecvTimeoutError, Rx, mpmc, mpsc, null::CloseHandle};

/// An worker pool submit message with unbounded channel, can dynamically scale workers based on workload.
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
/// use orb::worker_pool::{WorkerBlocking, WorkerPoolUnbounded};
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
/// let pool = WorkerPoolUnbounded::new_blocking(worker, 2, 8, Duration::from_secs(5));
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
/// use orb::worker_pool::{WorkerAsync, WorkerPoolUnbounded};
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
/// let pool = WorkerPoolUnbounded::new_async::<_, RT>(worker, None, 2, 8, Duration::from_secs(5));
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

impl<M> WorkerPoolUnbounded<M>
where
    M: Send + Sized + Unpin + 'static,
{
    /// Submits a message to the worker pool for processing.
    ///
    /// This method is non-blocking and will queue the message for the next available worker.
    #[inline(always)]
    pub fn submit(&self, msg: M) {
        self.tx.send(msg).expect("send");
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
        worker: W, rt: Option<RT::Exec>, min_workers: usize, max_workers: usize, timeout: Duration,
    ) -> Self
    where
        W: WorkerAsync<Msg = M>,
        RT: AsyncRuntime,
    {
        let inner = WorkerPoolInner::new(timeout, min_workers, max_workers);
        let (tx, rx) = mpmc::unbounded_async::<M>();
        let (close_h, close_rx) = mpsc::new();
        WorkerPoolInner::init_async::<W, RT, _>(&inner, rt.as_ref(), &worker, &rx);
        if max_workers > min_workers {
            let f = Self::watcher_async::<W, RT>(inner.clone(), worker, rx, close_rx);
            if let Some(_rt) = rt.as_ref() {
                _rt.spawn_detach(f);
            } else {
                RT::spawn_detach(f);
            }
        }
        Self { tx, inner, _close_h: close_h }
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
        worker: W, min_workers: usize, max_workers: usize, timeout: Duration,
    ) -> Self
    where
        W: WorkerBlocking<Msg = M>,
    {
        let inner = WorkerPoolInner::new(timeout, min_workers, max_workers);
        let (tx, rx) = mpmc::unbounded_blocking::<M>();
        let (close_h, close_rx) = mpsc::new();
        WorkerPoolInner::init_blocking::<W, _>(&inner, &worker, &rx);
        if max_workers > min_workers {
            let _inner = inner.clone();
            thread::spawn(move || {
                Self::watcher_blocking::<W>(_inner, worker, rx, close_rx);
            });
        }
        Self { tx, inner, _close_h: close_h }
    }

    async fn watcher_async<W, RT>(
        inner: Arc<WorkerPoolInner>, worker: W, rx: MAsyncRx<mpmc::List<M>>,
        close_rx: AsyncRx<mpsc::Null>,
    ) where
        W: WorkerAsync<Msg = M>,
        RT: AsyncRuntime,
    {
        loop {
            match close_rx.recv_with_timer(RT::sleep(Duration::from_secs(1))).await {
                Err(RecvTimeoutError::Timeout) => {
                    let worker_count = inner.worker_count();
                    if worker_count > inner.max_workers {
                        continue;
                    }
                    let mut pending_msg = rx.len();
                    if pending_msg > worker_count {
                        pending_msg -= worker_count;
                        if pending_msg > inner.max_workers - worker_count {
                            pending_msg = inner.max_workers - worker_count;
                        }
                        for _ in 0..pending_msg {
                            inner.clone().spawn_async_worker::<W, RT, _>(
                                None,
                                worker.clone(),
                                true,
                                rx.clone(),
                            );
                        }
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
        loop {
            match close_rx.recv_timeout(Duration::from_secs(1)) {
                Err(RecvTimeoutError::Timeout) => {
                    let worker_count = inner.worker_count();
                    if worker_count > inner.max_workers {
                        continue;
                    }
                    let mut pending_msg = rx.len();
                    if pending_msg > worker_count {
                        pending_msg -= worker_count;
                        if pending_msg > inner.max_workers - worker_count {
                            pending_msg = inner.max_workers - worker_count;
                        }
                        for _ in 0..pending_msg {
                            inner.clone().run_blocking_worker::<W, _>(
                                worker.clone(),
                                true,
                                rx.clone(),
                            );
                        }
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
