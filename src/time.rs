use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use crate::utils::Cancellable;
use futures_lite::stream::Stream;

/// Defines the time-related interface we used from async runtime
pub trait AsyncTime: Send + Sync + 'static {
    type Interval: TimeInterval;

    fn sleep(d: Duration) -> impl Future + Send;

    fn tick(d: Duration) -> Self::Interval;

    #[inline]
    fn timeout<F>(d: Duration, func: F) -> impl Future<Output = Result<F::Output, ()>> + Send
    where
        F: Future + Send,
    {
        Cancellable::new(func, Self::sleep(d))
    }
}

impl<F: std::ops::Deref<Target = T> + Send + Sync + 'static, T: AsyncTime> AsyncTime for F {
    type Interval = T::Interval;

    #[inline(always)]
    fn sleep(d: Duration) -> impl Future + Send {
        T::sleep(d)
    }

    #[inline(always)]
    fn tick(d: Duration) -> Self::Interval {
        T::tick(d)
    }
}


/// Defines the universal interval/ticker trait
pub trait TimeInterval: Unpin + Send {
    fn poll_tick(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Instant>;

    /// Async method that waits for the next tick
    fn tick(self) -> TickFuture<Self>
    where
        Self: Sized,
    {
        TickFuture::new(self)
    }

    /// Convert this TimeInterval into a Stream
    #[inline(always)]
    fn into_stream(self) -> IntervalStream<Self>
    where
        Self: Sized,
    {
        IntervalStream::new(self)
    }
}

/// A wrapper that implements Stream for a TimeInterval
pub struct IntervalStream<T: TimeInterval> {
    interval: T,
}

impl<T: TimeInterval> IntervalStream<T> {
    pub fn new(interval: T) -> Self {
        Self { interval }
    }
}

impl<T: TimeInterval> Stream for IntervalStream<T> {
    type Item = Instant;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unsafe {
            Pin::new_unchecked(&mut self.interval).poll_tick(ctx).map(Some)
        }
    }
}

/// Future for the tick operation
pub struct TickFuture<T: TimeInterval> {
    interval: T,
}

impl<T: TimeInterval> TickFuture<T> {
    pub fn new(interval: T) -> Self {
        Self { interval }
    }
}

impl<T: TimeInterval> Future for TickFuture<T> {
    type Output = Instant;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            Pin::new_unchecked(&mut self.interval).poll_tick(ctx)
        }
    }
}
