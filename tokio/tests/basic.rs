use orb_test_utils::*;
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
    test_runtime_basics(&rt);
    test_time_functionality(&rt);
    test_spawn_blocking::<TokioRT>(&rt);
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_tokio_tick_functionality(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value
    // Test tick functionality with multi-threaded runtime
    test_tick_async_wait(&rt);
    test_multiple_tick_instances(&rt);
    test_stream_next(&rt);
    test_stream_multiple_next(&rt);
}
