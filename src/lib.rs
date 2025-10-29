pub mod io;
pub mod runtime;
pub mod time;
pub mod utils;

/// Re-export all the traits you need
pub mod prelude {
    pub use crate::time::{AsyncTime, TimeInterval};
    pub use crate::io::AsyncIO;
    pub use crate::runtime::{AsyncExec, AsyncJoinHandle};
    pub use crate::AsyncRuntime;
    pub use crate::io::AsyncFd;
    // Re-export the Stream trait so users can import it
    pub use futures_lite::stream::Stream;
    pub use futures_lite::stream::StreamExt;
}

use prelude::*;

pub trait AsyncRuntime: AsyncExec + AsyncIO + AsyncTime {}

impl<F: std::ops::Deref<Target = T> + Send + Sync + 'static, T: AsyncRuntime> AsyncRuntime for F {}
