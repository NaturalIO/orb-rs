use captains_log::logfn;
use futures_lite::future::zip;
use orb::prelude::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[logfn]
pub fn test_spawn_async<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    let result = rt.block_on(async move {
        let start_ts = Instant::now();
        let handle: RT::AsyncHandle<_> = rt.spawn(async {
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
pub fn test_spawn_blocking<RT: AsyncRuntime + std::fmt::Debug>(rt: &RT) {
    let result = rt.block_on(async {
        // test spawn_blocking in the background does not affect foreground
        let start_ts = Instant::now();
        let handle: RT::ThreadHandle<_> = RT::spawn_blocking(|| {
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
