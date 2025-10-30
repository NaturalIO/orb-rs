//! TCP and Unix domain socket listener implementations.
//!
//! This module provides async listener abstractions for TCP and Unix domain sockets.
//!
//! Additionally, we provides:
//! - [UnifyAddr] type for smart address parsing, and trait [ResolveAddr]
//! to replace std [ToSocketAddrs](https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html),
//! - [UnifyStream] + [UnixListener] to provide consistent interface for both socket types.

use super::{AsyncFd, AsyncIO, AsyncRead, AsyncWrite};
use crate::time::AsyncTime;
use std::fmt;
use std::io;
use std::net::{
    AddrParseError, IpAddr, SocketAddr, SocketAddrV4, SocketAddrV6, TcpListener as StdTcpListener,
    TcpStream as StdTcpStream, ToSocketAddrs,
};
use std::time::Duration;

/// Helper macro to convert timeout errors to IO errors.
///
/// This macro is used internally to convert the `()` error returned by
/// timeout functions into a proper `io::Error` with `TimedOut` kind.
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
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
use std::path::{Path, PathBuf};
use std::str::FromStr;

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
    pub fn from_std(listener: StdTcpListener) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let inner = IO::to_async_fd_rd(listener)?;
        Ok(TcpListener { inner })
    }

    /// Bind a TcpListener to the specified address.
    pub async fn bind<A: ResolveAddr + ?Sized>(addr: &A) -> io::Result<Self> {
        // generic params are Sized by default, while str is ?Sized
        match addr.resolve() {
            Ok(UnifyAddr::Socket(_addr)) => {
                let listener = StdTcpListener::bind(&_addr)?;
                Self::from_std(listener)
            }
            Ok(UnifyAddr::Path(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("addr {:?} invalid", addr),
                ));
            }
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("addr {:?} invalid: {:?}", addr, e),
                ));
            }
        }
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
    pub fn from_std(listener: StdUnixListener) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let inner = IO::to_async_fd_rd(listener)?;
        Ok(UnixListener { inner })
    }

    /// Bind a UnixListener to the specified path.
    pub fn bind<P: AsRef<Path>>(p: P) -> io::Result<Self> {
        let listener = StdUnixListener::bind(p)?;
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
    pub async fn connect<A: ResolveAddr + ?Sized>(addr: &A) -> io::Result<Self> {
        // generic params are Sized by default, while str is ?Sized
        match addr.resolve() {
            Ok(UnifyAddr::Socket(socket_addr)) => {
                let stream = IO::connect_tcp(&socket_addr).await?;
                Ok(TcpStream { inner: stream })
            }
            Err(e) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("addr {:?} invalid: {:?}", addr, e),
            )),
            Ok(UnifyAddr::Path(_)) => {
                Err(io::Error::new(io::ErrorKind::Other, format!("addr {:?} invalid", addr)))
            }
        }
    }

    /// Connect to a TCP address asynchronously with a timeout.
    ///
    /// This method attempts to establish a TCP connection to the specified
    /// address, returning a TcpStream that can be used for communication.
    /// If the connection attempt takes longer than the specified timeout,
    /// an error will be returned.
    ///
    /// # Parameters
    ///
    /// * `addr` - The socket address to connect to
    /// * `timeout` - The maximum time to wait for the connection
    ///
    /// # Returns
    ///
    /// A future that resolves to a `Result` containing either the connected
    /// TcpStream or an I/O error.
    pub async fn connect_timeout<A>(addr: &A, timeout: std::time::Duration) -> io::Result<Self>
    where
        IO: AsyncTime,
        A: ResolveAddr + ?Sized,
    {
        // generic params are Sized by default, while str is ?Sized
        io_with_timeout!(IO, timeout, Self::connect::<A>(addr))
    }

    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
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
    pub async fn connect<P: AsRef<Path>>(addr: P) -> io::Result<Self> {
        let path_buf = addr.as_ref().to_path_buf();
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

/// Trait for async listener operations.
pub trait AsyncListener: Send + Sized + 'static + fmt::Debug {
    type Conn: Send + 'static + Sized;

    fn bind(addr: &str) -> impl Future<Output = io::Result<Self>> + Send;

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

impl<IO: AsyncIO> AsyncListener for TcpListener<IO> {
    type Conn = TcpStream<IO>;

    #[inline]
    async fn bind(addr: &str) -> io::Result<Self> {
        TcpListener::<IO>::bind(addr).await
    }

    #[inline(always)]
    fn accept(&mut self) -> impl Future<Output = io::Result<Self::Conn>> + Send {
        TcpListener::<IO>::accept(self)
    }

    #[inline(always)]
    fn local_addr(&self) -> io::Result<String> {
        TcpListener::<IO>::local_addr(self)
    }

    #[inline(always)]
    unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self>
    where
        Self: AsRawFd,
    {
        unsafe { TcpListener::try_from_raw_fd(addr, raw_fd) }
    }
}

impl<IO: AsyncIO> AsyncListener for UnixListener<IO> {
    type Conn = UnixStream<IO>;

    #[inline]
    async fn bind(addr: &str) -> io::Result<Self> {
        UnixListener::<IO>::bind(addr)
    }

    #[inline(always)]
    fn accept(&mut self) -> impl Future<Output = io::Result<Self::Conn>> + Send {
        UnixListener::<IO>::accept(self)
    }

    #[inline(always)]
    fn local_addr(&self) -> io::Result<String> {
        UnixListener::<IO>::local_addr(self)
    }

    #[inline(always)]
    unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self>
    where
        Self: AsRawFd,
    {
        unsafe { UnixListener::try_from_raw_fd(addr, raw_fd) }
    }
}

/// Unify behavior of tcp & unix addr
#[derive(Clone, PartialEq, Eq)]
pub enum UnifyAddr {
    /// SocketAddr
    Socket(SocketAddr),
    Path(std::path::PathBuf),
}

macro_rules! from_sockaddr {
    ($t: tt) => {
        impl From<$t> for UnifyAddr {
            #[inline]
            fn from(addr: $t) -> Self {
                Self::Socket(addr.into())
            }
        }
    };
}

from_sockaddr!(SocketAddr);
from_sockaddr!(SocketAddrV4);
from_sockaddr!(SocketAddrV6);

impl<I: Into<IpAddr>> From<(I, u16)> for UnifyAddr {
    #[inline]
    fn from(addr: (I, u16)) -> Self {
        Self::Socket(addr.into())
    }
}

impl From<PathBuf> for UnifyAddr {
    #[inline]
    fn from(addr: PathBuf) -> Self {
        Self::Path(addr)
    }
}

impl UnifyAddr {
    #[inline]
    pub fn parse(s: &str) -> Result<Self, AddrParseError> {
        if s.as_bytes()[0] as char == '/' {
            return Ok(Self::Path(std::path::PathBuf::from(s)));
        }
        let a = s.parse::<SocketAddr>()?;
        Ok(Self::Socket(a))
    }

    pub fn resolve(s: &str) -> Result<Self, AddrParseError> {
        // TODO change this to async
        match Self::parse(s) {
            Ok(addr) => return Ok(addr),
            Err(e) => match s.to_socket_addrs() {
                Ok(mut _v) => match _v.next() {
                    Some(a) => Ok(Self::Socket(a)),
                    None => Err(e),
                },
                Err(_) => Err(e),
            },
        }
    }
}

/// Resolve addr in async to one address for listen or connect
///
/// # NOTE:
///
/// When we can't directly resolve the IP, try to resolve it through the domain name with
/// background spawn thread, will not block current thread.
///
/// If multiple IP addresses are resolved, only the first result is taken
pub trait ResolveAddr: fmt::Debug + Send + Sync {
    // Trait are ?Sized by default
    fn resolve(&self) -> Result<UnifyAddr, AddrParseError>;
}

impl ResolveAddr for str {
    #[inline]
    fn resolve(&self) -> Result<UnifyAddr, AddrParseError> {
        return UnifyAddr::resolve(self);
    }
}

impl ResolveAddr for String {
    #[inline]
    fn resolve(&self) -> Result<UnifyAddr, AddrParseError> {
        return UnifyAddr::resolve(self.as_str());
    }
}

impl<T: Into<UnifyAddr> + Clone + Send + Sync + fmt::Debug> ResolveAddr for T {
    #[inline]
    fn resolve(&self) -> Result<UnifyAddr, AddrParseError> {
        Ok(self.clone().into())
    }
}

impl fmt::Display for UnifyAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Socket(s) => write!(f, "{}", s),
            Self::Path(p) => write!(f, "{}", p.display()),
        }
    }
}

