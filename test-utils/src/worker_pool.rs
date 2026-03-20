use captains_log::logfn;
use crossfire::waitgroup::{WaitGroup, WaitGroupGuard};
use orb::prelude::*;
use orb::worker_pool::{Worker, WorkerAsync, WorkerPoolBounded, WorkerPoolUnbounded};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Test message for async worker pool tests
#[derive(Clone)]
struct AsyncTestMsg {
    processed: Arc<AtomicUsize>,
    _guard: WaitGroupGuard<()>,
}

/// Async worker implementation for testing
struct TestAsyncWorker<RT: AsyncRuntime> {
    _phantom: std::marker::PhantomData<fn(&RT)>,
}

impl<RT: AsyncRuntime> Clone for TestAsyncWorker<RT> {
    fn clone(&self) -> Self {
        Self { _phantom: std::marker::PhantomData }
    }
}

impl<RT: AsyncRuntime> Worker for TestAsyncWorker<RT> {
    type Msg = AsyncTestMsg;
}

impl<RT: AsyncRuntime> WorkerAsync for TestAsyncWorker<RT> {
    async fn run(&self, msg: Self::Msg) {
        RT::sleep(Duration::from_millis(1)).await;
        msg.processed.fetch_add(1, Ordering::SeqCst);
    }
}

/// Slow async worker for timeout testing
struct SlowAsyncWorker<RT: AsyncRuntime> {
    _phantom: std::marker::PhantomData<fn(&RT)>,
}

impl<RT: AsyncRuntime> Clone for SlowAsyncWorker<RT> {
    fn clone(&self) -> Self {
        Self { _phantom: std::marker::PhantomData }
    }
}

impl<RT: AsyncRuntime> Worker for SlowAsyncWorker<RT> {
    type Msg = AsyncTestMsg;
}

impl<RT: AsyncRuntime> WorkerAsync for SlowAsyncWorker<RT> {
    async fn run(&self, msg: Self::Msg) {
        // Sleep longer to simulate slow processing
        RT::sleep(Duration::from_millis(50)).await;
        msg.processed.fetch_add(1, Ordering::SeqCst);
    }
}

/// Test unbounded async worker pool with fixed workers (no scaling)
#[logfn]
pub fn test_unbounded_async_worker_pool_basic<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        let processed_count = Arc::new(AtomicUsize::new(0));
        let workers = 4;
        let worker_timeout = Duration::from_secs(1);

        let worker = TestAsyncWorker::<RT> { _phantom: std::marker::PhantomData };

        let pool = WorkerPoolUnbounded::builder(worker, workers)
            .max_workers(workers)
            .timeout(worker_timeout)
            .new_async::<RT>(None);

        // Verify initial worker count
        RT::sleep(Duration::from_millis(50)).await;
        let initial_count = pool.worker_count();
        assert_eq!(initial_count, workers, "Should have exactly {} workers", workers);

        // Submit messages with WaitGroup for synchronization
        let msg_count = 100;
        let wg = WaitGroup::new((), 0);

        for _ in 0..msg_count {
            let _guard = wg.add_guard();
            pool.submit(AsyncTestMsg { processed: processed_count.clone(), _guard });
        }

        // Wait for all messages to be processed
        wg.wait_async().await;

        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            msg_count,
            "All messages should be processed"
        );
        assert_eq!(pool.worker_count(), workers, "Worker count should remain constant");

        log::info!("Async worker pool basic test passed: {} messages processed", msg_count);
    });
}

