use orb::prelude::*;
use orb_compio::{CompioExec, CompioRT};
use orb_test_utils::{runtime::*, time::*, *};
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
#[case(CompioRT::one())]
#[case(CompioRT::multi(2))]
fn test_compio_rt(setup: (), #[case] rt: CompioExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_spawn_async::<CompioRT>(&rt);
    test_spawn_blocking::<CompioRT>(&rt);
    test_sleep::<CompioRT>(&rt);
    test_tick::<CompioRT>(&rt);
    test_tick_stream::<CompioRT>(&rt);
    test_boxed_async_handle::<CompioRT>(&rt);
    test_static_spawn::<CompioRT>(&rt);
}

#[rstest]
fn test_compio_one(setup: ()) {
    let _ = setup;
    let rt = CompioRT::one();
    let counter = Arc::new(AtomicUsize::new(0));
    let _counter = counter.clone();
    rt.spawn(async move {
        loop {
            CompioRT::sleep(Duration::from_secs(1)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
            println!("back sleep");
        }
    });
    // background future only runs within the lifecycle on block_on
    rt.block_on(async move {
        CompioRT::sleep(Duration::from_secs(3)).await;
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

#[rstest]
fn test_compio_multi(setup: ()) {
    let _ = setup;
    let rt = CompioRT::multi(4);
    let counter = Arc::new(AtomicUsize::new(0));
    let _counter = counter.clone();
    rt.spawn(async move {
        loop {
            CompioRT::sleep(Duration::from_secs(1)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
            println!("back sleep");
        }
    });
    // background future only runs within the lifecycle on block_on
    rt.block_on(async move {
        CompioRT::sleep(Duration::from_secs(3)).await;
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

#[rstest]
fn test_compio_current(setup: ()) {
    let _ = setup;
    println!("test blockon effect");
    let counter = Arc::new(AtomicUsize::new(0));
    let _counter = counter.clone();
    let rt = CompioRT::current();
    let _rt = rt.clone();
    // background future only runs within the lifecycle on block_on
    _rt.spawn(async move {
        loop {
            CompioRT::sleep(Duration::from_secs(1)).await;
            _counter.fetch_add(1, Ordering::SeqCst);
            println!("back sleep");
        }
    });
    rt.block_on(async move {
        CompioRT::sleep(Duration::from_secs(3)).await;
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
