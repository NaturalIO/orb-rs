use captains_log::logfn;
use crossfire::waitgroup::{WaitGroup, WaitGroupGuard};
use orb::prelude::*;
use orb::worker_pool::{WorkerAsync, WorkerPoolBounded, WorkerPoolUnbounded};
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

impl<RT: AsyncRuntime> WorkerAsync for TestAsyncWorker<RT> {
    type Msg = AsyncTestMsg;

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

impl<RT: AsyncRuntime> WorkerAsync for SlowAsyncWorker<RT> {
    type Msg = AsyncTestMsg;

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

        let pool =
            WorkerPoolUnbounded::new_async::<_, RT>(worker, None, workers, workers, worker_timeout);

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

        let pool = WorkerPoolUnbounded::new_async::<_, RT>(
            worker,
            None,
            min_workers,
            max_workers,
            worker_timeout,
        );

        // Keep submitting messages until workers scale up beyond min_workers
        let wg = WaitGroup::new((), 0);
        let mut msg_id = 0;
        let max_attempts = 1000;

        while pool.worker_count() <= min_workers && msg_id < max_attempts {
            // Submit a batch of messages
            for _ in 0..10 {
                let _guard = wg.add_guard();
                pool.submit(AsyncTestMsg { processed: processed_count.clone(), _guard });
                msg_id += 1;
            }
            // Wait a bit for watcher to detect load and spawn workers
            RT::sleep(Duration::from_millis(100)).await;
        }

        let scaled_count = pool.worker_count();
        log::info!("Workers scaled up to: {} (submitted {} messages)", scaled_count, msg_id);
        assert!(
            scaled_count > min_workers,
            "Should have scaled up beyond min_workers, got {} workers",
            scaled_count
        );

        // Wait for all messages to be processed
        wg.wait_async().await;
        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            msg_id,
            "All messages should be processed"
        );

        // Wait for timeout - extra workers should exit
        RT::sleep(Duration::from_secs(2)).await;

        let final_count = pool.worker_count();
        log::info!("Final worker count after timeout: {}", final_count);
        assert_eq!(final_count, min_workers, "Extra workers should exit after timeout");
        log::info!(
            "Async timeout test passed: scaled up to {}, then back to {}",
            scaled_count,
            final_count
        );
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

        let pool = WorkerPoolBounded::new_async::<_, RT>(
            bound,
            worker,
            None,
            workers,
            workers,
            worker_timeout,
        );

        RT::sleep(Duration::from_millis(50)).await;
        let initial_count = pool.worker_count();
        assert_eq!(initial_count, workers, "Should have exactly {} workers", workers);

        let msg_count = 100;
        let wg = WaitGroup::new((), 0);

        for _ in 0..msg_count {
            let _guard = wg.add_guard();
            pool.submit(AsyncTestMsg { processed: processed_count.clone(), _guard });
        }

        wg.wait_async().await;

        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            msg_count,
            "All messages should be processed"
        );
        assert_eq!(pool.worker_count(), workers, "Worker count should remain constant");

        log::info!("Bounded async worker pool basic test passed: {} messages processed", msg_count);
    });
}

/// Test async worker pool with timeout - workers should exit after timeout
#[logfn]
pub fn test_bounded_async_worker_pool_timeout<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        let processed_count = Arc::new(AtomicUsize::new(0));
        let min_workers = 2;
        let max_workers = 2; // Fixed workers for bounded test
        let bound = 100;
        let worker_timeout = Duration::from_millis(500);

        let worker = TestAsyncWorker::<RT> { _phantom: std::marker::PhantomData };

        let pool = WorkerPoolBounded::new_async::<_, RT>(
            bound,
            worker,
            None,
            min_workers,
            max_workers,
            worker_timeout,
        );

        // Wait for workers to start
        RT::sleep(Duration::from_millis(50)).await;
        assert_eq!(pool.worker_count(), min_workers, "Should have {} workers", min_workers);

        let wg = WaitGroup::new((), 0);
        let msg_count = 50;

        for _ in 0..msg_count {
            let _guard = wg.add_guard();
            pool.submit(AsyncTestMsg { processed: processed_count.clone(), _guard });
        }

        wg.wait_async().await;
        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            msg_count,
            "All messages should be processed"
        );

        // Verify workers are still alive
        assert_eq!(pool.worker_count(), min_workers, "Workers should remain constant");

        log::info!("Bounded async timeout test passed: {} messages processed", msg_count);
    });
}

/// Test try_submit when queue is full
#[logfn]
pub fn test_bounded_async_worker_pool_try_submit<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        let processed_count = Arc::new(AtomicUsize::new(0));
        let workers = 1;
        let bound = 5;
        let worker_timeout = Duration::from_secs(1);

        // Use fast worker to avoid waiting too long
        let worker = TestAsyncWorker::<RT> { _phantom: std::marker::PhantomData };

        let pool = WorkerPoolBounded::new_async::<_, RT>(
            bound,
            worker,
            None,
            workers,
            workers,
            worker_timeout,
        );

        // Wait for worker to start
        RT::sleep(Duration::from_millis(50)).await;

        // Fill up the queue
        for _ in 0..bound {
            assert!(pool
                .try_submit(AsyncTestMsg {
                    processed: processed_count.clone(),
                    _guard: WaitGroup::new((), 0).add_guard()
                })
                .is_ok());
        }

        // Next submit should fail
        let result = pool.try_submit(AsyncTestMsg {
            processed: processed_count.clone(),
            _guard: WaitGroup::new((), 0).add_guard(),
        });
        assert!(result.is_err(), "Should fail when queue is full");

        // Wait for messages to be processed
        RT::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            bound,
            "All submitted messages should be processed"
        );

        log::info!("Bounded async try_submit test passed");
    });
}

/// Test submit_async method
#[logfn]
pub fn test_bounded_async_worker_pool_submit_async<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        let processed_count = Arc::new(AtomicUsize::new(0));
        let workers = 4;
        let bound = 100;
        let worker_timeout = Duration::from_secs(1);

        let worker = TestAsyncWorker::<RT> { _phantom: std::marker::PhantomData };

        // Use fixed workers (min_workers == max_workers) to avoid auto-scaling complexity
        let pool = WorkerPoolBounded::new_async::<_, RT>(
            bound,
            worker,
            None,
            workers,
            workers,
            worker_timeout,
        );

        RT::sleep(Duration::from_millis(50)).await;

        let msg_count = 100;
        let wg = WaitGroup::new((), 0);

        for _ in 0..msg_count {
            let _guard = wg.add_guard();
            pool.submit_async::<RT>(AsyncTestMsg { processed: processed_count.clone(), _guard })
                .await;
        }

        wg.wait_async().await;

        assert_eq!(
            processed_count.load(Ordering::SeqCst),
            msg_count,
            "All messages should be processed"
        );

        log::info!("Bounded async submit_async test passed: {} messages processed", msg_count);
    });
}
