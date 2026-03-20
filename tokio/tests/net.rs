use orb::prelude::*;
use orb_test_utils::{net::*, *};
use orb_tokio::{TokioExec, TokioRT};
use rstest::*;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
#[case(TokioRT::current())]
#[case(TokioRT::multi(2))]
fn test_addr_resolve(setup: (), #[case] rt: TokioExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_unify_addr_resolve::<TokioRT>(&rt);
}

#[rstest]
#[case(TokioRT::current())]
#[case(TokioRT::multi(2))]
fn test_tcp(setup: (), #[case] rt: TokioExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_tcp_client_server::<TokioRT>(&rt);
    test_unify_tcp_client_server::<TokioRT>(&rt);
}

#[rstest]
#[case(TokioRT::current())]
#[case(TokioRT::multi(2))]
fn test_unix(setup: (), #[case] rt: TokioExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_unix_client_server::<TokioRT>(&rt);
    test_unify_unix_client_server::<TokioRT>(&rt);
}
