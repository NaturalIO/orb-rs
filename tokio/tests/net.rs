use orb_test_utils::{net::*, *};
use orb_tokio::TokioRT;
use rstest::*;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_addr_resolve(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value
    test_unify_addr_resolve::<TokioRT>(&rt);
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_tcp(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value
    test_tcp_client_server(&rt);
    test_unify_tcp_client_server(&rt);
}

#[rstest]
#[case(TokioRT::new_multi_thread(2))]
#[case(TokioRT::new_current_thread())]
fn test_unix(setup: (), #[case] rt: TokioRT) {
    let _ = setup; // Explicitly ignore the fixture value
    test_unix_client_server(&rt);
    test_unify_unix_client_server(&rt);
}
