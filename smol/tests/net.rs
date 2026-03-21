use orb::prelude::*;
use orb_smol::{SmolExec, SmolRT};
use orb_test_utils::{net::*, *};
use rstest::*;

#[fixture]
fn setup() {
    init_logger();
}

#[rstest]
fn test_addr_resolve(setup: ()) {
    let _ = setup; // Explicitly ignore the fixture value
    let rt = SmolRT::current();
    test_unify_addr_resolve::<SmolRT>(&rt);
}
#[rstest]
#[case(SmolRT::current())]
#[case(SmolRT::one())]
#[case(SmolRT::multi(2))]
fn test_tcp(setup: (), #[case] rt: SmolExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_tcp_client_server::<SmolRT>(&rt);
    test_unify_tcp_client_server::<SmolRT>(&rt);
}

#[rstest]
#[case(SmolRT::current())]
#[case(SmolRT::one())]
#[case(SmolRT::multi(2))]
fn test_unix(setup: (), #[case] rt: SmolExec) {
    let _ = setup; // Explicitly ignore the fixture value
    test_unix_client_server::<SmolRT>(&rt);
    test_unify_unix_client_server::<SmolRT>(&rt);
}
