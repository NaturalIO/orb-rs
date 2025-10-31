use orb::prelude::*;
use orb_test_utils::{runtime::*, time::*, *};
use orb_tokio::TokioRT;
use rstest::*;
use std::time::Duration;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_tokio_rt(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value
    test_spawn_async(&rt);
    test_spawn_blocking::<TokioRT>(&rt);
    test_sleep(&rt);
    test_tick(&rt);
    test_tick_stream(&rt);
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_tokio_rt_panic(setup: (), #[case] rt: TokioRT) {
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
