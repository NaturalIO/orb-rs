use orb::net::{TcpListener, TcpStream, UnifyListener, UnifyStream};
use orb::prelude::*;
use orb_compio::CompioRT;
use orb_test_utils::net::*;
use rstest::*;

#[fixture]
fn setup() {
    orb_test_utils::init_logger();
}

#[rstest]
#[case(CompioRT::one())]
#[case(CompioRT::multi(2))]
fn test_compio_tcp_listener(setup: (), #[case] rt: <CompioRT as AsyncRuntime>::Exec) {
    let _ = setup;
    test_tcp_listener::<CompioRT>(&rt);
}

#[rstest]
#[case(CompioRT::one())]
#[case(CompioRT::multi(2))]
fn test_compio_unix_listener(setup: (), #[case] rt: <CompioRT as AsyncRuntime>::Exec) {
    let _ = setup;
    test_unix_listener::<CompioRT>(&rt);
}

#[rstest]
#[case(CompioRT::one())]
#[case(CompioRT::multi(2))]
fn test_compio_unify_listener(setup: (), #[case] rt: <CompioRT as AsyncRuntime>::Exec) {
    let _ = setup;
    test_unify_listener::<CompioRT>(&rt);
}

#[rstest]
#[case(CompioRT::one())]
#[case(CompioRT::multi(2))]
fn test_compio_tcp_stream(setup: (), #[case] rt: <CompioRT as AsyncRuntime>::Exec) {
    let _ = setup;
    test_tcp_stream::<CompioRT>(&rt);
}

#[rstest]
#[case(CompioRT::one())]
#[case(CompioRT::multi(2))]
fn test_compio_unix_stream(setup: (), #[case] rt: <CompioRT as AsyncRuntime>::Exec) {
    let _ = setup;
    test_unix_stream::<CompioRT>(&rt);
}

#[rstest]
#[case(CompioRT::one())]
#[case(CompioRT::multi(2))]
fn test_compio_unify_stream(setup: (), #[case] rt: <CompioRT as AsyncRuntime>::Exec) {
    let _ = setup;
    test_unify_stream::<CompioRT>(&rt);
}
