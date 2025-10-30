use orb::net::UnifyAddr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
use std::path::PathBuf;

#[test]
fn test_unify_addr_from_socket_addr_v4() {
    let ipv4_addr = Ipv4Addr::new(127, 0, 0, 1);
    let socket_v4 = SocketAddrV4::new(ipv4_addr, 8080);
    let unify_addr: UnifyAddr = socket_v4.into();

    match unify_addr {
        UnifyAddr::Socket(SocketAddr::V4(addr)) => {
            assert_eq!(*addr.ip(), ipv4_addr);
            assert_eq!(addr.port(), 8080);
        }
        _ => panic!("Expected SocketAddrV4"),
    }
}

#[test]
fn test_unify_addr_from_socket_addr_v6() {
    let ipv6_addr = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
    let socket_v6 = SocketAddrV6::new(ipv6_addr, 9090, 0, 0);
    let unify_addr: UnifyAddr = socket_v6.into();

    match unify_addr {
        UnifyAddr::Socket(SocketAddr::V6(addr)) => {
            assert_eq!(*addr.ip(), ipv6_addr);
            assert_eq!(addr.port(), 9090);
        }
        _ => panic!("Expected SocketAddrV6"),
    }
}

#[test]
fn test_unify_addr_from_socket_addr() {
    let ipv4_addr = Ipv4Addr::new(192, 168, 1, 1);
    let socket_addr: SocketAddr = SocketAddr::new(IpAddr::V4(ipv4_addr), 3000);
    let unify_addr: UnifyAddr = socket_addr.into();

    match unify_addr {
        UnifyAddr::Socket(addr) => {
            assert_eq!(addr.ip(), IpAddr::V4(ipv4_addr));
            assert_eq!(addr.port(), 3000);
        }
        _ => panic!("Expected SocketAddr"),
    }
}

#[test]
fn test_unify_addr_from_ip_port_tuple() {
    let ipv4_addr = Ipv4Addr::new(127, 0, 0, 1);
    let port = 5000u16;
    let unify_addr: UnifyAddr = (ipv4_addr, port).into();

    match unify_addr {
        UnifyAddr::Socket(addr) => {
            assert_eq!(addr.ip(), IpAddr::V4(ipv4_addr));
            assert_eq!(addr.port(), 5000);
        }
        _ => panic!("Expected SocketAddr from (IpAddr, u16) tuple"),
    }

    // Test with IpAddr directly
    let ip_addr = IpAddr::V6(Ipv6Addr::LOCALHOST);
    let port = 6000u16;
    let unify_addr: UnifyAddr = (ip_addr, port).into();

    match unify_addr {
        UnifyAddr::Socket(addr) => {
            assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
            assert_eq!(addr.port(), 6000);
        }
        _ => panic!("Expected SocketAddr from (IpAddr, u16) tuple with IpAddr"),
    }
}

#[test]
fn test_unify_addr_from_path_buf() {
    let path = PathBuf::from("/tmp/test.sock");
    let unify_addr: UnifyAddr = path.clone().into();

    match unify_addr {
        UnifyAddr::Path(p) => {
            assert_eq!(p, path);
        }
        _ => panic!("Expected PathBuf"),
    }
}

#[test]
fn test_unify_addr_display() {
    // Test SocketAddrV4 display
    let ipv4_addr = Ipv4Addr::new(127, 0, 0, 1);
    let socket_v4 = SocketAddrV4::new(ipv4_addr, 8080);
    let unify_addr: UnifyAddr = socket_v4.into();
    assert_eq!(unify_addr.to_string(), "127.0.0.1:8080");

    // Test PathBuf display
    let path = PathBuf::from("/tmp/test.sock");
    let unify_path: UnifyAddr = path.into();
    assert_eq!(unify_path.to_string(), "/tmp/test.sock");
}

#[test]
fn test_unify_addr_equality() {
    // Test SocketAddrV4 equality
    let addr1: UnifyAddr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080).into();
    let addr2: UnifyAddr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080).into();
    assert_eq!(addr1, addr2);

    // Test PathBuf equality
    let path = PathBuf::from("/tmp/test.sock");
    let path1: UnifyAddr = path.clone().into();
    let path2: UnifyAddr = path.into();
    assert_eq!(path1, path2);

    // Test inequality
    let addr3: UnifyAddr = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 8080).into();
    assert_ne!(addr1, addr3);

    // Test different types
    let path_diff: UnifyAddr = PathBuf::from("/tmp/test.sock").into();
    assert_ne!(addr1, path_diff);
}

#[test]
fn test_unify_addr_parse() {
    // Test TCP address parsing
    let tcp_addr = UnifyAddr::parse("127.0.0.1:8080").expect("Failed to parse TCP address");
    match tcp_addr {
        UnifyAddr::Socket(addr) => {
            assert_eq!(addr.ip().to_string(), "127.0.0.1");
            assert_eq!(addr.port(), 8080);
        }
        _ => panic!("Expected Socket address"),
    }

    // Test Unix socket path parsing
    let unix_addr = UnifyAddr::parse("/tmp/test.sock").expect("Failed to parse Unix address");
    match unix_addr {
        UnifyAddr::Path(path) => {
            assert_eq!(path, PathBuf::from("/tmp/test.sock"));
        }
        _ => panic!("Expected Path address"),
    }

    // Test parse does not resolve
    let invalid_tcp = UnifyAddr::parse("www.baidu.com");
    assert!(invalid_tcp.is_err());

    // Test invalid TCP address
    let invalid_tcp = UnifyAddr::parse("invalid_address");
    assert!(invalid_tcp.is_err());

    // Test IPv6 address
    let ipv6_addr = UnifyAddr::parse("[::1]:8080").expect("Failed to parse IPv6 address");
    match ipv6_addr {
        UnifyAddr::Socket(addr) => {
            assert_eq!(addr.port(), 8080);
        }
        _ => panic!("Expected Socket address"),
    }
}

#[test]
fn test_unify_addr_to_socket_addrs() {
    // Test SocketAddr conversion
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let unify_addr: UnifyAddr = socket_addr.into();

    let mut addrs = unify_addr.to_socket_addrs().expect("Failed to convert to socket addrs");
    let addr = addrs.next().expect("Expected at least one address");
    assert_eq!(addr, socket_addr);
    assert!(addrs.next().is_none()); // Should only have one address

    // Test SocketAddrV4 conversion
    let socket_v4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    let unify_addr: UnifyAddr = socket_v4.into();

    let mut addrs = unify_addr.to_socket_addrs().expect("Failed to convert to socket addrs");
    let addr = addrs.next().expect("Expected at least one address");
    assert_eq!(addr, SocketAddr::V4(socket_v4));
    assert!(addrs.next().is_none()); // Should only have one address

    // Test SocketAddrV6 conversion
    let socket_v6 = SocketAddrV6::new(Ipv6Addr::LOCALHOST, 8080, 0, 0);
    let unify_addr: UnifyAddr = socket_v6.into();

    let mut addrs = unify_addr.to_socket_addrs().expect("Failed to convert to socket addrs");
    let addr = addrs.next().expect("Expected at least one address");
    assert_eq!(addr, SocketAddr::V6(socket_v6));
    assert!(addrs.next().is_none()); // Should only have one address

    // Test Path conversion should fail
    let path = PathBuf::from("/tmp/test.sock");
    let unify_addr: UnifyAddr = path.into();

    let result = unify_addr.to_socket_addrs();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}
