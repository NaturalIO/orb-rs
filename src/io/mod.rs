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
pub use buf_io::{AsyncBufRead, AsyncBufStream, AsyncBufWrite};

/// Helper macro to convert timeout errors to IO errors.
///
/// This macro is used internally to convert the `()` error returned by
/// timeout functions into a proper `io::Error` with `TimedOut` kind.
macro_rules! io_with_timeout {
    ($IO: path, $timeout: expr, $f: expr) => {{
        if $timeout == Duration::from_secs(0) {
            $f.await
        } else {
            // the crate reference make this macro not exportable
            match <$IO as crate::time::AsyncTime>::timeout($timeout, $f).await {
                Ok(Ok(r)) => Ok(r),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(io::ErrorKind::TimedOut.into()),
            }
        }
    }};
}
pub(super) use io_with_timeout;

/// Trait for async I/O operations.
///
/// This trait defines the interface for performing asynchronous I/O operations
/// such as connecting to network services and converting file descriptors to
/// async handles.
///
/// # Associated Types
///
/// * `AsyncFd` - The type used to represent async file descriptors
pub trait AsyncIO {
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

impl<F: std::ops::Deref<Target = IO>, IO: AsyncIO> AsyncIO for F {
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

/// AsyncRead trait for runtime adapter
pub trait AsyncRead: Send {
    /// Async version of read function
    ///
    /// On ok, return the bytes read
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = io::Result<usize>> + Send;

    /// Read the exact number of bytes required to fill `buf`.
    ///
    /// This function repeatedly calls `read` until the buffer is completely filled.
    ///
    /// # Errors
    ///
    /// This function will return an error if the stream is closed before the
    /// buffer is filled.
    fn read_exact<'a>(
        &'a mut self, mut buf: &'a mut [u8],
    ) -> impl Future<Output = io::Result<()>> + Send + 'a {
        async move {
            while !buf.is_empty() {
                match self.read(buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let tmp = buf;
                        buf = &mut tmp[n..];
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            if !buf.is_empty() {
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, "failed to fill whole buffer"))
            } else {
                Ok(())
            }
        }
    }

    /// Reads at least `min_len` bytes into `buf`.
    ///
    /// This function repeatedly calls `read` until at least `min_len` bytes have been
    /// read. It is allowed to read more than `min_len` bytes, but not more than
    /// the length of `buf`.
    ///
    /// # Returns
    ///
    /// On success, returns the total number of bytes read. This will be at least
    /// `min_len`, and could be more, up to the length of `buf`.
    ///
    /// # Errors
    ///
    /// It will return an `UnexpectedEof` error if the stream is closed before at least `min_len` bytes have been read.
    fn read_at_least<'a>(
        &'a mut self, buf: &'a mut [u8], min_len: usize,
    ) -> impl Future<Output = io::Result<usize>> + Send + 'a {
        async move {
            let mut total_read = 0;
            while total_read < min_len && total_read < buf.len() {
                match self.read(&mut buf[total_read..]).await {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "failed to read minimum number of bytes",
                        ));
                    }
                    Ok(n) => total_read += n,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
            }
            Ok(total_read)
        }
    }
}

/// AsyncWrite trait for runtime adapter
pub trait AsyncWrite: Send {
    /// Async version of write function
    ///
    /// On ok, return the bytes written
    fn write(&mut self, buf: &[u8]) -> impl Future<Output = io::Result<usize>> + Send;

    /// Write the entire buffer `buf`.
    ///
    /// This function repeatedly calls `write` until the entire buffer is written.
    ///
    /// # Errors
    ///
    /// This function will return an error if the stream is closed before the
    /// entire buffer is written.
    fn write_all<'a>(
        &'a mut self, mut buf: &'a [u8],
    ) -> impl Future<Output = io::Result<()>> + Send + 'a {
        async move {
            while !buf.is_empty() {
                match self.write(buf).await {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write whole buffer",
                        ));
                    }
                    Ok(n) => {
                        buf = &buf[n..];
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }
    }
}
