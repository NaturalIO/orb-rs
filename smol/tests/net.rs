use async_executor::Executor;
use orb_smol::SmolRT;
use orb_test_utils::*;
use rstest::*;
use std::sync::Arc;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
fn test_tcp_client_server(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new(Arc::new(Executor::new()));

    orb_test_utils::test_tcp_client_server(&rt);
}

#[rstest]
fn test_unix_client_server(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::new(Arc::new(Executor::new()));

    orb_test_utils::test_unix_client_server(&rt);
}
