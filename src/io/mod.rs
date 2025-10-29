//! I/O utilities

use super::time::AsyncTime;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::ops::Deref;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

mod buffer;
pub use buffer::AllocateBuf;
mod buf_io;
pub use buf_io::{AsyncBufRead, AsyncBufStream, AsyncBufWrite, AsyncRead, AsyncWrite};

/// Because timeout function return () as error, this macro convert to io::Error
macro_rules! io_with_timeout {
    ($IO: path, $timeout: expr, $f: expr) => {{
        if $timeout == Duration::from_secs(0) {
            $f.await
        } else {
            match <$IO as AsyncTime>::timeout($timeout, $f).await {
                Ok(Ok(r)) => Ok(r),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(io::ErrorKind::TimedOut.into()),
            }
        }
    }};
}

/// Defines the I/O interface we used from async runtime
pub trait AsyncIO: Send + Sync + 'static {
    type AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static>: AsyncFd<T>;

    fn connect_tcp(
        addr: &SocketAddr,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<TcpStream>>> + Send;

    fn connect_unix(
        addr: &PathBuf,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<UnixStream>>> + Send;

    #[inline]
    fn connect_tcp_timeout(
        addr: &SocketAddr, timeout: Duration,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<TcpStream>>> + Send
    where
        Self: AsyncTime,
    {
        async move { io_with_timeout!(Self, timeout, Self::connect_tcp(addr)) }
    }

    #[inline]
    fn connect_unix_timeout(
        addr: &PathBuf, timeout: Duration,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<UnixStream>>>
    where
        Self: AsyncTime,
    {
        async move { io_with_timeout!(Self, timeout, Self::connect_unix(addr)) }
    }

    /// Required to set_nonblocking first
    fn to_async_fd_rd<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>>;

    /// Required to set_nonblocking first
    fn to_async_fd_rw<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>>;
}

/// The trait of async fd to turn sync I/O to async
pub trait AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static>:
    Send + Sync + 'static + Deref<Target = T>
{
    fn async_read<R>(
        &self, f: impl FnMut(&T) -> io::Result<R> + Send,
    ) -> impl Future<Output = io::Result<R>> + Send;

    fn async_write<R>(
        &self, f: impl FnMut(&T) -> io::Result<R> + Send,
    ) -> impl Future<Output = io::Result<R>> + Send;
}

impl<F: std::ops::Deref<Target = IO> + Send + Sync + 'static, IO: AsyncIO> AsyncIO for F {
    type AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static> = IO::AsyncFd<T>;

    fn connect_tcp(
        addr: &SocketAddr,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<TcpStream>>> + Send {
        IO::connect_tcp(addr)
    }

    fn connect_unix(
        addr: &PathBuf,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<UnixStream>>> + Send {
        IO::connect_unix(addr)
    }

    fn to_async_fd_rd<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        IO::to_async_fd_rd(fd)
    }

    fn to_async_fd_rw<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>> {
        IO::to_async_fd_rw(fd)
    }
}
