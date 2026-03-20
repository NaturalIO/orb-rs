use captains_log::logfn;
use futures_lite::future::zip;
use orb::prelude::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[logfn]
pub fn test_spawn_async<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    let result = rt.block_on(async move {
        let start_ts = Instant::now();
        let handle: <RT::Exec as AsyncExec>::AsyncJoiner<_> = rt.spawn(async {
            RT::sleep(Duration::from_secs(3)).await;
            100
        });
        let mut count = 0;
        while !handle.is_finished() {
            RT::sleep(Duration::from_millis(500)).await;
            count += 1;
        }
        assert_eq!(handle.await.unwrap(), 100);
        assert!(count > 5 && count <= 6);
        let elapsed = start_ts.elapsed();
        assert!(
            elapsed >= Duration::from_secs(3) && elapsed < Duration::from_secs(4),
            "{:?}",
            elapsed
        );
        // test spawn handle drop is detach

        let start_ts = Instant::now();
        let counter = Arc::new(AtomicUsize::new(0));
        let exited = Arc::new(AtomicBool::new(false));
        let _exited = exited.clone();
        let _counter = counter.clone();
        let handle = rt.spawn(async move {
            // Simulate some blocking work
            for _ in 0..5 {
                RT::sleep(std::time::Duration::from_secs(1)).await;
                _counter.fetch_add(1, Ordering::SeqCst);
            }
            _exited.store(true, Ordering::SeqCst);
            println!("background done");
        });
        RT::sleep(Duration::from_secs(1)).await;
        drop(handle);
        while !exited.load(Ordering::SeqCst) {
            RT::sleep(Duration::from_millis(300)).await;
            println!("check");
        }
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        let elapsed = start_ts.elapsed();
        assert!(
            elapsed < Duration::from_secs(6) && elapsed >= Duration::from_secs(5),
            "{:?}",
            elapsed
        );
        42
    });
    assert_eq!(result, 42);
}

#[logfn]
pub fn test_spawn_blocking<RT: AsyncRuntime>(rt: &RT::Exec) {
    let result = rt.block_on(async {
        // test spawn_blocking in the background does not affect foreground
        let start_ts = Instant::now();
        let handle: <RT::Exec as AsyncExec>::ThreadJoiner<_> = RT::spawn_blocking(|| {
            std::thread::sleep(Duration::from_secs(3));
            println!("back ground done");
            42
        });
        let async_f = async move {
            for _i in 0..2 {
                RT::sleep(Duration::from_millis(400)).await;
                println!("check");
            }
            41
        };
        while !handle.is_finished() {
            RT::sleep(Duration::from_millis(300)).await;
        }
        let (r1, r2) = zip(async_f, handle).await;
        assert_eq!(r1, 41);
        assert_eq!(r2, Ok(42));
        let elapsed = start_ts.elapsed();
        assert!(
            elapsed < Duration::from_secs(4) && elapsed >= Duration::from_secs(3),
            "{:?}",
            elapsed
        );

        // test spawn_blocking handle drop has no effect to the background
        let start_ts = Instant::now();
        let counter = Arc::new(AtomicUsize::new(0));
        let exited = Arc::new(AtomicBool::new(false));
        let _exited = exited.clone();
        let _counter = counter.clone();
        let handle = RT::spawn_blocking(move || {
            // Simulate some blocking work
            for _ in 0..5 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                _counter.fetch_add(1, Ordering::SeqCst);
            }
            _exited.store(true, Ordering::SeqCst);
            println!("background done");
        });
        RT::sleep(Duration::from_secs(1)).await;
        drop(handle);
        while !exited.load(Ordering::SeqCst) {
            RT::sleep(Duration::from_millis(100)).await;
            println!("check");
        }
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        let elapsed = start_ts.elapsed();
        assert!(
            elapsed < Duration::from_secs(6) && elapsed >= Duration::from_secs(5),
            "{:?}",
            elapsed
        );
        1
    });
    assert_eq!(result, 1);
}

/// Test Box<dyn AsyncJoiner> functionality including detach, abort, and join
#[logfn]
pub fn test_boxed_async_handle<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    use std::pin::Pin;

    rt.block_on(async {
        // Test 1: Box<dyn AsyncJoiner> with join
        let handle: <RT::Exec as AsyncExec>::AsyncJoiner<i32> = rt.spawn(async {
            RT::sleep(Duration::from_millis(100)).await;
            42
        });
        let mut boxed: Box<dyn AsyncJoiner<i32>> = Box::new(handle);
        assert!(!boxed.is_finished());
        RT::sleep(Duration::from_millis(150)).await;
        assert!(boxed.is_finished());
        // Pin the boxed handle and await it
        let pinned = Pin::new(&mut boxed);
        let result = pinned.await;
        assert_eq!(result, Ok(42));

        // Test 2: Box<dyn AsyncJoiner> with detach
        let counter = Arc::new(AtomicUsize::new(0));
        let exited = Arc::new(AtomicBool::new(false));
        let _exited = exited.clone();
        let _counter = counter.clone();
        let handle: <RT::Exec as AsyncExec>::AsyncJoiner<()> = rt.spawn(async move {
            for _ in 0..3 {
                RT::sleep(Duration::from_millis(100)).await;
                _counter.fetch_add(1, Ordering::SeqCst);
            }
            _exited.store(true, Ordering::SeqCst);
        });
        let boxed: Box<dyn AsyncJoiner<()>> = Box::new(handle);
        RT::sleep(Duration::from_millis(50)).await;
        boxed.detach_boxed();
        while !exited.load(Ordering::SeqCst) {
            RT::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(counter.load(Ordering::SeqCst), 3);

        // Test 3: Box<dyn AsyncJoiner> with abort
        let counter = Arc::new(AtomicUsize::new(0));
        let _counter = counter.clone();
        let handle: <RT::Exec as AsyncExec>::AsyncJoiner<()> = rt.spawn(async move {
            for _ in 0..10 {
                RT::sleep(Duration::from_millis(100)).await;
                _counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        let boxed: Box<dyn AsyncJoiner<()>> = Box::new(handle);
        RT::sleep(Duration::from_millis(50)).await;
        boxed.abort_boxed();
        RT::sleep(Duration::from_millis(300)).await;
        // Task should have been aborted, counter should be less than 10
        let count = counter.load(Ordering::SeqCst);
        assert!(count < 5, "Task should have been aborted, got count: {}", count);

        42
    });
}

/// Test AsyncRuntime::spawn static method
#[logfn]
pub fn test_static_spawn<RT>(rt: &RT::Exec)
where
    RT: AsyncRuntime,
{
    rt.block_on(async {
        // Test static spawn
        let handle = RT::spawn(async {
            RT::sleep(Duration::from_millis(100)).await;
            42
        });
        let result = handle.await.unwrap();
        assert_eq!(result, 42);

        // Test static spawn_detach
        let counter = Arc::new(AtomicUsize::new(0));
        let _counter = counter.clone();
        RT::spawn_detach(async move {
            RT::sleep(Duration::from_millis(50)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
        });
        RT::sleep(Duration::from_millis(150)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        42
    });
}
