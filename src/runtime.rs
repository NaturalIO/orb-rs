//! Runtime execution traits for async task management.
//!
//! This module defines the interface for spawning, executing, and managing
//! asynchronous tasks across different runtime implementations.
//!
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
///         let result = handle.await.unwrap();
///         assert_eq!(result, 42);
///     }
/// }
/// ```
pub trait AsyncExec: Send + Sync + 'static {
    type AsyncHandle<R: Send>: AsyncHandle<R>;

    type ThreadHandle<R: Send>: ThreadHandle<R> + Send;

    /// Spawn a task in the background, returning a handle to await its result.
    ///
    /// This method creates a new task that runs concurrently with the current
    /// task. The returned handle can be used to wait for the task's completion
    /// and retrieve its result.
    ///
    /// # NOTE:
    ///
    /// The return AsyncHandle adopts the behavior of tokio.
    ///
    /// The behavior of panic varies for runtimes:
    /// - tokio will capture handle to task result,
    /// - async-executor (smol) will not capture panic, the program will exit
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
    /// A handle that implements [`AsyncHandle`] and can be used to await
    /// the task's result.
    ///
    fn spawn<F, R>(&self, f: F) -> Self::AsyncHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static;

    /// Spawn a task and detach it (no handle returned).
    ///
    /// This method creates a new task that runs in the background without
    /// providing a way to wait for its completion. The task will continue
    /// running until it completes or the program exits.
    ///
    /// # NOTE:
    ///
    /// The behavior of panic varies for runtimes:
    /// - tokio will ignore other tasks panic after detached,
    /// - async-executor (smol) will not capture panic by default, the program will exit. There's a
    /// feature switch in [orb-smol](https://docs.rs/orb-smol) to change this behavior.
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

    /// Run blocking code in a background thread pool, and return an async join handle
    ///
    /// # NOTE:
    ///
    /// This method spawn with threal pool provide by runtime in current context, globally.
    /// In order for ResolveAddr job which does not have a AsyncExec handle, so this method is static.
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
    /// A handle that implements [`ThreadHandle`] and can be used to await
    /// the call result.
    fn spawn_blocking<F, R>(f: F) -> Self::ThreadHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
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
    type AsyncHandle<R: Send> = T::AsyncHandle<R>;

    type ThreadHandle<R: Send> = T::ThreadHandle<R>;

    #[inline(always)]
    fn spawn<F, R>(&self, f: F) -> Self::AsyncHandle<R>
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
    fn spawn_blocking<F, R>(f: F) -> Self::ThreadHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        T::spawn_blocking(f)
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
/// # NOTE:
///
/// The behavior of dropping a AsyncHandle should be detach, we adopt this behavior because
/// user is more familiar with tokio's behavior. We don't want bugs when dropping the task handle unnoticed.
///
/// # Type Parameters
///
/// * `T` - The return type of the task
///
/// # Returns
///
/// A future that resolves to `Ok(T)` if the task completed successfully,
/// or `Err(())` if the task panics.
pub trait AsyncHandle<T>: Future<Output = Result<T, ()>> + Send {
    /// Whether a task can be join immediately
    fn is_finished(&self) -> bool;

    /// Detach the task to run in the background without waiting for its result.
    ///
    /// After calling this method, the task will continue running until it
    /// completes or until its runtime dropped.
    fn detach(self);

    /// Abort the task execution, don't care for it's result
    fn abort(self);
}

/// A handle for spawn_blocking()
///
/// This trait provides methods for waiting for a blocking task's completion or
/// detaching it to run in the background.
///
/// Calling await on the ThreadHandle will get Result<T, ()>.
///
/// # NOTE:
///
/// The behavior of dropping a ThreadHandle will not abort the task (since it run as pthread)
///
/// # Type Parameters
///
/// * `T` - The return type of the task
///
/// # Returns
///
/// A future that resolves to `Ok(T)` if the task completed successfully,
/// or `Err(())` if the task panics.
pub trait ThreadHandle<T>: Future<Output = Result<T, ()>> {
    /// Whether a task can be join immediately
    fn is_finished(&self) -> bool;
}
