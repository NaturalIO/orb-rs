use orb_test_utils::*;
use orb_tokio::TokioRT;
use rstest::*;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
fn test_unify_addr_resolve(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value

    // Run the test directly
    orb_test_utils::test_unify_addr_resolve();
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_tcp_client_server(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value

    // Run the test directly without spawning a task
    orb_test_utils::test_tcp_client_server(&rt);
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_unix_client_server(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value

    // Run the test directly without spawning a task
    orb_test_utils::test_unix_client_server(&rt);
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_unify_tcp_client_server(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value

    // Run the test directly without spawning a task
    orb_test_utils::test_unify_tcp_client_server(&rt);
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_unify_unix_client_server(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value

    // Run the test directly without spawning a task
    orb_test_utils::test_unify_unix_client_server(&rt);
}