impl fmt::Debug for UnifyAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Socket(s) => write!(f, "path {}", s),
            Self::Path(p) => write!(f, "sock addr {}", p.display()),
        }
    }
}

impl ToSocketAddrs for UnifyAddr {
    type Iter = std::vec::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
        match self {
            Self::Socket(addr) => Ok(vec![*addr].into_iter()),
            Self::Path(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unix domain socket paths cannot be converted to SocketAddr",
            )),
        }
    }
}

impl std::str::FromStr for UnifyAddr {
    type Err = AddrParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl PartialEq<str> for UnifyAddr {
    fn eq(&self, other: &str) -> bool {
        match self {
            Self::Socket(s) => {
                match other.parse::<SocketAddr>() {
                    Ok(addr) => *s == addr,
                    Err(_) => {
                        // compatibility case: 'other' is IpAddr
                        match other.parse::<IpAddr>() {
                            Ok(addr) => s.ip() == addr,
                            Err(_) => false,
                        }
                    }
                }
            }
            Self::Path(p) => *p == std::path::Path::new(other),
        }
    }
}

/// Unify behavior of tcp & unix stream
pub enum UnifyStream<IO: AsyncIO> {
    Tcp(TcpStream<IO>),
    Unix(UnixStream<IO>),
}

impl<IO: AsyncIO> UnifyStream<IO> {
    /// Connect to a unified address asynchronously.
    ///
    /// This method attempts to establish a connection to the specified
    /// address, automatically determining whether to use TCP or Unix socket
    /// based on the address type.
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to connect to, can be a string, SocketAddr, or PathBuf
    ///
    /// # Returns
    ///
    /// A future that resolves to a `Result` containing either the connected
    /// UnifyStream or an I/O error.
    pub async fn connect<A: ResolveAddr + ?Sized>(addr: &A) -> io::Result<Self> {
        // generic params are Sized by default, while str is ?Sized
        match addr.resolve() {
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("addr {:?} invalid: {:?}", addr, e),
                ));
            }
            Ok(UnifyAddr::Socket(socket_addr)) => {
                let stream = IO::connect_tcp(&socket_addr).await?;
                let tcp_stream = TcpStream { inner: stream };
                Ok(UnifyStream::Tcp(tcp_stream))
            }
            Ok(UnifyAddr::Path(path)) => {
                let stream = IO::connect_unix(&path).await?;
                let unix_stream = UnixStream { inner: stream };
                Ok(UnifyStream::Unix(unix_stream))
            }
        }
    }

    /// Connect to a unified address asynchronously with a timeout.
    ///
    /// This method attempts to establish a connection to the specified
    /// address, automatically determining whether to use TCP or Unix socket
    /// based on the address type. If the connection attempt takes longer than
    /// the specified timeout, an error will be returned.
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to connect to, can be a string, SocketAddr, or PathBuf
    /// * `timeout` - The maximum time to wait for the connection
    ///
    /// # Returns
    ///
    /// A future that resolves to a `Result` containing either the connected
    /// UnifyStream or an I/O error.
    pub async fn connect_timeout<A>(addr: &A, timeout: Duration) -> io::Result<Self>
    where
        IO: AsyncTime,
        A: ResolveAddr + ?Sized,
    {
        // generic params are Sized by default, while str is ?Sized
        io_with_timeout!(IO, timeout, Self::connect::<A>(addr))
    }

    #[inline]
    pub async fn shutdown_write(&mut self) -> io::Result<()> {
        match self {
            UnifyStream::Tcp(stream) => {
                stream.inner.async_write(|s| s.shutdown(std::net::Shutdown::Write)).await
            }
            UnifyStream::Unix(stream) => {
                stream.inner.async_write(|s| s.shutdown(std::net::Shutdown::Write)).await
            }
        }
    }

    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self {
            UnifyStream::Tcp(stream) => stream.peer_addr(),
            UnifyStream::Unix(_) => Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "unix socket don't support peer_addr",
            )),
        }
    }
}

