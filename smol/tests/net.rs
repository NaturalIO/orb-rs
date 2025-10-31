use async_executor::Executor;
use orb_smol::SmolRT;
use orb_test_utils::{net::*, *};
use rstest::*;
use std::sync::Arc;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
fn test_addr_resolve(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new(Arc::new(Executor::new()));
    test_unify_addr_resolve::<SmolRT>(&rt);
}
#[rstest]
#[case(SmolRT::new(std::sync::Arc::new(async_executor::Executor::new())))]
#[cfg(feature = "global")]
#[case(SmolRT::new_global())]
fn test_tcp(setup: (), #[case] rt: SmolRT) {
    let _ = setup; // Explicitly ignore the fixture value
    test_tcp_client_server(&rt);
    test_unify_tcp_client_server(&rt);
}

#[rstest]
#[case(SmolRT::new(std::sync::Arc::new(async_executor::Executor::new())))]
#[cfg(feature = "global")]
#[case(SmolRT::new_global())]
fn test_unix(setup: (), #[case] rt: SmolRT) {
    let _ = setup; // Explicitly ignore the fixture value
    test_unix_client_server(&rt);
    test_unify_unix_client_server(&rt);
}
