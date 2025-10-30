//! Asynchronous I/O traits and utilities.
//!
//! This module provides traits for performing asynchronous I/O operations
//! in a runtime-agnostic way. It includes functionality for connecting to
//! network services, working with file descriptors, and performing async
//! read/write operations.
//!
//! Further more, we have abstract buffered I/O  with [AsyncBufRead], [AsyncBufWrite], and [AsyncBufStream]
//!
//! # Design Notes
//!
//! We choose to provide `async fn` style IO function instead of `poll_xxx` style functions, because:
//!
//! - `async-io` crate don't have `poll_xxx` interfaces
//! - `poll_xxx` functions is pre-async-await stuff and difficult to use.
//! - you can always make an async fn with `poll_xxx`
//!
//! We choose to abstract [AsyncFd] instead of stream, because:
//! - All async stream can be converted between std version of stream
//! - All types of files/streams and be converted between OS raw fd.
//! - There's slight difference between tokio stream and async-io counterparts.
//! - What we do here is just wrap any std blocking function with async poller when they are
//! readable or writeable, similar with `async-io`, as a light-weight implementation.

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::ops::Deref;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

mod buf_io;
pub use buf_io::{AsyncBufRead, AsyncBufStream, AsyncBufWrite, AsyncRead, AsyncWrite};

/// Helper macro to convert timeout errors to IO errors.
///
/// This macro is used internally to convert the `()` error returned by
/// timeout functions into a proper `io::Error` with `TimedOut` kind.
#[macro_export]
macro_rules! io_with_timeout {
    ($IO: path, $timeout: expr, $f: expr) => {{
        if $timeout == Duration::from_secs(0) {
            $f.await
        } else {
            // rust 2018 macro will replace crate name after export
            match <$IO as crate::time::AsyncTime>::timeout($timeout, $f).await {
                Ok(Ok(r)) => Ok(r),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(io::ErrorKind::TimedOut.into()),
            }
        }
    }};
}

/// Trait for async I/O operations.
///
/// This trait defines the interface for performing asynchronous I/O operations
/// such as connecting to network services and converting file descriptors to
/// async handles.
///
/// # Associated Types
///
/// * `AsyncFd` - The type used to represent async file descriptors
pub trait AsyncIO: Send + Sync + 'static {
    /// The type used to represent async file descriptors.
    ///
    /// This associated type represents a wrapper around a file descriptor
    /// that provides async read/write operations.
    type AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static>: AsyncFd<T>;

    /// Connect to a TCP address asynchronously.
    ///
    /// # NOTE
    ///
    /// This is for runtime implementation, for user should use [`TcpStream::<IO>::connect()`](crate::net::TcpStream) instead**.
    ///
    /// This method attempts to establish a TCP connection to the specified
    /// address, returning an async file descriptor that can be used for
    /// communication.
    ///
    /// # Parameters
    ///
    /// * `addr` - The socket address to connect to
    ///
    /// # Returns
    ///
    /// A future that resolves to a `Result` containing either the connected
    /// async file descriptor or an I/O error.
    fn connect_tcp(
        addr: &SocketAddr,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<TcpStream>>> + Send;

    /// Connect to a Unix socket address asynchronously.
    ///
    /// # NOTE
    ///
    /// This is for runtime implementation, for user should use [`UnixStream::<IO>::connect()`](crate::net::UnixStream) instead**.
    ///
    /// This method attempts to establish a Unix socket connection to the
    /// specified path, returning an async file descriptor that can be used
    /// for communication.
    ///
    /// # Parameters
    ///
    /// * `addr` - The path to the Unix socket
    ///
    /// # Returns
    ///
    /// A future that resolves to a `Result` containing either the connected
    /// async file descriptor or an I/O error.
    fn connect_unix(
        addr: &PathBuf,
    ) -> impl Future<Output = io::Result<Self::AsyncFd<UnixStream>>> + Send;

    /// Wrap a readable file object as an async handle
    ///
    /// The file descriptor will subscribe for read
    /// to the runtime poller
    ///
    /// # Parameters
    ///
    /// * `fd` - The file descriptor to wrap
    ///
    /// # Returns
    ///
    /// A `Result` containing either the async file descriptor handle or
    /// an I/O error.
    ///
    /// # Safety
    ///
    /// The file descriptor must be set to non-blocking mode before calling
    /// this method.
    fn to_async_fd_rd<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>>;

    /// Wrap a readable/writable file object as an async handle.
    ///
    /// The file descriptor will subscribe for read + write
    /// to the runtime poller
    ///
    /// # Parameters
    ///
    /// * `fd` - The file descriptor to wrap
    ///
    /// # Returns
    ///
    /// A `Result` containing either the async file descriptor handle or
    /// an I/O error.
    ///
    /// # Safety
    ///
    /// The file descriptor must be set to non-blocking mode before calling
    /// this method.
    fn to_async_fd_rw<T: AsRawFd + AsFd + Send + Sync + 'static>(
        fd: T,
    ) -> io::Result<Self::AsyncFd<T>>;
}

/// Trait for async file descriptor operations.
///
/// This trait provides methods for performing async read and write operations
/// on file descriptors.
///
/// # Type Parameters
///
/// * `T` - The underlying file descriptor type
pub trait AsyncFd<T: AsRawFd + AsFd + Send + Sync + 'static>:
    Send + Sync + 'static + Deref<Target = T>
{
    /// Perform an async read operation.
    ///
    /// This method executes the provided closure asynchronously, allowing
    /// it to perform read operations on the underlying file descriptor.
    ///
    /// # Parameters
    ///
    /// * `f` - A closure that performs the actual read operation
    ///
    /// # Returns
    ///
    /// A future that resolves to the result of the read operation.
    fn async_read<R>(
        &self, f: impl FnMut(&T) -> io::Result<R> + Send,
    ) -> impl Future<Output = io::Result<R>> + Send;

    /// Perform an async write operation.
    ///
    /// This method executes the provided closure asynchronously, allowing
    /// it to perform write operations on the underlying file descriptor.
    ///
    /// # Parameters
    ///
    /// * `f` - A closure that performs the actual write operation
    ///
    /// # Returns
    ///
    /// A future that resolves to the result of the write operation.
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
