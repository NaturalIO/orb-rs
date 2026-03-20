use orb::prelude::*;
use orb_test_utils::{runtime::*, time::*, worker_pool::*, *};
use orb_tokio::{TokioExec, TokioRT};
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
#[case(TokioRT::current())]
#[case(TokioRT::multi(2))]
fn test_tokio_rt(setup: (), #[case] rt: TokioExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_spawn_async::<TokioRT>(&rt);
    test_spawn_blocking::<TokioRT>(&rt);
    test_sleep::<TokioRT>(&rt);
    test_tick::<TokioRT>(&rt);
    test_tick_stream::<TokioRT>(&rt);
    test_boxed_async_handle::<TokioRT>(&rt);
    test_static_spawn::<TokioRT>(&rt);
    test_unbounded_async_worker_pool_basic::<TokioRT>(&rt);
    test_unbounded_async_worker_pool_timeout::<TokioRT>(&rt);
    test_bounded_async_worker_pool_basic::<TokioRT>(&rt);
    test_bounded_async_worker_pool_timeout::<TokioRT>(&rt);
}

#[rstest]
#[case(TokioRT::current())]
#[case(TokioRT::multi(2))]
fn test_tokio_rt_panic(setup: (), #[case] rt: TokioExec) {
    let _ = setup; // Explicitly ignore the fixture value
    let _rt = rt.clone();
    rt.block_on(async move {
        let handle = _rt.spawn(async {
            TokioRT::sleep(Duration::from_secs(1)).await;
            panic!("test task panic");
        });
        // the panic hook will work, but the main task is fine
        assert!(handle.await.is_err());
    });
}

#[rstest]
fn test_tokio_current(setup: ()) {
    let _ = setup;
    println!("test blockon effect");
    let counter = Arc::new(AtomicUsize::new(0));
    let _counter = counter.clone();
    let rt = TokioRT::current();
    let _rt = rt.clone();
    // background future only runs within the lifecycle on block_on
    _rt.spawn(async move {
        loop {
            TokioRT::sleep(Duration::from_secs(1)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
            println!("back sleep");
        }
    });
    rt.block_on(async move {
        TokioRT::sleep(Duration::from_secs(3)).await;
    });
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
fn test_tokio_one(setup: ()) {
    let _ = setup;
    let rt = TokioRT::one();
    let counter = Arc::new(AtomicUsize::new(0));
    let _counter = counter.clone();
    rt.spawn(async move {
        loop {
            TokioRT::sleep(Duration::from_secs(1)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
            println!("back sleep");
        }
    });
    // background future only runs within the lifecycle on block_on
    rt.block_on(async move {
        TokioRT::sleep(Duration::from_secs(3)).await;
    });
    let mut rx_count = counter.load(Ordering::SeqCst);
    assert!(rx_count >= 2 && rx_count <= 4, "{rx_count}");
    for i in 0..5 {
        std::thread::sleep(Duration::from_secs(1));
        println!("front sleep {i}");
    }
    rx_count = counter.load(Ordering::SeqCst);
    assert!(rx_count >= 6 && rx_count <= 9, "{rx_count}");
}
