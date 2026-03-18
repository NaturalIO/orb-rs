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
//! - `global`: Enables the global executor feature, which allows using `smol` default global executor
//!   instead of providing your own executor instance. (by default not enabled, omit the `smol`
//!   dependency)
//!
//! - `unwind`: Use AssertUnwindSafe to capture panic inside the task, and return Err(()) to the
//! task join handle. (by default not enabled, panic terminates the program)
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
use crossfire::{MAsyncRx, mpmc, null::CloseHandle};
use futures_lite::{future::block_on, stream::StreamExt};
use orb::io::{AsyncFd, AsyncIO};
use orb::runtime::{AsyncExec, AsyncExecDyn, AsyncHandle, ThreadHandle};
use orb::time::{AsyncTime, TimeInterval};
use std::fmt;
use std::future::Future;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::num::NonZero;
use std::ops::Deref;
use std::os::{
    fd::{AsFd, AsRawFd},
    unix::net::UnixStream,
};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::*;
use std::thread;
use std::time::{Duration, Instant};

/// The SmolRT implements AsyncRuntime trait
#[derive(Clone)]
pub struct SmolRT(Option<SmolRTInner>);

#[derive(Clone)]
struct SmolRTInner {
    ex: Arc<Executor<'static>>,
    _close_h: Option<CloseHandle<mpmc::Null>>,
}

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
        Self(Some(SmolRTInner { ex: executor, _close_h: None }))
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
    async fn connect_unix(addr: &Path) -> io::Result<Self::AsyncFd<UnixStream>> {
        let stream = Async::<UnixStream>::connect(addr).await?;
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
    fn interval(d: Duration) -> Self::Interval {
        let later = std::time::Instant::now() + d;
        SmolInterval(Timer::interval_at(later, d))
    }
}

macro_rules! unwind_wrap {
    ($f: expr) => {{
        #[cfg(feature = "unwind")]
        {
            use futures_lite::future::FutureExt;
            std::panic::AssertUnwindSafe($f).catch_unwind()
        }
        #[cfg(not(feature = "unwind"))]
        $f
    }};
}

/// AsyncHandle implementation for smol
#[cfg(feature = "unwind")]
pub struct SmolJoinHandle<T>(
    Option<async_executor::Task<Result<T, Box<dyn std::any::Any + Send>>>>,
);
#[cfg(not(feature = "unwind"))]
pub struct SmolJoinHandle<T>(Option<async_executor::Task<T>>);

impl<T: Send> AsyncHandle<T> for SmolJoinHandle<T> {
    #[inline]
    fn is_finished(&self) -> bool {
        self.0.as_ref().unwrap().is_finished()
    }

    #[inline(always)]
    fn abort(self) {
        // do nothing, the inner task will be dropped
    }

    #[inline(always)]
    fn detach(mut self) {
        self.0.take().unwrap().detach();
    }

    #[inline(always)]
    fn abort_boxed(self: Box<Self>) {
        // do nothing, the inner task will be dropped
    }

    #[inline(always)]
    fn detach_boxed(mut self: Box<Self>) {
        self.0.take().unwrap().detach();
    }
}

impl<T> Future for SmolJoinHandle<T> {
    type Output = Result<T, ()>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _self = unsafe { self.get_unchecked_mut() };
        if let Some(inner) = _self.0.as_mut() {
            if let Poll::Ready(r) = Pin::new(inner).poll(cx) {
                #[cfg(feature = "unwind")]
                {
                    return Poll::Ready(r.map_err(|_e| ()));
                }
                #[cfg(not(feature = "unwind"))]
                {
                    return Poll::Ready(Ok(r));
                }
            }
            Poll::Pending
        } else {
            Poll::Ready(Err(()))
        }
    }
}

impl<T> Drop for SmolJoinHandle<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.detach();
        }
    }
}

pub struct BlockingJoinHandle<T>(async_executor::Task<T>);

impl<T> ThreadHandle<T> for BlockingJoinHandle<T> {
    #[inline]
    fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
}

impl<T> Future for BlockingJoinHandle<T> {
    type Output = Result<T, ()>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _self = unsafe { self.get_unchecked_mut() };
        if let Poll::Ready(r) = Pin::new(&mut _self.0).poll(cx) {
            return Poll::Ready(Ok(r));
        }
        Poll::Pending
    }
}

impl AsyncExecDyn for SmolRT {
    #[inline(always)]
    fn spawn_detach_dyn(&self, f: Box<dyn Future<Output = ()> + Send + Unpin>) {
        self.spawn(unwind_wrap!(f)).detach();
    }
}

impl AsyncExec for SmolRT {
    type AsyncHandle<R: Send> = SmolJoinHandle<R>;

    type ThreadHandle<R: Send> = BlockingJoinHandle<R>;

    #[inline(always)]
    fn one() -> Self {
        Self::multi(1)
    }

    #[inline(always)]
    fn multi(mut size: usize) -> Self {
        if size == 0 {
            size = usize::from(
                thread::available_parallelism().unwrap_or(NonZero::new(1usize).unwrap()),
            )
        }
        #[cfg(feature = "global")]
        {
            unsafe { std::env::set_var("SMOL_THREADS", size.to_string()) };
            Self(None)
        }
        #[cfg(not(feature = "global"))]
        {
            let (close_h, rx): (CloseHandle<mpmc::Null>, MAsyncRx<mpmc::Null>) = mpmc::new();
            // Prevent spawning another thread by running the process driver on this thread.
            let inner = SmolRTInner { ex: Arc::new(Executor::new()), _close_h: Some(close_h) };
            #[cfg(not(target_os = "espidf"))]
            inner.ex.spawn(async_process::driver()).detach();
            let ex = inner.ex.clone();
            for n in 1..=size {
                let _ex = ex.clone();
                let _rx = rx.clone();
                thread::Builder::new()
                    .name(format!("smol-{}", n))
                    .spawn(move || {
                        block_on(_ex.run(async move {
                            _rx.recv();
                            println!("thread exit");
                        }))
                    })
                    .expect("cannot spawn executor thread");
            }
            Self(Some(inner))
        }
    }

    /// Spawn a task in the background
    fn spawn<F, R>(&self, f: F) -> Self::AsyncHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        // Although SmolJoinHandle don't need Send marker, but here in the spawn()
        // need to restrict the requirements
        let handle = match &self.0 {
            Some(inner) => inner.ex.spawn(unwind_wrap!(f)),
            None => {
                #[cfg(feature = "global")]
                {
                    smol::spawn(unwind_wrap!(f))
                }
                #[cfg(not(feature = "global"))]
                unreachable!();
            }
        };
        SmolJoinHandle(Some(handle))
    }

    /// Depends on how you initialize SmolRT, spawn with executor or globally
    #[inline]
    fn spawn_detach<F, R>(&self, f: F)
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        self.spawn(unwind_wrap!(f)).detach();
    }

    #[inline]
    fn spawn_blocking<F, R>(f: F) -> Self::ThreadHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        BlockingJoinHandle(blocking::unblock(f))
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
        if let Some(inner) = &self.0 {
            block_on(inner.ex.run(f))
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