impl<IO: AsyncIO> fmt::Debug for UnifyStream<IO> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Tcp(stream) => stream.fmt(f),
            Self::Unix(stream) => stream.fmt(f),
        }
    }
}

impl<IO: AsyncIO> AsyncRead for UnifyStream<IO> {
    #[inline(always)]
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            UnifyStream::Tcp(stream) => stream.read(buf).await,
            UnifyStream::Unix(stream) => stream.read(buf).await,
        }
    }
}

impl<IO: AsyncIO> AsyncWrite for UnifyStream<IO> {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            UnifyStream::Tcp(stream) => stream.write(buf).await,
            UnifyStream::Unix(stream) => stream.write(buf).await,
        }
    }
}

/// Unify behavior of tcp & unix socket listener, provides ad bind that directly accept str
pub enum UnifyListener<IO: AsyncIO> {
    Tcp(TcpListener<IO>),
    Unix(UnixListener<IO>),
}

impl<IO: AsyncIO> UnifyListener<IO> {
    #[inline(always)]
    pub fn from_std_unix(l: StdUnixListener) -> io::Result<Self> {
        return Ok(UnifyListener::Unix(UnixListener::<IO>::from_std(l)?));
    }

    #[inline(always)]
    pub fn from_std_tcp(l: StdTcpListener) -> io::Result<Self> {
        return Ok(UnifyListener::Tcp(TcpListener::<IO>::from_std(l)?));
    }

