//! Utility types and functions for async operations.
//!
//! This module provides helper types and functions that support the
//! other modules in the crate.

use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A cancellable future that can be aborted when another future completes.
    ///
    /// This struct allows you to race two futures and cancel one when the
    /// other completes. It's primarily used internally to implement timeouts.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The main future that provides the primary result
    /// * `C` - The cancellation future that, when completed, aborts the main future
    pub struct Cancellable<F, C> {
        #[pin]
        future: F,
        #[pin]
        cancel_future: C,
    }
}

impl<F: Future + Send, C: Future + Send> Cancellable<F, C> {
    /// Create a new cancellable future.
    ///
    /// # Parameters
    ///
    /// * `future` - The main future to execute
    /// * `cancel_future` - The future that, when completed, cancels the main future
    ///
    /// # Returns
    ///
    /// A new cancellable future
    pub fn new(future: F, cancel_future: C) -> Self {
        Self { future, cancel_future }
    }
}

impl<F: Future + Send, C: Future + Send> Future for Cancellable<F, C> {
    type Output = Result<F::Output, ()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut _self = self.project();
        let future = unsafe { Pin::new_unchecked(&mut _self.future) };
        if let Poll::Ready(output) = future.poll(cx) {
            return Poll::Ready(Ok(output));
        }
        let cancel_future = unsafe { Pin::new_unchecked(&mut _self.cancel_future) };
        if let Poll::Ready(_) = cancel_future.poll(cx) {
            return Poll::Ready(Err(()));
        }
        return Poll::Pending;
    }
}
