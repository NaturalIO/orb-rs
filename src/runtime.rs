//! The runtime model defines interface to adapt various async runtimes.
//!
//! The adaptors are provided as sub-crates:
//!
//! - [orb-tokio](https://docs.rs/orb-tokio)
//!
//! - [orb-smol](https://docs.rs/orb-smol)
//!

use std::future::Future;

/// Defines the execution-related interface we used from async runtime
pub trait AsyncExec: Send + Sync + 'static {
    /// Spawn a task in the background, returning a handle to await its result
    fn spawn<F, R>(&self, f: F) -> impl AsyncJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static;

    /// Spawn a task and detach it (no handle returned)
    fn spawn_detach<F, R>(&self, f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static;

    /// Run a future to completion on the runtime
    fn block_on<F, R>(&self, f: F) -> R
    where
        F: Future<Output = R> + Send,
        R: Send + 'static;
}

impl<FT: std::ops::Deref<Target = T> + Send + Sync + 'static, T: AsyncExec> AsyncExec for FT {
    #[inline(always)]
    fn spawn<F, R>(&self, f: F) -> impl AsyncJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        T::spawn(self.deref(), f)
    }

    #[inline(always)]
    fn spawn_detach<F, R>(&self, f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        T::spawn_detach(self.deref(), f)
    }

    #[inline(always)]
    fn block_on<F, R>(&self, f: F) -> R
    where
        F: Future<Output = R> + Send,
        R: Send + 'static,
    {
        T::block_on(self, f)
    }
}

/// A handle that can be used to await the result of a spawned task.
pub trait AsyncJoinHandle<T: Send + 'static>: Send + 'static {
    /// Detaches the task, allowing it to run in the background without waiting for its result.
    fn join(self) -> impl Future<Output = Result<T, ()>> + Send;

    fn detach(self);
}
