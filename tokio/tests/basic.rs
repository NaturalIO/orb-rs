use orb_test_utils::{runtime::*, time::*, *};
use orb_tokio::TokioRT;
use rstest::*;

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
