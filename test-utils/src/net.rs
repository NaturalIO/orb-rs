use captains_log::logfn;
use orb::io::{AsyncRead, AsyncWrite};
use orb::net::{TcpListener, TcpStream, UnifyListener, UnifyStream, UnixListener, UnixStream};
use orb::prelude::*;
use std::time::Duration;

/// Test UnifyAddr resolve functionality
#[logfn]
pub fn test_unify_addr_resolve() {
    use orb::net::UnifyAddr;
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::PathBuf;

    // Test TCP address resolution
    let tcp_addr = UnifyAddr::resolve("127.0.0.1:8080").expect("Failed to resolve TCP address");
    match tcp_addr {
        UnifyAddr::Socket(addr) => {
            assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
            assert_eq!(addr.port(), 8080);
        }
        _ => panic!("Expected Socket address"),
    }

    // Test Unix socket path resolution
    let unix_addr = UnifyAddr::resolve("/tmp/test.sock").expect("Failed to resolve Unix address");
    match unix_addr {
        UnifyAddr::Path(path) => {
            assert_eq!(path, PathBuf::from("/tmp/test.sock"));
        }
        _ => panic!("Expected Path address"),
    }

    // Test hostname resolution (this should work for localhost)
    let localhost_addr = UnifyAddr::resolve("localhost:8080");
    // Note: This might fail in some environments, so we just check it doesn't panic
    if let Ok(addr) = localhost_addr {
        match addr {
            UnifyAddr::Socket(_) => {} // Expected
            _ => panic!("Expected Socket address for localhost"),
        }
    }

    // Test invalid address resolution
    let invalid_addr = UnifyAddr::resolve("invalid_address_that_does_not_exist");
    assert!(invalid_addr.is_err());
}

/// Test TCP client-server communication
#[logfn]
pub fn test_tcp_client_server<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        // Create a shared variable to store the server address

        // Use port 0 to let the OS choose a random available port
        let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut listener = TcpListener::<RT>::bind(&addr).expect("Failed to create TCP listener");

        // Get the actual port assigned by the OS
        let server_addr = listener.local_addr().expect("Failed to get local address");

        // Store the address in the shared variable

        // Start server in a separate task
        let server_handle = rt.spawn(async move {
            // Accept one connection
            let mut stream = listener.accept().await.expect("Failed to accept connection");

            // Read data from client
            let mut buffer = [0; 32];
            let n = stream.read(&mut buffer).await.expect("Failed to read from client");
            let received = String::from_utf8_lossy(&buffer[..n]);

            // Verify received data
            assert_eq!(received, "Hello from client!");

            // Send response to client
            let response = "Hello from server!";
            stream.write(response.as_bytes()).await.expect("Failed to write to client");

            // Return success
            true
        });

        // Connect as client
        let addr: std::net::SocketAddr = server_addr.parse().unwrap();
        let mut client_stream =
            TcpStream::<RT>::connect(&addr).await.expect("Failed to connect to server");

        // Send data to server
        let message = "Hello from client!";
        client_stream.write(message.as_bytes()).await.expect("Failed to write to server");

        // Read response from server
        let mut buffer = [0; 32];
        let n = client_stream.read(&mut buffer).await.expect("Failed to read from server");
        let received = String::from_utf8_lossy(&buffer[..n]);

        // Verify received data
        assert_eq!(received, "Hello from server!");

        // Wait for server to complete
        let server_result = server_handle.join().await.expect("Server task failed");
        assert!(server_result);
    });
}

/// Test Unix client-server communication
#[logfn]
pub fn test_unix_client_server<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    // Clean up any existing socket file
    let _ = std::fs::remove_file("/tmp/test_socket_client_server");

    rt.block_on(async {
        // Start server in a separate task
        let server_handle = rt.spawn(async {
            let mut listener = UnixListener::<RT>::bind("/tmp/test_socket_client_server")
                .expect("Failed to create Unix listener");

            // Accept one connection
            let mut stream = listener.accept().await.expect("Failed to accept connection");

            // Read data from client
            let mut buffer = [0; 32];
            let n = stream.read(&mut buffer).await.expect("Failed to read from client");
            let received = String::from_utf8_lossy(&buffer[..n]);

            // Verify received data
            assert_eq!(received, "Hello from client!");

            // Send response to client
            let response = "Hello from server!";
            stream.write(response.as_bytes()).await.expect("Failed to write to client");

            // Return success
            true
        });

        // Give server time to start
        RT::sleep(Duration::from_millis(100)).await;

        // Connect as client
        let mut client_stream =
            UnixStream::<RT>::connect(&std::path::PathBuf::from("/tmp/test_socket_client_server"))
                .await
                .expect("Failed to connect to server");

        // Send data to server
        let message = "Hello from client!";
        client_stream.write(message.as_bytes()).await.expect("Failed to write to server");

        // Read response from server
        let mut buffer = [0; 32];
        let n = client_stream.read(&mut buffer).await.expect("Failed to read from server");
        let received = String::from_utf8_lossy(&buffer[..n]);

        // Verify received data
        assert_eq!(received, "Hello from server!");

        // Wait for server to complete
        let server_result = server_handle.join().await.expect("Server task failed");
        assert!(server_result);
    });

    // Clean up the socket file after test
    let _ = std::fs::remove_file("/tmp/test_socket_client_server");
}

