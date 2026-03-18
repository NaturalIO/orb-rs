#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
#![doc = include_str!("../README.md")]

//! ## Modules
//!
//! - [`runtime`] - Traits for task spawn, join and block_on.
//! - [`io`] - Traits for asynchronous I/O operations, and buffered I/O wrapper.
//! - [`net`] - Wrapper types for networking, and a "unify" type for tcp + unix stream.
//! - [`time`] - Traits for time-related operations like sleeping and intervals
//! - [`utils`] - Utility types and functions
//!
//! At top level [AsyncRuntime] trait will combine all the capabilities, including
//! [`AsyncExec`], [`AsyncIO`], and [`AsyncTime`].
//!
//! ## Runtime Varieties Note
//!
//! ### Task Detach
//!
//! The drop behavior of task handle is unified to "detach", task will not be cancel unless
//! [abort](AsyncHandle::abort) is called.
//!
//! ### Panic
//!
//! - tokio will issolate panic between tasks, a task handle may return Err() on join.
//! - smol will not issolate panic. Although a panic hook will work, the program might panic if one
//! of the task panic. You may use feature `unwind` to enable panic capturing.
//!
//! ### Cloning
//!
//! Since [AsyncExec] cannot know the runtime type within thread-local context, we have impl Clone
//! for AsyncExec. It's common practice to Clone the runtime instance and pass it through `block_on`.
pub mod io;
pub mod net;
pub mod runtime;
pub mod time;
pub mod utils;

/// Re-export all the traits you need
///
/// This module contains all the essential traits needed to work with Orb.
/// Importing this prelude is the recommended way to use Orb in your code.
pub mod prelude {
    pub use crate::AsyncRuntime;
    pub use crate::io::{AsyncBufRead, AsyncBufWrite, AsyncFd, AsyncIO, AsyncRead, AsyncWrite};
    pub use crate::net::AsyncListener;
    pub use crate::runtime::{AsyncExec, AsyncExecDyn, AsyncHandle, ThreadHandle};
    pub use crate::time::{AsyncTime, TimeInterval};
    // Re-export the Stream trait so users can import it
    pub use futures_lite::stream::Stream;
    pub use futures_lite::stream::StreamExt;
}

use prelude::*;

/// A marker trait that combines all the core async runtime capabilities,
/// including [`AsyncExec`], [`AsyncIO`], and [`AsyncTime`]. It serves as a convenient
/// way to specify that a type provides all the core async runtime functionality.
///
/// You can write your own trait by inheriting AsyncRuntime or any other trait, to provide extra
/// functions along with the runtime object.
/// There's an blanket trait to auto impl AsyncRuntime on anything that is `Deref<Target>` to an AsyncRuntime.
pub trait AsyncRuntime: AsyncExec + AsyncIO + AsyncTime {}
