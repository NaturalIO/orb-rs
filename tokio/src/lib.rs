//! # Orb Tokio Runtime
//!
//! This crate provides a Tokio-based implementation of the Orb async runtime traits.
//! It allows users to leverage Tokio's powerful async runtime with the unified Orb interface.
//!
//! The main type provided is [`TokioRT`], which implements the core runtime functionality.
//!
//! See the [main Orb documentation](https://github.com/NaturalIO/orb) for more information.
//!
//! ## Usage
//!
//! ```rust
//! use orb_tokio::TokioRT;
//!
//! let rt = TokioRT::new_multi_thread(4);
//! ```

use orb::io::{AsyncFd, AsyncIO};
pub use orb::runtime::{AsyncExec, AsyncJoinHandle};
use orb::time::{AsyncTime, TimeInterval};
use std::fmt;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::ops::Deref;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::*;
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Handle, Runtime};

/// The main struct for tokio runtime IO, assign this type to AsyncIO trait when used.
pub enum TokioRT {
    Runtime(Runtime),
    Handle(Handle),
}

impl fmt::Debug for TokioRT {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Runtime(_) => write!(f, "tokio(rt)"),
            Self::Handle(_) => write!(f, "tokio(handle)"),
        }
    }
}

impl TokioRT {
    /// Capture a runtime
    #[inline]
    pub fn new_with_runtime(rt: Runtime) -> Self {
        Self::Runtime(rt)
    }

    #[inline]
    pub fn new_multi_thread(workers: usize) -> Self {
        let mut builder = Builder::new_multi_thread();
        if workers > 0 {
            builder.worker_threads(workers);
        }
        Self::Runtime(builder.enable_all().build().unwrap())
    }

    #[inline]
    pub fn new_current_thread() -> Self {
        let mut builder = Builder::new_current_thread();
        Self::Runtime(builder.enable_all().build().unwrap())
    }

    /// Only capture a runtime handle. Should acquire with
    /// `async { Handle::current() }`
    #[inline]
    pub fn new_with_handle(handle: Handle) -> Self {
        Self::Handle(handle)
    }
}

impl orb::AsyncRuntime for TokioRT {}

impl AsyncIO for TokioRT {
    type AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static> = TokioFD<T>;

    #[inline(always)]
    async fn connect_tcp(addr: &SocketAddr) -> io::Result<Self::AsyncFd<TcpStream>> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        // into_std will not change back to blocking
        Self::to_async_fd_rw(stream.into_std()?)
    }

    #[inline(always)]
    async fn connect_unix(addr: &PathBuf) -> io::Result<Self::AsyncFd<UnixStream>> {
        let stream = tokio::net::UnixStream::connect(addr).await?;
        // into_std will not change back to blocking
        Self::to_async_fd_rw(stream.into_std()?)
    }

    #[inline(always)]
    fn to_async_fd_rd<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        use tokio::io;
        Ok(TokioFD(io::unix::AsyncFd::with_interest(fd, io::Interest::READABLE)?))
    }

    #[inline(always)]
    fn to_async_fd_rw<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        use tokio::io;
        use tokio::io::Interest;
        Ok(TokioFD(io::unix::AsyncFd::with_interest(fd, Interest::READABLE | Interest::WRITABLE)?))
    }
}

impl AsyncTime for TokioRT {
    type Interval = TokioInterval;

    #[inline(always)]
    fn sleep(d: Duration) -> impl Future + Send {
        tokio::time::sleep(d)
    }

    #[inline(always)]
    fn tick(d: Duration) -> Self::Interval {
        let later = tokio::time::Instant::now() + d;
        TokioInterval(tokio::time::interval_at(later, d))
    }
}

impl AsyncExec for TokioRT {
    /// Spawn a task in the background, returning a handle to await its result
    #[inline]
    fn spawn<F, R>(&self, f: F) -> impl AsyncJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        match self {
            Self::Runtime(s) => {
                return TokioJoinHandle(s.spawn(f));
            }
            Self::Handle(s) => {
                return TokioJoinHandle(s.spawn(f));
            }
        }
    }

    /// Spawn a task and detach it (no handle returned)
    #[inline]
    fn spawn_detach<F, R>(&self, f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        match self {
            Self::Runtime(s) => {
                s.spawn(f);
            }
            Self::Handle(s) => {
                s.spawn(f);
            }
        }
    }

    /// Run a future to completion on the runtime
    #[inline]
    fn block_on<F, R>(&self, f: F) -> R
    where
        F: Future<Output = R> + Send,
        R: Send + 'static,
    {
        match self {
            Self::Runtime(s) => {
                return s.block_on(f);
            }
            Self::Handle(s) => {
                return s.block_on(f);
            }
        }
    }
}

/// Associate type for TokioRT
pub struct TokioInterval(tokio::time::Interval);

impl TimeInterval for TokioInterval {
    #[inline]
    fn poll_tick(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Instant> {
        let _self = self.get_mut();
        if let Poll::Ready(i) = _self.0.poll_tick(ctx) {
            Poll::Ready(i.into_std())
        } else {
            Poll::Pending
        }
    }
}

/// Associate type for TokioRT
pub struct TokioFD<T: AsRawFd + AsFd + Send + Sync + 'static>(tokio::io::unix::AsyncFd<T>);

impl<T: AsRawFd + AsFd + Send + Sync + 'static> AsyncFd<T> for TokioFD<T> {
    #[inline(always)]
    async fn async_read<R>(&self, f: impl FnMut(&T) -> io::Result<R> + Send) -> io::Result<R> {
        self.0.async_io(tokio::io::Interest::READABLE, f).await
    }

    #[inline(always)]
    async fn async_write<R>(&self, f: impl FnMut(&T) -> io::Result<R> + Send) -> io::Result<R> {
        self.0.async_io(tokio::io::Interest::WRITABLE, f).await
    }
}

impl<T: AsRawFd + AsFd + Send + Sync + 'static> Deref for TokioFD<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0.get_ref()
    }
}

/// A wrapper around tokio's JoinHandle that implements AsyncJoinHandle
pub struct TokioJoinHandle<T>(tokio::task::JoinHandle<T>);

impl<T: Send + 'static> AsyncJoinHandle<T> for TokioJoinHandle<T> {
    #[inline]
    async fn join(self) -> Result<T, ()> {
        match self.0.await {
            Ok(r) => Ok(r),
            Err(_) => Err(()),
        }
    }

    #[inline]
    fn detach(self) {
        // Tokio's JoinHandle doesn't need explicit detach, it will run in background
        // when the handle is dropped
    }
}
