#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
#![doc = include_str!("../README.md")]

//! ## Runtime Varieties Note
//!
//! ### Task Detach
//!
//! The drop behavior of task handle is unified to "detach", task will not be cancel unless
//! [abort](AsyncJoiner::abort) is called.
//!
//! ### Panic
//!
//! - tokio will issolate panic between tasks, a task handle may return Err() on join.
//! - smol will not issolate panic. Although a panic hook will work, the program might panic if one
//! of the task panic. You may use feature `unwind` to enable panic capturing.
//!
//! ### thread local context
//!
//! When the first time enter block_on in `current()` runtime, or threaded worker spawn,
//! we have maintain thread local ref for the runtime, so that you can use [AsyncRuntime::spawn()] static method directly.
//! (Note that calling `AsyncRuntime::spawn()` from non-async context will panic (just like `tokio::spawn` behavior)
//!
//! ## Cloning the runtime
//!
//! [AsyncExec] is a cloneable runtime handle, can be initiated by [AsyncRuntime::current()], [AsyncRuntime::one()], [AsyncRuntime::multi()]

pub mod io;
pub mod net;
pub mod runtime;
pub mod time;
pub mod utils;
#[cfg(feature = "worker")]
pub mod worker_pool;

/// Re-export all the traits you need
///
/// This module contains all the essential traits needed to work with Orb.
/// Importing this prelude is the recommended way to use Orb in your code.
pub mod prelude {
    pub use crate::AsyncRuntime;
    pub use crate::io::{AsyncBufRead, AsyncBufWrite, AsyncFd, AsyncIO, AsyncRead, AsyncWrite};
    pub use crate::net::AsyncListener;
    pub use crate::runtime::{AsyncExec, AsyncJoiner, ThreadJoiner};
    pub use crate::time::{AsyncTime, TimeInterval};
    // Re-export the Stream trait so users can import it
    pub use futures_lite::stream::Stream;
    pub use futures_lite::stream::StreamExt;
}

use prelude::*;

/// A marker trait that combines all the core async runtime capabilities,
/// including [`AsyncIO`], and [`AsyncTime`]. It serves as a convenient
/// way to specify that a type provides all the core async runtime functionality.
///
/// You can write your own trait by inheriting AsyncRuntime or any other trait, to provide extra
/// functions along with the runtime object.
/// There's an blanket trait to auto impl AsyncRuntime on anything that is `Deref<Target>` to an AsyncRuntime.
pub trait AsyncRuntime: AsyncIO + AsyncTime + 'static + Send + Sync {
    type Exec: AsyncExec;

    /// Initiate executor using current thread.
    ///
    /// # Safety
    ///
    /// You should run [AsyncExec::block_on()] with this executor.
    ///
    /// If spawn without a `block_on()` running, it's possible
    /// the runtime just init future without scheduling.
    fn current() -> Self::Exec;

    /// Initiate executor with one background thread.
    ///
    /// # NOTE
    ///
    /// [AsyncExec::block_on()] is optional, you can directly call [AsyncExec::spawn] with it.
    fn one() -> Self::Exec;

    /// Initiate executor with multiple background threads.
    ///
    /// # NOTE
    ///
    /// When `num` == 0, start threads that match cpu number.
    ///
    /// [AsyncExec::block_on()] is optional, you can directly call [AsyncExec::spawn] with it.
    fn multi(num: usize) -> Self::Exec;

    /// Spawn a task with Self::Exec in the thread context, returning a handle to await its result.
    ///
    /// This method creates a new task that runs concurrently with the current
    /// task. The returned handle can be used to wait for the task's completion
    /// and retrieve its result.
    ///
    /// # NOTE:
    ///
    /// The return AsyncJoiner adopts the behavior of tokio.
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
    /// A handle that implements [`AsyncJoiner`] and can be used to await
    /// the task's result.
    ///
    /// # Panic
    ///
    /// Panic when not called in the runtime worker context.
    fn spawn<F, R>(f: F) -> <Self::Exec as AsyncExec>::AsyncJoiner<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static;

    /// Spawn a task with Self::Exec in the thread context, and detach it (no handle returned).
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
    ///   feature switch in [orb-smol](https://docs.rs/orb-smol) to change this behavior.
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
    /// # Panic
    ///
    /// Panic when not called in the runtime worker context.
    fn spawn_detach<F, R>(f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static;

    /// Run blocking code with the thread context in blocking thread pool,
    /// and return an async join handle
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
    /// A handle that implements [`ThreadJoiner`] and can be used to await
    /// the call result.
    ///
    /// # Panic
    ///
    /// Panic when not called in the runtime worker context.
    fn spawn_blocking<F, R>(f: F) -> <Self::Exec as AsyncExec>::ThreadJoiner<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static;
}
