use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, Stdin, Stdout};

/// Combines `stdin` and `stdout` into a single `AsyncRead + AsyncWrite` stream
/// for `yamux` multiplexing.
///
/// Note: Nothing may write to `stdout` other than this stream.
#[cfg_attr(not(unix), allow(dead_code))]
pub struct StdioStream {
    stdin: Stdin,
    stdout: Stdout,
}

#[cfg_attr(not(unix), allow(dead_code))]
impl StdioStream {
    pub fn new() -> Self {
        Self {
            stdin: tokio::io::stdin(),
            stdout: tokio::io::stdout(),
        }
    }
}

impl Default for StdioStream {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncRead for StdioStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_read(cx, buf)
    }
}

impl AsyncWrite for StdioStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stdout).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_shutdown(cx)
    }
}
