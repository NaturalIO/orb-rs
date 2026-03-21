//! # Orb Compio Runtime
//!
//! This crate provides a Compio-based implementation of the Orb async runtime traits.
//! It allows users to leverage Compio's completion-based async runtime with the unified Orb interface.
//!
//! The main type provided is [`CompioRT`], which implements the core runtime functionality.
//!
//! See the [main Orb documentation](https://github.com/NaturalIO/orb) for more information.
//!
//! ## Usage
//!
//! With multi thread runtime using compio dispatcher:
//!
//! ```rust
//! use orb_compio::CompioRT;
//! use orb::prelude::*;
//!
//! type RT = CompioRT;
//!
//! let rt = RT::multi(4);
//! ```
//!
//! With single thread runtime:
//!
//! ```rust
//! use orb_compio::CompioRT;
//! use orb::prelude::*;
//!
//! type RT = CompioRT;
//!
//! let rt = RT::one();
//! ```
//!
//! ## Features
//!
//! - `io-uring` (default): Use io-uring for async I/O on Linux
//! - `polling`: Use polling-based async I/O (fallback for non-Linux platforms)

pub use orb::AsyncRuntime;
use orb::io::{AsyncFd, AsyncIO};
pub use orb::runtime::{AsyncExec, AsyncJoiner, ThreadJoiner};
use orb::time::{AsyncTime, TimeInterval};
use std::fmt;
use std::future::Future;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::*;
use std::time::{Duration, Instant};

use compio::dispatcher::Dispatcher;
use compio::runtime::Task;

pub struct CompioRT {}

/// The CompioRT implements AsyncRuntime trait
#[derive(Clone)]
pub struct CompioExec {
    inner: CompioExecInner,
}

#[derive(Clone)]
enum CompioExecInner {
    /// Single-threaded runtime (for current() and one())
    Single,
    /// Multi-threaded dispatcher (for multi())
    Dispatcher(Arc<Dispatcher>),
}

/// Join handle for async tasks
pub struct CompioJoinHandle<T>(Option<Task<T>>);

/// Join handle for blocking tasks
pub struct CompioThreadHandle<T>(Option<Task<std::thread::Result<T>>>);

/// Interval type for Compio
pub struct CompioInterval {
    interval: Duration,
    next_tick: Instant,
}

/// AsyncFd wrapper for Compio
pub struct CompioFD<T: AsRawFd + AsFd + Send + Sync + 'static> {
    fd: T,
}

impl AsyncIO for CompioRT {
    type AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static> = CompioFD<T>;

    #[inline(always)]
    async fn connect_tcp(addr: &SocketAddr) -> io::Result<Self::AsyncFd<TcpStream>> {
        let addr = *addr;
        let stream = std::net::TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        Self::to_async_fd_rw(stream)
    }

    #[inline(always)]
    async fn connect_unix(addr: &Path) -> io::Result<Self::AsyncFd<UnixStream>> {
        let path = addr.to_path_buf();
        let stream = std::os::unix::net::UnixStream::connect(path)?;
        stream.set_nonblocking(true)?;
        Self::to_async_fd_rw(stream)
    }

    #[inline(always)]
    fn to_async_fd_rd<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        Ok(CompioFD { fd })
    }

    #[inline(always)]
    fn to_async_fd_rw<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        Ok(CompioFD { fd })
    }
}

