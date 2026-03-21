use orb::prelude::*;
use orb_smol::{SmolExec, SmolRT};
use orb_test_utils::{runtime::*, time::*, worker_pool::*, *};
use rstest::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
#[case(SmolRT::current())]
#[case(SmolRT::one())]
#[case(SmolRT::multi(0))]
fn test_smol_rt(setup: (), #[case] rt: SmolExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_spawn_async::<SmolRT>(&rt);
    test_spawn_blocking::<SmolRT>(&rt);
    test_sleep::<SmolRT>(&rt);
    test_tick::<SmolRT>(&rt);
    test_tick_stream::<SmolRT>(&rt);
    test_boxed_async_handle::<SmolRT>(&rt);
    test_static_spawn::<SmolRT>(&rt);
    test_unbounded_async_worker_pool_basic::<SmolRT>(&rt);
    test_unbounded_async_worker_pool_timeout::<SmolRT>(&rt);
    test_bounded_async_worker_pool_basic::<SmolRT>(&rt);
    test_bounded_async_worker_pool_timeout::<SmolRT>(&rt);
    test_bounded_async_worker_pool_try_submit::<SmolRT>(&rt);
    test_bounded_async_worker_pool_submit_async::<SmolRT>(&rt);
}

#[rstest]
fn test_smol_current_blockon(setup: ()) {
    let _ = setup;
    println!("test blockon effect");
    let counter = Arc::new(AtomicUsize::new(0));
    let _counter = counter.clone();
    let rt = SmolRT::current();
    rt.spawn(async move {
        loop {
            SmolRT::sleep(Duration::from_secs(1)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
            println!("back sleep");
        }
    });
    // background future only runs within the lifecycle on block_on
    rt.block_on(SmolRT::sleep(Duration::from_secs(3)));
    let mut rx_count = counter.load(Ordering::SeqCst);
    assert!(rx_count >= 2 && rx_count <= 4, "{rx_count}");
    for i in 0..5 {
        std::thread::sleep(Duration::from_secs(1));
        println!("front sleep {i}");
    }
    rx_count = counter.load(Ordering::SeqCst);
    assert!(rx_count >= 2 && rx_count <= 4, "{rx_count}");
}

#[rstest]
fn test_smol_one_blockon(setup: ()) {
    let _ = setup;
    let rt = SmolRT::one();
    let counter = Arc::new(AtomicUsize::new(0));
    let _counter = counter.clone();
    rt.spawn(async move {
        loop {
            SmolRT::sleep(Duration::from_secs(1)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
            println!("back sleep");
        }
    });
    // background future only runs within the lifecycle on block_on
    rt.block_on(SmolRT::sleep(Duration::from_secs(3)));
    let mut rx_count = counter.load(Ordering::SeqCst);
    assert!(rx_count >= 2 && rx_count <= 4, "{rx_count}");
    for i in 0..5 {
        std::thread::sleep(Duration::from_secs(1));
        println!("front sleep {i}");
    }
    rx_count = counter.load(Ordering::SeqCst);
    assert!(rx_count >= 6 && rx_count <= 9, "{rx_count}");
}

#[cfg(not(feature = "unwind"))]
#[rstest]
#[should_panic]
fn test_smol_rt_panic(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::current();
    let _rt = rt.clone();
    // the panic hook will work, but the program will terminate
    rt.block_on(async move {
        let handle = _rt.spawn(async {
            SmolRT::sleep(Duration::from_secs(1)).await;
            panic!("test task panic");
        });
        let _ = handle.await;
    });
}

#[cfg(feature = "unwind")]
#[rstest]
fn test_smol_rt_panic(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::current();
    let _rt = rt.clone();
    // the panic hook will work, but the program will terminate
    rt.block_on(async move {
        let handle = _rt.spawn(async {
            SmolRT::sleep(Duration::from_secs(1)).await;
            panic!("test task panic");
        });
        assert!(handle.await.is_err());
        println!("panic captured");
    });
}
