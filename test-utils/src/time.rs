use captains_log::logfn;
use orb::prelude::*;
use orb::time::TimeInterval;
use std::time::{Duration, Instant};

#[logfn]
pub fn test_sleep<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    let start = Instant::now();
    rt.block_on(async {
        RT::sleep(Duration::from_millis(50)).await;
    });
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(50));
}

#[logfn]
pub fn test_tick<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        // Test multiple tick instances sequentially
        let start = Instant::now();
        let ticker1 = RT::tick(Duration::from_secs(1));
        let ticker2 = RT::tick(Duration::from_secs(1));
        let ticker3 = RT::tick(Duration::from_secs(3));
        ticker1.tick().await;
        ticker2.tick().await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_secs(1) && elapsed < Duration::from_secs(2));
        ticker3.tick().await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_secs(3) && elapsed < Duration::from_secs(4));
    });
}

/// Test Stream::next functionality
#[logfn]
pub fn test_tick_stream<RT>(rt: &RT)
where
    RT: AsyncRuntime + std::fmt::Debug,
{
    rt.block_on(async {
        let start = Instant::now();
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
