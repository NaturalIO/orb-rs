use crossfire::waitgroup::{WaitGroup, WaitGroupGuard};
use orb::worker_pool::{WorkerBlocking, WorkerPoolBounded, WorkerPoolUnbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Test message for worker pool tests
#[derive(Clone)]
struct TestMsg {
    id: usize,
    processed: Arc<AtomicUsize>,
    wg: WaitGroupGuard<()>,
}

/// Blocking worker implementation for testing
#[derive(Clone)]
struct TestBlockingWorker;

impl WorkerBlocking for TestBlockingWorker {
    type Msg = TestMsg;

    fn run(&self, msg: Self::Msg) {
        std::thread::sleep(Duration::from_millis(1));
        msg.processed.fetch_add(1, Ordering::SeqCst);
    }
}

/// Test basic blocking worker pool functionality with fixed workers (no scaling)
#[test]
fn test_unbounded_blocking_worker_pool_basic() {
    let processed_count = Arc::new(AtomicUsize::new(0));
    let workers = 4;
    let worker_timeout = Duration::from_secs(1);

    let worker = TestBlockingWorker;
    // min_workers == max_workers means no dynamic scaling
    let pool = WorkerPoolUnbounded::new_blocking(worker, workers, workers, worker_timeout);

    // Verify initial worker count
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(pool.worker_count(), workers, "Should have exactly {} workers", workers);

    // Submit messages with WaitGroup for synchronization
    let msg_count = 100;
    let wg = WaitGroup::new((), 0);

    for i in 0..msg_count {
        let guard = wg.add_guard();
        pool.submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard });
    }

    // Wait for all messages to be processed
    wg.wait();

    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        msg_count,
        "All messages should be processed"
    );
    assert_eq!(pool.worker_count(), workers, "Worker count should remain constant");

    println!("Basic blocking worker pool test passed: {} messages processed", msg_count);
}

/// Slow worker that takes longer to process, allowing queue to build up
#[derive(Clone)]
struct SlowBlockingWorker;

impl WorkerBlocking for SlowBlockingWorker {
    type Msg = TestMsg;

    fn run(&self, msg: Self::Msg) {
        // Sleep longer to simulate slow processing
        std::thread::sleep(Duration::from_millis(50));
        msg.processed.fetch_add(1, Ordering::SeqCst);
    }
}

/// Test worker pool with timeout - workers should exit after timeout
#[test]
fn test_unbounded_blocking_worker_pool_timeout() {
    let processed_count = Arc::new(AtomicUsize::new(0));
    let min_workers = 1;
    let max_workers = 4;
    // Use a longer timeout so workers don't exit during the test
    let worker_timeout = Duration::from_millis(500);

    let worker = SlowBlockingWorker;
    let pool = WorkerPoolUnbounded::new_blocking(worker, min_workers, max_workers, worker_timeout);

    // Submit many messages quickly to build up queue
    let msg_count = 100;
    let wg = WaitGroup::new((), 0);

    for i in 0..msg_count {
        let guard = wg.add_guard();
        pool.submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard });
    }

    // Wait for watcher thread to detect load and spawn workers
    // Watcher checks every 1 second
    std::thread::sleep(Duration::from_secs(2));

    let scaled_count = pool.worker_count();
    println!("Workers scaled up to: {} (submitted {} messages)", scaled_count, msg_count);

    // Wait for all messages to be processed
    wg.wait();
    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        msg_count,
        "All messages should be processed"
    );

    // If we scaled up, verify workers exit after timeout
    if scaled_count > min_workers {
        // Wait for timeout - extra workers should exit
        std::thread::sleep(Duration::from_secs(2));

        let final_count = pool.worker_count();
        println!("Final worker count after timeout: {}", final_count);
        assert_eq!(final_count, min_workers, "Extra workers should exit after timeout");
        println!(
            "Timeout test passed: scaled up to {}, then back to {}",
            scaled_count, final_count
        );
    } else {
        println!("Note: Workers did not scale up, queue was processed too quickly");
    }
}

/// Test basic blocking worker pool functionality with fixed workers (no scaling)
#[test]
fn test_bounded_blocking_worker_pool_basic() {
    let processed_count = Arc::new(AtomicUsize::new(0));
    let workers = 4;
    let bound = 100;
    let worker_timeout = Duration::from_secs(1);

    let worker = TestBlockingWorker;
    let pool = WorkerPoolBounded::new_blocking(bound, worker, workers, workers, worker_timeout);

    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(pool.worker_count(), workers, "Should have exactly {} workers", workers);

    let msg_count = 100;
    let wg = WaitGroup::new((), 0);

    for i in 0..msg_count {
        let guard = wg.add_guard();
        pool.submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard });
    }

    wg.wait();

    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        msg_count,
        "All messages should be processed"
    );
    assert_eq!(pool.worker_count(), workers, "Worker count should remain constant");
}

/// Test worker pool with timeout - workers should exit after timeout
#[test]
fn test_bounded_blocking_worker_pool_timeout() {
    let processed_count = Arc::new(AtomicUsize::new(0));
    let min_workers = 1;
    let max_workers = 4;
    let bound = 100;
    let worker_timeout = Duration::from_millis(500);

    let worker = SlowBlockingWorker;
    let pool =
        WorkerPoolBounded::new_blocking(bound, worker, min_workers, max_workers, worker_timeout);

    let msg_count = 100;
    let wg = WaitGroup::new((), 0);

    for i in 0..msg_count {
        let guard = wg.add_guard();
        pool.submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard });
    }

    std::thread::sleep(Duration::from_secs(2));

    let scaled_count = pool.worker_count();

    wg.wait();
    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        msg_count,
        "All messages should be processed"
    );

    if scaled_count > min_workers {
        std::thread::sleep(Duration::from_secs(2));

        let final_count = pool.worker_count();
        assert_eq!(final_count, min_workers, "Extra workers should exit after timeout");
    }
}

/// Test try_submit when queue is full
#[test]
fn test_bounded_blocking_worker_pool_try_submit() {
    let processed_count = Arc::new(AtomicUsize::new(0));
    let workers = 1;
    let bound = 5;
    let worker_timeout = Duration::from_secs(1);

    let worker = SlowBlockingWorker;
    let pool = WorkerPoolBounded::new_blocking(bound, worker, workers, workers, worker_timeout);

    // Wait for worker to start
    std::thread::sleep(Duration::from_millis(50));

    // Fill up the queue
    for i in 0..bound {
        assert!(
            pool.try_submit(TestMsg {
                id: i,
                processed: processed_count.clone(),
                wg: WaitGroup::new((), 0).add_guard()
            })
            .is_ok()
        );
    }

    // Next submit should fail
    let result = pool.try_submit(TestMsg {
        id: bound,
        processed: processed_count.clone(),
        wg: WaitGroup::new((), 0).add_guard(),
    });
    assert!(result.is_err(), "Should fail when queue is full");

    // Wait for messages to be processed
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        bound,
        "All submitted messages should be processed"
    );
}