impl<T: AsRawFd + AsFd + Send + Sync + 'static> AsyncFd<T> for CompioFD<T> {
    async fn async_read<R>(&self, mut f: impl FnMut(&T) -> io::Result<R> + Send) -> io::Result<R> {
        loop {
            match f(&self.fd) {
                Ok(r) => return Ok(r),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    futures_lite::future::yield_now().await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn async_write<R>(&self, mut f: impl FnMut(&T) -> io::Result<R> + Send) -> io::Result<R> {
        loop {
            match f(&self.fd) {
                Ok(r) => return Ok(r),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    futures_lite::future::yield_now().await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl<T: AsRawFd + AsFd + Send + Sync + 'static> Deref for CompioFD<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.fd
    }
}

impl AsyncTime for CompioRT {
    type Interval = CompioInterval;

    #[inline(always)]
    fn sleep(d: Duration) -> impl Future<Output = ()> + Send {
        async move {
            compio::time::sleep(d).await;
        }
    }

    #[inline(always)]
    fn interval(d: Duration) -> Self::Interval {
        CompioInterval { interval: d, next_tick: Instant::now() + d }
    }
}

impl TimeInterval for CompioInterval {
    fn poll_tick(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Instant> {
        let this = self.get_mut();
        let now = Instant::now();
        if now >= this.next_tick {
            let tick_time = this.next_tick;
            this.next_tick = now + this.interval;
            Poll::Ready(tick_time)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl AsyncRuntime for CompioRT {
    type Exec = CompioExec;

    /// Initiate executor using current thread.
    ///
    /// # Safety
    ///
    /// You should run [AsyncExec::block_on()] with this executor.
    ///
    /// If spawn without a `block_on()` running, it's possible
    /// the runtime just init future without scheduling.
    fn current() -> Self::Exec {
        CompioExec { inner: CompioExecInner::Single }
    }

    /// Initiate executor with one background thread.
    ///
    /// # NOTE
    ///
    /// [AsyncExec::block_on()] is optional, you can directly call [AsyncExec::spawn] with it.
    #[inline(always)]
    fn one() -> Self::Exec {
        Self::multi(1)
    }

    /// Initiate executor with multiple background threads.
    ///
    /// # NOTE
    ///
    /// When `num` == 0, start threads that match cpu number.
    ///
    /// [AsyncExec::block_on()] is optional, you can directly call [AsyncExec::spawn] with it.
    #[inline(always)]
    fn multi(num: usize) -> Self::Exec {
        let builder = Dispatcher::builder();
        let builder = if num > 0 {
            builder.worker_threads(NonZeroUsize::new(num).unwrap_or(NonZeroUsize::new(1).unwrap()))
        } else {
            builder
        };
        let dispatcher = builder.build().expect("Failed to create dispatcher");
        CompioExec { inner: CompioExecInner::Dispatcher(Arc::new(dispatcher)) }
    }

    /// Spawn a task in the background, returning a handle to await its result
    #[inline]
    fn spawn<F, R>(f: F) -> CompioJoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let task = compio::runtime::spawn(f);
        CompioJoinHandle(Some(task))
    }

    /// Spawn a task and detach it (no handle returned)
    #[inline]
    fn spawn_detach<F, R>(f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        compio::runtime::spawn(f).detach();
    }

    #[inline(always)]
    fn spawn_blocking<F, R>(f: F) -> CompioThreadHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let task = compio::runtime::spawn_blocking(f);
        CompioThreadHandle(Some(task))
    }
}

impl<T: Send> AsyncJoiner<T> for CompioJoinHandle<T> {
    #[inline]
    fn is_finished(&self) -> bool {
        self.0.as_ref().map_or(true, |t| t.is_finished())
    }

    #[inline]
    fn detach(self) {
        // Task is detached when dropped without awaiting
    }

    #[inline]
    fn abort(self) {
        // compio tasks don't have explicit abort, just drop
    }

    #[inline(always)]
    fn abort_boxed(self: Box<Self>) {
        // compio tasks don't have explicit abort
    }

    #[inline(always)]
    fn detach_boxed(self: Box<Self>) {
        // Task is detached when dropped
    }
}

impl<T: Send> Future for CompioJoinHandle<T> {
    type Output = Result<T, ()>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(ref mut task) = this.0 {
            match Pin::new(task).poll(cx) {
                Poll::Ready(r) => Poll::Ready(Ok(r)),
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(Err(()))
        }
    }
}

impl<T> Drop for CompioJoinHandle<T> {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.detach();
        }
    }
}

impl<T: Send> ThreadJoiner<T> for CompioThreadHandle<T> {
    #[inline]
    fn is_finished(&self) -> bool {
        self.0.as_ref().map_or(true, |t| t.is_finished())
    }
}

impl<T: Send> Future for CompioThreadHandle<T> {
    type Output = Result<T, ()>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(ref mut task) = this.0 {
            match Pin::new(task).poll(cx) {
                Poll::Ready(Ok(r)) => Poll::Ready(Ok(r)),
                Poll::Ready(Err(_)) => Poll::Ready(Err(())),
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(Err(()))
        }
    }
}

impl AsyncExec for CompioExec {
    type AsyncJoiner<R: Send> = CompioJoinHandle<R>;
    type ThreadJoiner<R: Send> = CompioThreadHandle<R>;

    fn spawn<F, R>(&self, f: F) -> Self::AsyncJoiner<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        match &self.inner {
            CompioExecInner::Single => {
                let task = compio::runtime::spawn(f);
                CompioJoinHandle(Some(task))
            }
            CompioExecInner::Dispatcher(dispatcher) => {
                let rx = dispatcher.dispatch(move || f);
                let task =
                    compio::runtime::spawn(
                        async move { rx.await.expect("Dispatcher task failed") },
                    );
                CompioJoinHandle(Some(task))
            }
        }
    }

    fn spawn_detach<F, R>(&self, f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        match &self.inner {
            CompioExecInner::Single => {
                compio::runtime::spawn(f).detach();
            }
            CompioExecInner::Dispatcher(dispatcher) => {
                let _ = dispatcher.dispatch(|| f);
            }
        }
    }

    fn spawn_blocking<F, R>(&self, f: F) -> Self::ThreadJoiner<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        // Both Single and Dispatcher use compio::runtime::spawn_blocking
        // which works globally
        let task = compio::runtime::spawn_blocking(f);
        CompioThreadHandle(Some(task))
    }

    fn block_on<F, R>(&self, f: F) -> R
    where
        F: Future<Output = R> + Send,
        R: 'static,
    {
        futures_lite::future::block_on(f)
    }
}

impl fmt::Debug for CompioExec {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.inner {
            CompioExecInner::Single => write!(f, "compio(single)"),
            CompioExecInner::Dispatcher(_) => write!(f, "compio(dispatcher)"),
        }
    }
}
