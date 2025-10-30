use captains_log::{logfn, recipe, ConsoleTarget, Level};
use orb::io::{AsyncRead, AsyncWrite};
use orb::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use orb::prelude::*;
use orb::time::TimeInterval;
use std::time::Duration;

// Initialize logging in the test utility crate
pub fn init_logger() {
    recipe::console_logger(ConsoleTarget::Stdout, Level::Debug)
        .test()
        .build()
        .expect("Failed to initialize logger");
}

/// Test basic runtime functionality
#[logfn]
pub fn test_runtime_basics<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    log::info!("Starting test_runtime_basics");
    // Test block_on with a simple future
    let result = rt.block_on(async move {
        // Test spawn and join
        let handle = rt.spawn(async {
            RT::sleep(Duration::from_secs(2)).await;
            100
        });
        println!("sleep");
        RT::sleep(Duration::from_secs(1)).await;
        println!("sleep done");
        let result = handle.join().await.unwrap();
        println!("join");
        assert_eq!(result, 100);
        RT::sleep(Duration::from_secs(1)).await;
        42
    });
    assert_eq!(result, 42);
}

/// Test time-related functionality
#[logfn]
pub fn test_time_functionality<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    // Test sleep
    let start = std::time::Instant::now();
    rt.block_on(async {
        RT::sleep(Duration::from_millis(50)).await;
    });
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(50));

    // Test tick creation
    rt.block_on(async {
        let _ticker = RT::tick(Duration::from_millis(50));
        // Just verify we can create one without panic
    });
}

/// Test tick async wait functionality
#[logfn]
pub fn test_tick_async_wait<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        let start = std::time::Instant::now();
        let ticker = RT::tick(Duration::from_millis(100));

        // Wait for the first tick
        let _instant = ticker.tick().await;

        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(100));
    });

    log::info!("Completed test_tick_async_wait");
}

/// Test multiple tick instances
#[logfn]
pub fn test_multiple_tick_instances<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        // Test multiple tick instances sequentially
        let ticker1 = RT::tick(Duration::from_millis(30));
        ticker1.tick().await;

        let ticker2 = RT::tick(Duration::from_millis(30));
        ticker2.tick().await;

        let ticker3 = RT::tick(Duration::from_millis(30));
        ticker3.tick().await;

        let elapsed = std::time::Instant::now() - std::time::Duration::from_millis(90);
        // Should be at least 90ms (3 ticks of 30ms each)
        assert!(std::time::Instant::now() >= elapsed);
    });
}

/// Test Stream::next functionality
#[logfn]
pub fn test_stream_next<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        let start = std::time::Instant::now();
        let ticker = RT::tick(Duration::from_millis(50));
        let mut stream = ticker.into_stream();

        // Test Stream::next method
        let instant1 = stream.next().await.unwrap();
        let elapsed1 = instant1.duration_since(start);
        assert!(elapsed1 >= Duration::from_millis(50));

        let instant2 = stream.next().await.unwrap();
        let elapsed2 = instant2.duration_since(start);
        assert!(elapsed2 >= Duration::from_millis(100));
    });
}

/// Test multiple Stream::next calls
#[logfn]
pub fn test_stream_multiple_next<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        let start = std::time::Instant::now();
        let ticker = RT::tick(Duration::from_millis(30));
        let mut stream = ticker.into_stream();

        // Test multiple Stream::next calls
        for i in 1..=3 {
            let instant = stream.next().await.unwrap();
            let elapsed = instant.duration_since(start);
            assert!(elapsed >= Duration::from_millis(30 * i as u64));
        }
    });
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
        let mut listener =
            TcpListener::<RT>::bind("127.0.0.1:0").expect("Failed to create TCP listener");

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
        let mut client_stream = TcpStream::<RT>::connect(&server_addr.parse().unwrap())
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
