use crossfire::waitgroup::{WaitGroup, WaitGroupGuard};
use orb::worker_pool::{Worker, WorkerBlocking, WorkerPoolBounded, WorkerPoolUnbounded};
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

impl Worker for TestBlockingWorker {
    type Msg = TestMsg;
}

impl WorkerBlocking for TestBlockingWorker {
    fn run(&self, msg: Self::Msg) {
        std::thread::sleep(Duration::from_millis(1));
        msg.processed.fetch_add(1, Ordering::SeqCst);
    }
}

/// Slow worker that takes longer to process, allowing queue to build up
#[derive(Clone)]
struct SlowBlockingWorker;

impl Worker for SlowBlockingWorker {
    type Msg = TestMsg;
}

impl WorkerBlocking for SlowBlockingWorker {
    fn run(&self, msg: Self::Msg) {
        // Sleep longer to simulate slow processing
        std::thread::sleep(Duration::from_millis(50));
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
    let pool = WorkerPoolUnbounded::builder(worker, workers)
        .max_workers(workers)
        .timeout(worker_timeout)
        .new_blocking();

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

/// Test worker pool with timeout - workers should exit after timeout
#[test]
fn test_unbounded_blocking_worker_pool_timeout() {
    let processed_count = Arc::new(AtomicUsize::new(0));
    let min_workers = 1;
    let max_workers = 4;
    // Use a longer timeout so workers don't exit during the test
    let worker_timeout = Duration::from_millis(50);

    let worker = SlowBlockingWorker;
    let pool = WorkerPoolUnbounded::builder(worker, min_workers)
        .max_workers(max_workers)
        .timeout(worker_timeout)
        .new_blocking();

    // Submit many messages quickly to build up queue
    let mut msg_count = 0;
    let wg = WaitGroup::new((), 0);

    loop {
        for i in 0..100 {
            let guard = wg.add_guard();
            pool.submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard });
            msg_count += 1;
        }
        println!("submit {msg_count} worker {}", pool.worker_count());
        if pool.worker_count() == max_workers {
            break;
        }
        std::thread::sleep(worker_timeout);
    }
    // Wait for watcher thread to detect load and spawn workers
    // Watcher checks every 1 second
    std::thread::sleep(Duration::from_secs(2));
    println!("waiting");
    // Wait for all messages to be processed
    wg.wait();
    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        msg_count,
        "All messages should be processed"
    );
    while pool.worker_count() > min_workers {
        // Wait for timeout - extra workers should exit
        std::thread::sleep(Duration::from_secs(2));
    }
    println!("current worker {}", pool.worker_count());
}

/// Test basic blocking worker pool functionality with fixed workers (no scaling)
#[test]
fn test_bounded_blocking_worker_pool_basic() {
    let processed_count = Arc::new(AtomicUsize::new(0));
    let workers = 4;
    let bound = 100;
    let worker_timeout = Duration::from_secs(1);

    let worker = TestBlockingWorker;
    let pool = WorkerPoolBounded::builder(worker, workers)
        .max_workers(workers)
        .timeout(worker_timeout)
        .new_blocking(bound);

    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(pool.worker_count(), workers, "Should have exactly {} workers", workers);

    let wg = WaitGroup::new((), 0);
    for i in 0..bound {
        let guard = wg.add_guard();
        assert!(
            pool.try_submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard })
                .is_ok()
        );
    }
    for i in 0..bound {
        let guard = wg.add_guard();
        pool.submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard });
    }
    wg.wait();
    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        bound * 2,
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
    let bound = 2;
    let worker_timeout = Duration::from_secs(1);

    let worker = SlowBlockingWorker;
    let pool = WorkerPoolBounded::builder(worker, min_workers)
        .max_workers(max_workers)
        .timeout(worker_timeout)
        .new_blocking(bound);

    let mut msg_count = 0;
    let wg = WaitGroup::new((), 0);

    while pool.worker_count() < max_workers {
        for i in 0..bound {
            let guard = wg.add_guard();
            pool.submit(TestMsg { id: i, processed: processed_count.clone(), wg: guard });
            msg_count += 1;
        }
        println!("submit {msg_count}, {}", pool.worker_count());
    }
    std::thread::sleep(Duration::from_secs(2));

    wg.wait();
    assert_eq!(
        processed_count.load(Ordering::SeqCst),
        msg_count,
        "All messages should be processed"
    );

    while pool.worker_count() > min_workers {
        std::thread::sleep(Duration::from_secs(2));
    }
    println!("cur workers {}", pool.worker_count());
}
