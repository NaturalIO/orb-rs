//! TCP and Unix domain socket listener implementations.
//!
//! This module provides async listener abstractions for TCP and Unix domain sockets,
//! implementing the [`AsyncListener`] trait for both types.

use super::{AsyncFd, AsyncIO, AsyncRead, AsyncWrite};
use std::fmt;
use std::io;
use std::net::TcpListener as StdTcpListener;
use std::net::{SocketAddr, TcpStream as StdTcpStream};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
use std::path::Path;

/// A TCP socket listener that implements AsyncListener.
pub struct TcpListener<IO: AsyncIO> {
    inner: IO::AsyncFd<StdTcpListener>,
}

/// A Unix domain socket listener that implements AsyncListener.
pub struct UnixListener<IO: AsyncIO> {
    inner: IO::AsyncFd<StdUnixListener>,
}

/// A TCP stream that implements AsyncRead and AsyncWrite.
pub struct TcpStream<IO: AsyncIO> {
    inner: IO::AsyncFd<StdTcpStream>,
}

/// A Unix stream that implements AsyncRead and AsyncWrite.
pub struct UnixStream<IO: AsyncIO> {
    inner: IO::AsyncFd<StdUnixStream>,
}

impl<IO: AsyncIO> TcpListener<IO> {
    /// Create a new TcpListener from a std TcpListener.
    fn from_std(listener: StdTcpListener) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let inner = IO::to_async_fd_rd(listener)?;
        Ok(TcpListener { inner })
    }

    /// Bind a TcpListener to the specified address.
    pub fn bind(addr: &str) -> io::Result<Self> {
        let socket_addr: SocketAddr = addr.parse().map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid address {}: {}", addr, e))
        })?;
        let listener = StdTcpListener::bind(socket_addr)?;
        Self::from_std(listener)
    }

    /// Accept a new connection.
    pub async fn accept(&mut self) -> io::Result<TcpStream<IO>> {
        match self.inner.async_read(|listener| listener.accept()).await {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to set non-blocking: {}", e),
                    )
                })?;
                let inner = IO::to_async_fd_rw(stream)?;
                Ok(TcpStream { inner })
            }
            Err(e) => Err(e),
        }
    }

    /// Get the local address of the listener.
    pub fn local_addr(&self) -> io::Result<String> {
        let addr = self.inner.local_addr()?;
        Ok(addr.to_string())
    }

    /// Try to recover a listener from RawFd.
    ///
    /// Will set listener to non_blocking to validate the fd.
    ///
    /// # Arguments
    ///
    /// * addr: the addr is for determine address type
    pub unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self> {
        let _ = addr; // addr is not used for TCP listeners
        let listener = unsafe { StdTcpListener::from_raw_fd(raw_fd) };
        // Validate the fd by setting it to non-blocking
        listener.set_nonblocking(true).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("Failed to set non-blocking: {}", e))
        })?;
        Self::from_std(listener)
    }
}

impl<IO: AsyncIO> UnixListener<IO> {
    /// Create a new UnixListener from a std UnixListener.
    fn from_std(listener: StdUnixListener) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let inner = IO::to_async_fd_rd(listener)?;
        Ok(UnixListener { inner })
    }

    /// Bind a UnixListener to the specified path.
    pub fn bind(addr: &str) -> io::Result<Self> {
        // Remove existing socket file if it exists
        let path = Path::new(addr);
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to remove existing socket file: {}", e),
                )
            })?;
        }

        let listener = StdUnixListener::bind(path)?;
        Self::from_std(listener)
    }

    /// Accept a new connection.
    pub async fn accept(&mut self) -> io::Result<UnixStream<IO>> {
        match self.inner.async_read(|listener| listener.accept()).await {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to set non-blocking: {}", e),
                    )
                })?;
                let inner = IO::to_async_fd_rw(stream)?;
                Ok(UnixStream { inner })
            }
            Err(e) => Err(e),
        }
    }

    /// Get the local address of the listener.
    pub fn local_addr(&self) -> io::Result<String> {
        let addr = self.inner.local_addr()?;
        Ok(addr
            .as_pathname()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No pathname for Unix socket"))?
            .to_string_lossy()
            .into_owned())
    }

    /// Try to recover a listener from RawFd.
    ///
    /// Will set listener to non_blocking to validate the fd.
    ///
    /// # Arguments
    ///
    /// * addr: the addr is for determine address type
    pub unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self> {
        let _ = addr; // addr is not used for Unix listeners
        let listener = unsafe { StdUnixListener::from_raw_fd(raw_fd) };
        // Validate the fd by setting it to non-blocking
        listener.set_nonblocking(true).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("Failed to set non-blocking: {}", e))
        })?;
        Self::from_std(listener)
    }
}

impl<IO: AsyncIO> fmt::Debug for TcpListener<IO> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.local_addr() {
            Ok(addr) => write!(f, "TcpListener({})", addr),
            Err(_) => write!(f, "TcpListener(unknown)"),
        }
    }
}

