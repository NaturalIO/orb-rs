use super::{AsyncRead, AsyncWrite};
use std::future::Future;
use std::{fmt, io};

/// A buffered reader that wraps an `AsyncRead` trait object and a buffer.
pub struct AsyncBufRead {
    buf: Vec<u8>,
    pos: usize,
    cap: usize,
}

impl AsyncBufRead {
    /// Creates a new `AsyncBufRead` with the given reader and buffer capacity.
    #[inline]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity {} must > 0", capacity);
        AsyncBufRead { buf: vec![0; capacity], pos: 0, cap: 0 }
    }

    #[inline]
    pub async fn read_buffered<T: AsyncRead>(
        &mut self, reader: &mut T, buf: &mut [u8],
    ) -> io::Result<usize> {
        // If we have bytes in our buffer, copy them to `buf`.
        if self.pos < self.cap {
            let n = std::cmp::min(buf.len(), self.cap - self.pos);
            buf[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }

        // If the request is larger than our buffer, read directly into `buf`.
        // This avoids extra copying.
        if buf.len() >= self.buf.len() {
            return reader.read(buf).await;
        }

        // Otherwise, fill our buffer and then copy to `buf`.
        self.cap = reader.read(&mut self.buf).await?;
        self.pos = 0;
        let n = std::cmp::min(buf.len(), self.cap);
        buf[..n].copy_from_slice(&self.buf[..n]);
        self.pos += n;
        Ok(n)
    }
}

/// A buffered writer that wraps an `AsyncWrite` trait object and a buffer.
pub struct AsyncBufWrite {
    buf: Vec<u8>,
    pos: usize,
}

impl AsyncBufWrite {
    /// Creates a new `AsyncBufWrite` with the given writer and buffer capacity.
    #[inline]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity {} must > 0", capacity);
        AsyncBufWrite { buf: vec![0; capacity], pos: 0 }
    }

    /// Flushes the buffered data to the underlying writer.
    #[inline]
    pub async fn flush<W: AsyncWrite>(&mut self, writer: &mut W) -> io::Result<()> {
        if self.pos > 0 {
            writer.write_all(&self.buf[..self.pos]).await?;
            self.pos = 0;
        }
        Ok(())
    }

    #[inline]
    pub async fn write_buffered<W: AsyncWrite>(
        &mut self, writer: &mut W, buf: &[u8],
    ) -> io::Result<usize> {
        // If the incoming buffer is larger than our internal buffer's capacity,
        // flush our buffer and write the incoming buffer directly.
        if buf.len() >= self.buf.len() {
            self.flush(writer).await?;
            return writer.write(buf).await;
        }

        // If the incoming buffer doesn't fit in the remaining space in our buffer,
        // flush our buffer.
        if self.buf.len() - self.pos < buf.len() {
            self.flush(writer).await?;
        }
        // Copy the incoming buffer into our internal buffer.
        let n = buf.len();
        self.buf[self.pos..self.pos + n].copy_from_slice(buf);
        self.pos += n;
        Ok(n)
    }
}

pub struct AsyncBufStream<T: AsyncRead + AsyncWrite> {
    read_buf: AsyncBufRead,
    write_buf: AsyncBufWrite,
    inner: T,
}

impl<T: AsyncRead + AsyncWrite> AsyncBufStream<T> {
    #[inline]
    pub fn new(stream: T, buf_size: usize) -> Self {
        Self {
            read_buf: AsyncBufRead::new(buf_size),
            write_buf: AsyncBufWrite::new(buf_size),
            inner: stream,
        }
    }

    #[inline(always)]
    pub async fn flush(&mut self) -> io::Result<()> {
        self.write_buf.flush(&mut self.inner).await
    }

    #[inline(always)]
    pub fn get_inner(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: AsyncRead + AsyncWrite + fmt::Debug> fmt::Debug for AsyncBufStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: AsyncRead + AsyncWrite + fmt::Display> fmt::Display for AsyncBufStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: AsyncRead + AsyncWrite> AsyncRead for AsyncBufStream<T> {
    /// Async version of read function
    ///
    /// On ok, return the bytes read
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = io::Result<usize>> + Send {
        async move { self.read_buf.read_buffered(&mut self.inner, buf).await }
    }
}

impl<T: AsyncRead + AsyncWrite> AsyncWrite for AsyncBufStream<T> {
    /// Async version of write function
    ///
    /// On ok, return the bytes written
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> impl Future<Output = io::Result<usize>> + Send {
        async move { self.write_buf.write_buffered(&mut self.inner, buf).await }
    }
}
