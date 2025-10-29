use async_executor::Executor;
use orb_smol::SmolRT;
use orb_test_utils::*;
use rstest::*;
use std::sync::Arc;

#[fixture]
fn setup() {
    init_logger();
}

#[cfg(feature = "global")]
#[rstest]
fn test_smol_rt_with_global(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new_global();
    test_runtime_basics(&rt);
    test_time_functionality(&rt);
}

#[cfg(feature = "global")]
#[rstest]
fn test_smol_tick_functionality_global(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new_global();
    test_tick_async_wait(&rt);
    test_multiple_tick_instances(&rt);
    test_stream_next(&rt);
    test_stream_multiple_next(&rt);
}

#[rstest]
fn test_smol_rt_with_executor(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    // Test with executor
    let rt = SmolRT::new(Arc::new(Executor::new()));
    test_runtime_basics(&rt);
    test_time_functionality(&rt);
}

#[rstest]
fn test_smol_tick_functionality(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    // Test tick functionality with executor
    let rt = SmolRT::new(Arc::new(Executor::new()));
    test_tick_async_wait(&rt);
    test_multiple_tick_instances(&rt);
    test_stream_next(&rt);
    test_stream_multiple_next(&rt);
}