impl<IO: AsyncIO> fmt::Debug for UnixListener<IO> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.local_addr() {
            Ok(addr) => write!(f, "UnixListener({})", addr),
            Err(_) => write!(f, "UnixListener(unknown)"),
        }
    }
}

impl<IO: AsyncIO> AsRawFd for TcpListener<IO> {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl<IO: AsyncIO> AsRawFd for UnixListener<IO> {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

// Implement AsyncRead and AsyncWrite for TcpStream
impl<IO: AsyncIO> TcpStream<IO> {
    /// Connect to a TCP address asynchronously.
    ///
    /// This method attempts to establish a TCP connection to the specified
    /// address, returning a TcpStream that can be used for communication.
    ///
    /// # Parameters
    ///
    /// * `addr` - The socket address to connect to
    ///
    /// # Returns
    ///
    /// A future that resolves to a `Result` containing either the connected
    /// TcpStream or an I/O error.
    pub async fn connect(addr: &SocketAddr) -> io::Result<Self> {
        let stream = IO::connect_tcp(addr).await?;
        Ok(TcpStream { inner: stream })
    }
}

impl<IO: AsyncIO> AsyncRead for TcpStream<IO> {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use std::io::Read;
        self.inner.async_read(|mut stream| stream.read(buf)).await
    }
}

impl<IO: AsyncIO> AsyncWrite for TcpStream<IO> {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use std::io::Write;
        self.inner.async_write(|mut stream| stream.write(buf)).await
    }
}

// Implement AsyncRead and AsyncWrite for UnixStream
impl<IO: AsyncIO> UnixStream<IO> {
    /// Connect to a Unix socket address asynchronously.
    ///
    /// This method attempts to establish a Unix socket connection to the
    /// specified path, returning a UnixStream that can be used for communication.
    ///
    /// # Parameters
    ///
    /// * `addr` - The path to the Unix socket
    ///
    /// # Returns
    ///
    /// A future that resolves to a `Result` containing either the connected
    /// UnixStream or an I/O error.
    pub async fn connect(addr: &Path) -> io::Result<Self> {
        let path_buf = addr.to_path_buf();
        let stream = IO::connect_unix(&path_buf).await?;
        Ok(UnixStream { inner: stream })
    }
}

impl<IO: AsyncIO> AsyncRead for UnixStream<IO> {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use std::io::Read;
        self.inner.async_read(|mut stream| stream.read(buf)).await
    }
}

impl<IO: AsyncIO> AsyncWrite for UnixStream<IO> {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use std::io::Write;
        self.inner.async_write(|mut stream| stream.write(buf)).await
    }
}

impl<IO: AsyncIO> fmt::Debug for TcpStream<IO> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TcpStream")
    }
}

impl<IO: AsyncIO> fmt::Debug for UnixStream<IO> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "UnixStream")
    }
}

/// Create a TCP listener bound to the specified address.
pub fn listen_tcp<IO: AsyncIO>(addr: &str) -> io::Result<TcpListener<IO>> {
    TcpListener::bind(addr)
}

/// Create a Unix domain socket listener bound to the specified path.
pub fn listen_unix<IO: AsyncIO>(path: &str) -> io::Result<UnixListener<IO>> {
    UnixListener::bind(path)
}

/// Trait for async listener operations.
pub trait AsyncListener: Send + Sized + 'static + fmt::Debug {
    type Conn: Send + 'static + Sized;

    fn bind(addr: &str) -> io::Result<Self>;

    fn accept(&mut self) -> impl Future<Output = io::Result<Self::Conn>> + Send;

    fn local_addr(&self) -> io::Result<String>;

    /// Try to recover a listener from RawFd
    ///
    /// Will set listener to non_blocking to validate the fd
    ///
    /// # Arguments
    ///
    /// * addr: the addr is for determine address type
    unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self>
    where
        Self: AsRawFd;
}

// Implement AsyncListener for TcpListener using the public methods
impl<IO: AsyncIO> AsyncListener for TcpListener<IO> {
    type Conn = TcpStream<IO>;

    fn bind(addr: &str) -> io::Result<Self> {
        TcpListener::bind(addr)
    }

    fn accept(&mut self) -> impl Future<Output = io::Result<Self::Conn>> + Send {
        TcpListener::accept(self)
    }

    fn local_addr(&self) -> io::Result<String> {
        TcpListener::local_addr(self)
    }

    unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self>
    where
        Self: AsRawFd,
    {
        unsafe { TcpListener::try_from_raw_fd(addr, raw_fd) }
    }
}

// Implement AsyncListener for UnixListener using the public methods
impl<IO: AsyncIO> AsyncListener for UnixListener<IO> {
    type Conn = UnixStream<IO>;

    fn bind(addr: &str) -> io::Result<Self> {
        UnixListener::bind(addr)
    }

    fn accept(&mut self) -> impl Future<Output = io::Result<Self::Conn>> + Send {
        UnixListener::accept(self)
    }

    fn local_addr(&self) -> io::Result<String> {
        UnixListener::local_addr(self)
    }

    unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self>
    where
        Self: AsRawFd,
    {
        unsafe { UnixListener::try_from_raw_fd(addr, raw_fd) }
    }
}
