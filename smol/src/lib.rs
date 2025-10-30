//! # Smol Runtime adapter for Orb framework
//!
//! This crate provides a Smol-based implementation of the Orb async runtime traits.
//! It allows users to leverage Smol's lightweight async runtime with the unified Orb interface.
//!
//! The main type provided is [`SmolRT`], which implements the core runtime functionality.
//!
//! See the [Orb crate](https://docs.rs/orb) for more information.
//!
//! ## Features
//!
//! - `global`: Enables the global executor feature, which allows using a global executor
//!   instead of providing your own executor instance.
//!
//! ## Usage
//!
//! With a custom executor:
//!
//! ```rust
//! use orb_smol::SmolRT;
//! use std::sync::Arc;
//! use async_executor::Executor;
//!
//! let executor = Arc::new(Executor::new());
//! let rt = SmolRT::new(executor);
//! ```
//!
//! With the global executor (requires the `global` feature):
//!
//! ```rust
//! use orb_smol::SmolRT;
//!
//! #[cfg(feature = "global")]
//! let rt = SmolRT::new_global();
//! ```

use async_executor::Executor;
use async_io::{Async, Timer};
use futures_lite::future::block_on;
use futures_lite::stream::StreamExt;
use orb::io::{AsyncFd, AsyncIO};
use orb::runtime::{AsyncExec, AsyncJoinHandle};
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
use std::sync::Arc;
use std::task::*;
use std::time::{Duration, Instant};

/// The SmolRT implements AsyncRuntime trait
#[derive(Clone)]
pub struct SmolRT(Option<Arc<Executor<'static>>>);

impl fmt::Debug for SmolRT {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.is_some() { write!(f, "smol") } else { write!(f, "smol(global)") }
    }
}

impl SmolRT {
    #[cfg(feature = "global")]
    #[inline]
    pub fn new_global() -> Self {
        Self(None)
    }

    /// spawn coroutine with specified Executor
    #[inline]
    pub fn new(executor: Arc<Executor<'static>>) -> Self {
        Self(Some(executor))
    }
}

impl orb::AsyncRuntime for SmolRT {}

impl AsyncIO for SmolRT {
    type AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static> = SmolFD<T>;

    #[inline(always)]
    async fn connect_tcp(addr: &SocketAddr) -> io::Result<Self::AsyncFd<TcpStream>> {
        let _addr = addr.clone();
        let stream = Async::<TcpStream>::connect(_addr).await?;
        // into_inner will not change back to blocking
        Self::to_async_fd_rw(stream.into_inner()?)
    }

    #[inline(always)]
    async fn connect_unix(addr: &PathBuf) -> io::Result<Self::AsyncFd<UnixStream>> {
        let _addr = addr.clone();
        let stream = Async::<UnixStream>::connect(_addr).await?;
        // into_inner will not change back to blocking
        Self::to_async_fd_rw(stream.into_inner()?)
    }

    #[inline(always)]
    fn to_async_fd_rd<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        Ok(SmolFD(Async::new(fd)?))
    }

    #[inline(always)]
    fn to_async_fd_rw<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        Ok(SmolFD(Async::new(fd)?))
    }
}

impl AsyncTime for SmolRT {
    type Interval = SmolInterval;

    #[inline(always)]
    fn sleep(d: Duration) -> impl Future + Send {
        Timer::after(d)
    }

    #[inline(always)]
    fn tick(d: Duration) -> Self::Interval {
        let later = std::time::Instant::now() + d;
        SmolInterval(Timer::interval_at(later, d))
    }
}

/// AsyncJoinHandle implementation for smol
pub struct SmolJoinHandle<T>(async_executor::Task<T>);

impl<T: Send + 'static> AsyncJoinHandle<T> for SmolJoinHandle<T> {
    #[inline]
    async fn join(self) -> Result<T, ()> {
        Ok(self.0.await)
    }

    #[inline]
    fn detach(self) {
        self.0.detach();
    }
}

impl AsyncExec for SmolRT {
    /// Spawn a task in the background
    fn spawn<F, R>(&self, f: F) -> impl AsyncJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let handle = match &self.0 {
            Some(exec) => exec.spawn(f),
            None => {
                #[cfg(feature = "global")]
                {
                    smol::spawn(f)
                }
                #[cfg(not(feature = "global"))]
                unreachable!();
            }
        };
        SmolJoinHandle(handle)
    }

    /// Depends on how you initialize SmolRT, spawn with executor or globally
    #[inline]
    fn spawn_detach<F, R>(&self, f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        self.spawn(f).detach();
    }

    #[inline]
    fn spawn_blocking<F, R>(f: F) -> impl AsyncJoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        SmolJoinHandle(blocking::unblock(f))
    }

    /// Run a future to completion on the runtime
    ///
    /// NOTE: when initialized  with an executor,  will block current thread until the future
    /// returns
    #[inline]
    fn block_on<F, R>(&self, f: F) -> R
    where
        F: Future<Output = R> + Send,
        R: Send + 'static,
    {
        if let Some(exec) = &self.0 {
            block_on(exec.run(f))
        } else {
            #[cfg(feature = "global")]
            {
                smol::block_on(f)
            }
            #[cfg(not(feature = "global"))]
            unreachable!();
        }
    }
}

/// Associate type for SmolRT
pub struct SmolInterval(Timer);

impl TimeInterval for SmolInterval {
    #[inline]
    fn poll_tick(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Instant> {
        let _self = self.get_mut();
        match _self.0.poll_next(ctx) {
            Poll::Ready(Some(i)) => Poll::Ready(i),
            Poll::Ready(None) => unreachable!(),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Associate type for SmolRT
pub struct SmolFD<T: AsRawFd + AsFd + Send + Sync + 'static>(Async<T>);

impl<T: AsRawFd + AsFd + Send + Sync + 'static> AsyncFd<T> for SmolFD<T> {
    #[inline(always)]
    async fn async_read<R>(&self, f: impl FnMut(&T) -> io::Result<R> + Send) -> io::Result<R> {
        self.0.read_with(f).await
    }

    #[inline(always)]
    async fn async_write<R>(&self, f: impl FnMut(&T) -> io::Result<R> + Send) -> io::Result<R> {
        self.0.write_with(f).await
    }
}

impl<T: AsRawFd + AsFd + Send + Sync + 'static> Deref for SmolFD<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0.get_ref()
    }
}
