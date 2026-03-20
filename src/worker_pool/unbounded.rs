use std::{thread, time::Duration};

use super::*;
use crate::prelude::*;
use crossfire::{AsyncRx, MAsyncRx, MRx, MTx, RecvTimeoutError, Rx, mpmc, mpsc, null::CloseHandle};

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
    #[inline(always)]
    pub fn submit(&self, msg: M) {
        self.tx.send(msg).expect("send");
    }

    pub fn new_async<W, RT>(
        worker: W, rt: Option<RT::Exec>, min_workers: usize, max_workers: usize, timeout: Duration,
    ) -> Self
    where
        W: WorkerAsync<Msg = M>,
        RT: AsyncRuntime,
    {
        let inner = WorkerPoolInner::new(min_workers, max_workers, timeout);
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

    pub fn new_blocking<W>(
        worker: W, min_workers: usize, max_workers: usize, timeout: Duration,
    ) -> Self
    where
        W: WorkerBlocking<Msg = M>,
    {
        let inner = WorkerPoolInner::new(min_workers, max_workers, timeout);
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
