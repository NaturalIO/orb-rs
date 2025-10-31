use async_executor::Executor;
use orb::prelude::*;
use orb_smol::SmolRT;
use orb_test_utils::{runtime::*, time::*, *};
use rstest::*;
use std::sync::Arc;
use std::time::Duration;

#[fixture]
fn setup() {
    init_logger();
}

#[cfg(feature = "global")]
#[rstest]
fn test_smol_global(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new_global();
    test_spawn_async(&rt);
    test_spawn_blocking::<SmolRT>(&rt);
    test_sleep(&rt);
    test_tick(&rt);
    test_tick_stream(&rt);
}

#[rstest]
fn test_smol_rt_with_executor(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new(Arc::new(Executor::new()));
    test_spawn_async(&rt);
    test_spawn_blocking::<SmolRT>(&rt);
    test_sleep(&rt);
    test_tick(&rt);
    test_tick_stream(&rt);
}

#[rstest]
#[should_panic]
fn test_smol_rt_panic(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new(Arc::new(Executor::new()));
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