    /// This is a smart version of bind, accepts string type addr
    ///
    /// For unix, will remove the path if exist, prevent failure
    pub async fn bind<A: ResolveAddr + ?Sized>(addr: &A) -> io::Result<Self> {
        // generic params are Sized by default, while str is ?Sized
        match addr.resolve() {
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("addr {:?} invalid: {:?}", addr, e),
                ));
            }
            Ok(UnifyAddr::Socket(_addr)) => Ok(Self::Tcp(TcpListener::<IO>::bind(&_addr).await?)),
            Ok(UnifyAddr::Path(ref path)) => {
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
                return Ok(Self::Unix(UnixListener::<IO>::bind(path)?));
            }
        }
    }

    #[inline]
    pub async fn accept(&mut self) -> io::Result<UnifyStream<IO>> {
        match self {
            UnifyListener::Tcp(listener) => match listener.accept().await {
                Ok(stream) => Ok(UnifyStream::Tcp(stream)),
                Err(e) => Err(e),
            },
            UnifyListener::Unix(listener) => match listener.accept().await {
                Ok(stream) => Ok(UnifyStream::Unix(stream)),
                Err(e) => Err(e),
            },
        }
    }

    #[inline]
    pub fn local_addr(&self) -> io::Result<String> {
        match self {
            UnifyListener::Tcp(listener) => listener.local_addr(),
            UnifyListener::Unix(listener) => listener.local_addr(),
        }
    }

    /// This function is for graceful restart, recognize address type according to string
    pub unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self>
    where
        Self: AsRawFd,
    {
        match UnifyAddr::from_str(addr) {
            Err(e) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("addr {:?} invalid: {:?}", addr, e),
            )),
            Ok(UnifyAddr::Socket(_)) => {
                let listener = unsafe { StdTcpListener::from_raw_fd(raw_fd) };
                match TcpListener::from_std(listener) {
                    Ok(l) => Ok(UnifyListener::Tcp(l)),
                    Err(e) => Err(e),
                }
            }
            Ok(UnifyAddr::Path(_)) => {
                let listener = unsafe { StdUnixListener::from_raw_fd(raw_fd) };
                match UnixListener::from_std(listener) {
                    Ok(l) => Ok(UnifyListener::Unix(l)),
                    Err(e) => Err(e),
                }
            }
        }
    }
}

impl<IO: AsyncIO> AsyncListener for UnifyListener<IO> {
    type Conn = UnifyStream<IO>;

    #[inline]
    async fn bind(addr: &str) -> io::Result<Self> {
        UnifyListener::<IO>::bind(addr).await
    }

    #[inline]
    async fn accept(&mut self) -> io::Result<UnifyStream<IO>> {
        UnifyListener::<IO>::accept(self).await
    }

    #[inline]
    fn local_addr(&self) -> io::Result<String> {
        UnifyListener::<IO>::local_addr(self)
    }

    /// This function is for graceful restart, recognize address type according to string
    #[inline]
    unsafe fn try_from_raw_fd(addr: &str, raw_fd: RawFd) -> io::Result<Self>
    where
        Self: AsRawFd,
    {
        unsafe { UnifyListener::try_from_raw_fd(addr, raw_fd) }
    }
}

impl<IO: AsyncIO> fmt::Debug for UnifyListener<IO> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Tcp(listener) => listener.fmt(f),
            Self::Unix(listener) => listener.fmt(f),
        }
    }
}

impl<IO: AsyncIO> AsRawFd for UnifyListener<IO> {
    fn as_raw_fd(&self) -> RawFd {
        match self {
            Self::Tcp(listener) => listener.as_raw_fd(),
            Self::Unix(listener) => listener.as_raw_fd(),
        }
    }
}
