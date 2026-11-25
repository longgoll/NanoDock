use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

/// Trait-object stream returned by any multiplexed `StreamTransport`.
pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}

pub type BoxedStream = Box<dyn AsyncStream>;

/// Abstraction for multiplexed transport to `/var/run/docker.sock` inside WSL2.
///
/// Implemented by `YamuxStdioTransport` (and extensible to `AF_HYPERV` later).
#[async_trait]
pub trait StreamTransport: Send + Sync {
    async fn open_stream(&self) -> anyhow::Result<BoxedStream>;
    fn is_alive(&self) -> bool;
    async fn close(&self) -> anyhow::Result<()>;
}
