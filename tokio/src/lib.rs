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

pub use orb::AsyncRuntime;
use orb::io::{AsyncFd, AsyncIO};
pub use orb::runtime::{AsyncExec, AsyncJoiner, ThreadJoiner};
use orb::time::{AsyncTime, TimeInterval};
use std::fmt;
use std::future::Future;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::ops::Deref;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::*;
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Handle, Runtime};

pub struct TokioRT {}

impl AsyncIO for TokioRT {
    type AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static> = TokioFD<T>;

    #[inline(always)]
    async fn connect_tcp(addr: &SocketAddr) -> io::Result<Self::AsyncFd<TcpStream>> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        // into_std will not change back to blocking
        Self::to_async_fd_rw(stream.into_std()?)
    }

    #[inline(always)]
    async fn connect_unix(addr: &Path) -> io::Result<Self::AsyncFd<UnixStream>> {
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
    fn interval(d: Duration) -> Self::Interval {
        let later = tokio::time::Instant::now() + d;
        TokioInterval(tokio::time::interval_at(later, d))
    }
}

impl AsyncRuntime for TokioRT {
    type Exec = TokioExec;

    /// Initiate executor using current thread.
    ///
    /// # Safety
    ///
    /// You should run [Self::block_on()] with this executor.
    ///
    /// If spawn without a `block_on()` running, it's possible
    /// the runtime just init future without scheduling.
    fn current() -> Self::Exec {
        TokioExec::new_current_thread()
    }

    /// Initiate executor with one background thread.
    ///
    /// # NOTE
    ///
    /// [Self::block_on()] is optional.
    #[inline(always)]
    fn one() -> Self::Exec {
        TokioExec::new_multi_thread(1)
    }

    /// Initiate executor with multiple background threads.
    ///
    /// # NOTE
    ///
    /// When `num` == 0, start threads that match cpu number
    /// [Self::block_on()] is optional.
    #[inline(always)]
    fn multi(num: usize) -> Self::Exec {
        TokioExec::new_multi_thread(num)
    }

    /// Spawn a task in the background, returning a handle to await its result
    #[inline]
    fn spawn<F, R>(f: F) -> TokioJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        // Although AsyncJoiner don't need Send marker, but here in the spawn()
        // need to restrict the requirements
        return TokioJoinHandle(tokio::spawn(f));
    }

    /// Spawn a task and detach it (no handle returned)
    #[inline]
    fn spawn_detach<F, R>(f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        tokio::spawn(f);
    }

    #[inline(always)]
    fn spawn_blocking<F, R>(f: F) -> TokioThreadHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        TokioThreadHandle(tokio::task::spawn_blocking(f))
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

/// A wrapper around tokio's JoinHandle that implements AsyncJoiner
pub struct TokioJoinHandle<T>(tokio::task::JoinHandle<T>);

impl<T: Send> AsyncJoiner<T> for TokioJoinHandle<T> {
    #[inline]
    fn is_finished(&self) -> bool {
        self.0.is_finished()
    }

    #[inline]
    fn detach(self) {
        // Tokio's JoinHandle doesn't need explicit detach, it will run in background
        // when the handle is dropped
    }

    #[inline]
    fn abort(self) {
        self.0.abort();
    }

    #[inline(always)]
    fn abort_boxed(self: Box<Self>) {
        self.0.abort();
    }

    #[inline(always)]
    fn detach_boxed(self: Box<Self>) {
        // Tokio's JoinHandle doesn't need explicit detach, it will run in background
        // when the handle is dropped
    }
}

impl<T> Future for TokioJoinHandle<T> {
    type Output = Result<T, ()>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _self = unsafe { self.get_unchecked_mut() };
        if let Poll::Ready(r) = Pin::new(&mut _self.0).poll(cx) {
            return Poll::Ready(r.map_err(|_e| ()));
        }
        Poll::Pending
    }
}

/// A wrapper around tokio's JoinHandle that implements ThreadJoiner
pub struct TokioThreadHandle<T>(tokio::task::JoinHandle<T>);

impl<T> ThreadJoiner<T> for TokioThreadHandle<T> {
    #[inline]
    fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
}

impl<T> Future for TokioThreadHandle<T> {
    type Output = Result<T, ()>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _self = unsafe { self.get_unchecked_mut() };
        if let Poll::Ready(r) = Pin::new(&mut _self.0).poll(cx) {
            return Poll::Ready(r.map_err(|_e| ()));
        }
        Poll::Pending
    }
}

/// The main struct for tokio runtime IO, assign this type to AsyncIO trait when used.
pub enum TokioExec {
    // Runtime don't have clone, since we don't have thread context, we need to put runtime to Arc
    // to impl Clone. (We usually need to clone self before calling block_on)
    Runtime(Arc<Runtime>),
    Handle(Handle),
}

impl Clone for TokioExec {
    /// Clone a TokioRT::Handle out of runtime, for spawn
    fn clone(&self) -> Self {
        match self {
            Self::Handle(h) => {
                return Self::Handle(h.clone());
            }
            Self::Runtime(rt) => Self::Runtime(rt.clone()),
        }
    }
}

impl TokioExec {
    /// Capture a runtime
    #[inline]
    pub fn new_with_runtime(rt: Runtime) -> Self {
        Self::Runtime(Arc::new(rt))
    }

    #[inline]
    pub fn new_multi_thread(workers: usize) -> Self {
        let mut builder = Builder::new_multi_thread();
        if workers > 0 {
            builder.worker_threads(workers);
        }
        let rt = builder.enable_all().build().unwrap();
        Self::Runtime(Arc::new(rt))
    }

    #[inline]
    pub fn new_current_thread() -> Self {
        let mut builder = Builder::new_current_thread();
        let rt = builder.enable_all().build().unwrap();
        Self::Runtime(Arc::new(rt))
    }

    /// Only capture a runtime handle. Should acquire with
    /// `async { Handle::current() }`
    #[inline]
    pub fn new_with_handle(handle: Handle) -> Self {
        Self::Handle(handle)
    }
}

impl AsyncExec for TokioExec {
    type AsyncJoiner<R: Send> = TokioJoinHandle<R>;

    type ThreadJoiner<R: Send> = TokioThreadHandle<R>;

    /// Spawn a task in the background, returning a handle to await its result
    #[inline]
    fn spawn<F, R>(&self, f: F) -> TokioJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        // Although AsyncJoiner don't need Send marker, but here in the spawn()
        // need to restrict the requirements
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

    #[inline(always)]
    fn spawn_blocking<F, R>(&self, f: F) -> TokioThreadHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        match self {
            Self::Runtime(s) => TokioThreadHandle(s.spawn_blocking(f)),
            Self::Handle(s) => TokioThreadHandle(s.spawn_blocking(f)),
        }
    }

    /// Run a future to completion on the runtime
    #[inline]
    fn block_on<F, R>(&self, f: F) -> R
    where
        F: Future<Output = R>,
        R: 'static,
    {
        match self {
            Self::Runtime(s) => {
                return s.block_on(f);
            }
            Self::Handle(_s) => {
                // panic in order to prevent misbehaved code.
                // refer to https://docs.rs/tokio/latest/tokio/runtime/struct.Handle.html#method.block_on
                panic!("handle is not allowed to block_on");
            }
        }
    }
}

impl fmt::Debug for TokioExec {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Runtime(_) => write!(f, "tokio(rt)"),
            Self::Handle(_) => write!(f, "tokio(handle)"),
        }
    }
}