/// Test UnifyStream and UnifyListener TCP client-server communication
#[logfn]
pub fn test_unify_tcp_client_server<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        // Use port 0 to let the OS choose a random available port
        let mut listener =
            UnifyListener::<RT>::bind(&"127.0.0.1:0").expect("Failed to create TCP UnifyListener");

        // Get the actual port assigned by the OS
        let server_addr = listener.local_addr().expect("Failed to get local address");

        // Start server in a separate task
        let server_handle = rt.spawn(async move {
            // Accept one connection
            let mut stream = listener.accept().await.expect("Failed to accept connection");

            // Read data from client
            let mut buffer = [0; 32];
            let n = stream.read(&mut buffer).await.expect("Failed to read from client");
            let received = String::from_utf8_lossy(&buffer[..n]);

            // Verify received data
            assert_eq!(received, "Hello from client!");

            // Send response to client
            let response = "Hello from server!";
            stream.write(response.as_bytes()).await.expect("Failed to write to client");

            // Return success
            true
        });

        // Give server time to start
        RT::sleep(Duration::from_millis(100)).await;

        // Connect as client - use the server address string directly
        let mut client_stream =
            UnifyStream::<RT>::connect(&server_addr).await.expect("Failed to connect to server");

        // Send data to server
        let message = "Hello from client!";
        client_stream.write(message.as_bytes()).await.expect("Failed to write to server");

        // Read response from server
        let mut buffer = [0; 32];
        let n = client_stream.read(&mut buffer).await.expect("Failed to read from server");
        let received = String::from_utf8_lossy(&buffer[..n]);

        // Verify received data
        assert_eq!(received, "Hello from server!");

        // Wait for server to complete
        let server_result = server_handle.join().await.expect("Server task failed");
        assert!(server_result);
    });
}

/// Test UnifyStream and UnifyListener Unix client-server communication
#[logfn]
pub fn test_unify_unix_client_server<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    // Clean up any existing socket file
    let _ = std::fs::remove_file("/tmp/test_unify_socket_client_server");

    rt.block_on(async {
        // Start server in a separate task
        let server_handle = rt.spawn(async {
            let mut listener = UnifyListener::<RT>::bind(&"/tmp/test_unify_socket_client_server")
                .expect("Failed to create Unix UnifyListener");

            // Accept one connection
            let mut stream = listener.accept().await.expect("Failed to accept connection");

            // Read data from client
            let mut buffer = [0; 32];
            let n = stream.read(&mut buffer).await.expect("Failed to read from client");
            let received = String::from_utf8_lossy(&buffer[..n]);

            // Verify received data
            assert_eq!(received, "Hello from client!");

            // Send response to client
            let response = "Hello from server!";
            stream.write(response.as_bytes()).await.expect("Failed to write to client");

            // Return success
            true
        });

        // Give server time to start
        RT::sleep(Duration::from_millis(100)).await;

        // Connect as client
        let mut client_stream = UnifyStream::<RT>::connect(&"/tmp/test_unify_socket_client_server")
            .await
            .expect("Failed to connect to server");

        // Send data to server
        let message = "Hello from client!";
        client_stream.write(message.as_bytes()).await.expect("Failed to write to server");

        // Read response from server
        let mut buffer = [0; 32];
        let n = client_stream.read(&mut buffer).await.expect("Failed to read from server");
        let received = String::from_utf8_lossy(&buffer[..n]);

        // Verify received data
        assert_eq!(received, "Hello from server!");

        // Wait for server to complete
        let server_result = server_handle.join().await.expect("Server task failed");
        assert!(server_result);
    });

    // Clean up the socket file after test
    let _ = std::fs::remove_file("/tmp/test_unify_socket_client_server");
}
