//! Runtime execution traits for async task management.
//!
//! This module defines the interface for spawning, executing, and managing
//! asynchronous tasks across different runtime implementations.
//!
//! The adaptors are provided as separate crates:
//!
//! - [orb-tokio](https://docs.rs/orb-tokio) - For the Tokio runtime
//! - [orb-smol](https://docs.rs/orb-smol) - For the Smol runtime

use std::future::Future;

/// Trait for async runtime execution capabilities.
///
/// This trait defines the core execution operations that any async runtime
/// should provide, including spawning tasks, running futures to completion,
/// and detaching tasks.
///
/// # Example
///
/// ```rust
/// use orb::prelude::*;
/// use std::future::Future;
///
/// fn example<R: AsyncExec>(runtime: &R) -> impl Future<Output = ()> {
///     async move {
///         // Spawn a task
///         let handle = runtime.spawn(async {
///             // Do some async work
///             42
///         });
///         
///         // Wait for the result
///         let result = handle.join().await.unwrap();
///         assert_eq!(result, 42);
///     }
/// }
/// ```
pub trait AsyncExec: Send + Sync + 'static {
    /// Spawn a task in the background, returning a handle to await its result.
    ///
    /// This method creates a new task that runs concurrently with the current
    /// task. The returned handle can be used to wait for the task's completion
    /// and retrieve its result.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The future type to spawn
    /// * `R` - The return type of the future
    ///
    /// # Parameters
    ///
    /// * `f` - The future to spawn
    ///
    /// # Returns
    ///
    /// A handle that implements [`AsyncJoinHandle`] and can be used to await
    /// the task's result.
    fn spawn<F, R>(&self, f: F) -> impl AsyncJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static;

    /// Spawn a task and detach it (no handle returned).
    ///
    /// This method creates a new task that runs in the background without
    /// providing a way to wait for its completion. The task will continue
    /// running until it completes or the program exits.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The future type to spawn
    /// * `R` - The return type of the future
    ///
    /// # Parameters
    ///
    /// * `f` - The future to spawn
    fn spawn_detach<F, R>(&self, f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static;

    /// Run a future to completion on the runtime.
    ///
    /// This method blocks the current thread until the provided future
    /// completes, returning its result.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The future type to run
    /// * `R` - The return type of the future
    ///
    /// # Parameters
    ///
    /// * `f` - The future to run to completion
    ///
    /// # Returns
    ///
    /// The output of the future when it completes.
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

/// A handle for managing spawned async tasks.
///
/// This trait provides methods for waiting for a task's completion or
/// detaching it to run in the background.
///
/// # Type Parameters
///
/// * `T` - The return type of the task
pub trait AsyncJoinHandle<T: Send + 'static>: Send + 'static {
    /// Wait for the task to complete and return its result.
    ///
    /// This method returns a future that resolves to either the task's
    /// successful result or an error if the task panicked.
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok(T)` if the task completed successfully,
    /// or `Err(())` if the task failed.
    fn join(self) -> impl Future<Output = Result<T, ()>> + Send;

    /// Detach the task to run in the background without waiting for its result.
    ///
    /// After calling this method, the task will continue running until it
    /// completes or the program exits, but there will be no way to retrieve
    /// its result.
    ///
    /// # Warning
    ///
    /// Some runtimes (like smol) will cancel the future if you drop the task handle
    /// without calling this method. If you want the task to continue running in
    /// the background, you must explicitly call `detach()` rather than just
    /// dropping the handle.
    fn detach(self);
}
