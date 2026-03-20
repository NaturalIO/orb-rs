//pub mod bounded;
pub mod unbounded;

use crate::prelude::*;
pub use unbounded::WorkerPoolUnbounded;

use crossfire::{AsyncRxTrait, BlockingRxTrait};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

const ZERO_DUR: Duration = Duration::from_secs(0);

pub trait WorkerAsync: Send + Sync + Unpin + 'static + Clone {
    type Msg: Send + Sized + Unpin + 'static;

    #[inline]
    fn init(&self, _count: usize) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn run(&self, msg: Self::Msg) -> impl Future<Output = ()> + Send;

    fn on_exit(&self, _count: usize) {}
}

pub trait WorkerBlocking: Send + Sync + 'static + Clone {
    type Msg: Send + Sized + Unpin + 'static;

    #[inline]
    fn init(&self, _count: usize) {}

    fn run(&self, msg: Self::Msg);

    fn on_exit(&self, _count: usize) {}
}

struct WorkerPoolInner {
    count: AtomicUsize,
    min_workers: usize,
    max_workers: usize,
    timeout: Duration,
}

impl WorkerPoolInner {
    #[inline(always)]
    fn new(min_workers: usize, max_workers: usize, mut timeout: Duration) -> Arc<Self> {
        assert!(min_workers > 0);
        assert!(max_workers >= min_workers);
        if timeout == ZERO_DUR {
            timeout = Duration::from_secs(2);
        }

        Arc::new(Self { count: AtomicUsize::new(0), min_workers, max_workers, timeout })
    }

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
        worker.on_exit(self.dec());
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
        std::thread::spawn(move || {
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