/// Test async worker pool with timeout - workers should exit after timeout
#[logfn]
pub fn test_unbounded_async_worker_pool_timeout<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        let processed_count = Arc::new(AtomicUsize::new(0));
        let min_workers = 1;
        let max_workers = 4;
        // Use a longer timeout so workers don't exit during the test
        let worker_timeout = Duration::from_millis(500);

        let worker = SlowAsyncWorker::<RT> { _phantom: std::marker::PhantomData };

        let pool = WorkerPoolUnbounded::builder(worker, min_workers)
            .max_workers(max_workers)
            .timeout(worker_timeout)
            .new_async::<RT>(None);

        // Keep submitting messages until workers scale up beyond min_workers
        let wg = WaitGroup::new((), 0);
        let mut msg_count = 0;
        loop {
            // Submit a batch of messages
            for _ in 0..100 {
                let _guard = wg.add_guard();
                pool.submit(AsyncTestMsg { processed: processed_count.clone(), _guard });
                msg_count += 1;
            }
            println!("submit {msg_count} worker {}", pool.worker_count());
            if pool.worker_count() == max_workers {
                break;
            }
            // Wait a bit for watcher to detect load and spawn workers
            RT::sleep(worker_timeout).await;
        }

        // Wait for all messages to be processed
        wg.wait_async().await;
        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            msg_count,
            "All messages should be processed"
        );
        // Wait for timeout - extra workers should exit

        while pool.worker_count() > min_workers {
            RT::sleep(worker_timeout).await;
        }
        println!("current worker {}", pool.worker_count());
    });
}

/// Test bounded async worker pool with fixed workers (no scaling)
#[logfn]
pub fn test_bounded_async_worker_pool_basic<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        let processed_count = Arc::new(AtomicUsize::new(0));
        let workers = 4;
        let bound = 100;
        let worker_timeout = Duration::from_secs(1);

        let worker = TestAsyncWorker::<RT> { _phantom: std::marker::PhantomData };

        let pool = WorkerPoolBounded::builder(worker, workers)
            .max_workers(workers)
            .timeout(worker_timeout)
            .new_async::<RT>(bound, None);

        RT::sleep(Duration::from_millis(50)).await;
        let initial_count = pool.worker_count();
        assert_eq!(initial_count, workers, "Should have exactly {} workers", workers);

        let wg = WaitGroup::new((), 0);

        for _i in 0..bound {
            let _guard = wg.add_guard();
            assert!(pool
                .try_submit(AsyncTestMsg { processed: processed_count.clone(), _guard })
                .is_ok());
        }
        for _i in 0..bound {
            let _guard = wg.add_guard();
            pool.submit_async(AsyncTestMsg { processed: processed_count.clone(), _guard }).await;
        }
        wg.wait_async().await;
        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            bound * 2,
            "All messages should be processed"
        );
        assert_eq!(pool.worker_count(), workers, "Worker count should remain constant");
    });
}

/// Test async worker pool with timeout - workers should exit after timeout
/// Note: For bounded pools, scaling is triggered when submit times out due to full queue
#[logfn]
pub fn test_bounded_async_worker_pool_timeout<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        let processed_count = Arc::new(AtomicUsize::new(0));
        let min_workers = 1;
        let max_workers = 4;
        // Use a larger bound to avoid blocking during submission
        let bound = 2;
        // Use a longer timeout so workers don't exit during the test
        let worker_timeout = Duration::from_millis(500);

        let worker = SlowAsyncWorker::<RT> { _phantom: std::marker::PhantomData };

        let pool = WorkerPoolBounded::builder(worker, min_workers)
            .max_workers(max_workers)
            .timeout(worker_timeout)
            .new_async::<RT>(bound, None);

        let mut msg_count = 0;
        let wg = WaitGroup::new((), 0);

        while pool.worker_count() < max_workers {
            for _ in 0..bound {
                let _guard = wg.add_guard();
                pool.submit_async(AsyncTestMsg { processed: processed_count.clone(), _guard })
                    .await;
                msg_count += 1;
            }
            println!("submit {msg_count}, {}", pool.worker_count());
        }
        // Wait for all messages to be processed
        wg.wait_async().await;
        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            msg_count,
            "All messages should be processed"
        );
        while pool.worker_count() > min_workers {
            RT::sleep(Duration::from_millis(800)).await;
        }
        let final_count = pool.worker_count();
        assert_eq!(final_count, min_workers, "Workers should exit after timeout");
    });
}
