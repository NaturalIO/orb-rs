//! Time-related traits and utilities for async operations.
//!
//! This module provides traits for working with time in an async context,
//! including sleeping, timeouts, and periodic timers.

use crate::utils::Cancellable;
use futures_lite::stream::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// Trait for async time-related operations.
///
/// This trait defines the interface for time-related operations such as
/// sleeping, creating intervals, and applying timeouts to futures.
///
/// # Associated Types
///
/// * `Interval` - The type used for periodic timers
pub trait AsyncTime {
    /// The type used for periodic timers.
    type Interval: TimeInterval;

    /// Sleep for the specified duration.
    ///
    /// This method returns a future that completes after the specified
    /// duration has elapsed.
    ///
    /// # Parameters
    ///
    /// * `d` - The duration to sleep
    ///
    /// # Returns
    ///
    /// A future that completes after the specified duration
    fn sleep(d: Duration) -> impl Future + Send;

    /// Create a periodic timer that ticks at the specified interval.
    ///
    /// This method creates a timer that repeatedly fires at the specified
    /// interval, useful for implementing periodic tasks.
    ///
    /// # Parameters
    ///
    /// * `d` - The interval between ticks
    ///
    /// # Returns
    ///
    /// An interval object that implements [`TimeInterval`]
    fn tick(d: Duration) -> Self::Interval;

    /// Apply a timeout to a future.
    ///
    /// This method returns a future that completes either when the provided
    /// future completes or when the specified timeout duration elapses,
    /// whichever happens first.
    ///
    /// # Parameters
    ///
    /// * `d` - The timeout duration
    /// * `func` - The future to apply the timeout to
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok` with the result of the original future
    /// if it completes before the timeout, or `Err(())` if the timeout elapses
    /// first.
    #[inline]
    fn timeout<F>(d: Duration, func: F) -> impl Future<Output = Result<F::Output, ()>> + Send
    where
        F: Future + Send,
    {
        Cancellable::new(func, Self::sleep(d))
    }
}

impl<F: std::ops::Deref<Target = T>, T: AsyncTime> AsyncTime for F {
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

/// Trait for periodic timers.
///
/// This trait defines the interface for periodic timers that can be used
/// to implement recurring tasks.
pub trait TimeInterval: Unpin + Send {
    /// Poll for the next tick.
    ///
    /// This method is used internally by the async runtime to check if
    /// the next timer tick is ready.
    ///
    /// # Parameters
    ///
    /// * `ctx` - The task context for polling
    ///
    /// # Returns
    ///
    /// A `Poll` containing the instant when the tick occurred, or `Poll::Pending`
    /// if the tick is not yet ready.
    fn poll_tick(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Instant>;

    /// Wait asynchronously for the next tick.
    ///
    /// This method returns a future that completes when the next timer tick occurs.
    ///
    /// # Returns
    ///
    /// A future that resolves to the instant when the tick occurred.
    fn tick(self) -> TickFuture<Self>
    where
        Self: Sized,
    {
        TickFuture::new(self)
    }

    /// Convert this interval into a stream.
    ///
    /// This method converts the interval into a stream that yields the
    /// instant of each tick.
    ///
    /// # Returns
    ///
    /// A stream that yields the instant of each tick.
    #[inline(always)]
    fn into_stream(self) -> IntervalStream<Self>
    where
        Self: Sized,
    {
        IntervalStream::new(self)
    }
}

/// A wrapper that implements `Stream` for a `TimeInterval`.
///
/// This struct allows a `TimeInterval` to be used as a `Stream` that
/// yields the instant of each tick.
///
/// # Type Parameters
///
/// * `T` - The underlying interval type
pub struct IntervalStream<T: TimeInterval> {
    interval: T,
}

impl<T: TimeInterval> IntervalStream<T> {
    /// Create a new interval stream.
    ///
    /// # Parameters
    ///
    /// * `interval` - The interval to wrap
    ///
    /// # Returns
    ///
    /// A new interval stream
    pub fn new(interval: T) -> Self {
        Self { interval }
    }
}

impl<T: TimeInterval> Stream for IntervalStream<T> {
    type Item = Instant;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unsafe { Pin::new_unchecked(&mut self.interval).poll_tick(ctx).map(Some) }
    }
}

/// Future for the tick operation.
///
/// This future completes when the next timer tick occurs.
///
/// # Type Parameters
///
/// * `T` - The underlying interval type
pub struct TickFuture<T: TimeInterval> {
    interval: T,
}

impl<T: TimeInterval> TickFuture<T> {
    /// Create a new tick future.
    ///
    /// # Parameters
    ///
    /// * `interval` - The interval to wait for
    ///
    /// # Returns
    ///
    /// A new tick future
    pub fn new(interval: T) -> Self {
        Self { interval }
    }
}

impl<T: TimeInterval> Future for TickFuture<T> {
    type Output = Instant;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut self.interval).poll_tick(ctx) }
    }
}
