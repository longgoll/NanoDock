use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Context as _;
use async_trait::async_trait;
use futures::future::poll_fn;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use tracing::{debug, error};
use yamux::{Config, Connection, Mode};

use crate::transport::traits::{BoxedStream, StreamTransport};

enum ControlMsg {
    OpenStream(oneshot::Sender<anyhow::Result<yamux::Stream>>),
    Close,
}

/// Shared `yamux` client driver over any bidirectional async stream (stdio pipe or Hyper-V socket).
pub struct YamuxClient {
    cmd_tx: mpsc::Sender<ControlMsg>,
    alive: Arc<AtomicBool>,
}

impl YamuxClient {
    /// Spawns a background `yamux` connection driver over `io`, invoking `on_exit` when closed.
    pub fn spawn<IO, F>(io: IO, on_exit: F) -> Self
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        F: FnOnce() + Send + 'static,
    {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<ControlMsg>(64);
        let alive = Arc::new(AtomicBool::new(true));
        let alive_flag = Arc::clone(&alive);

        tokio::spawn(async move {
            let mut conn = Connection::new(io.compat(), build_yamux_config(), Mode::Client);
            loop {
                tokio::select! {
                    msg = cmd_rx.recv() => {
                        match msg {
                            Some(ControlMsg::OpenStream(reply)) => {
                                let res = poll_fn(|cx| conn.poll_new_outbound(cx))
                                    .await
                                    .map_err(|e| anyhow::anyhow!("yamux open stream: {e}"));
                                let _ = reply.send(res);
                            }
                            Some(ControlMsg::Close) | None => {
                                let _ = poll_fn(|cx| conn.poll_close(cx)).await;
                                break;
                            }
                        }
                    }
                    inbound = poll_fn(|cx| conn.poll_next_inbound(cx)) => {
                        match inbound {
                            Some(Ok(_)) => debug!("ignoring unexpected inbound yamux stream"),
                            Some(Err(e)) => {
                                error!("yamux connection error: {e}");
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
            alive_flag.store(false, Ordering::SeqCst);
            on_exit();
        });

        Self { cmd_tx, alive }
    }
}

fn build_yamux_config() -> Config {
    let mut cfg = Config::default();
    cfg.set_max_num_streams(1024);
    // Must be >= 256 KiB * max_num_streams (256 MiB for 1024 streams) per yamux 0.13 invariant.
    cfg.set_max_connection_receive_window(Some(256 * 1024 * 1024));
    cfg
}

#[async_trait]
impl StreamTransport for YamuxClient {
    async fn open_stream(&self) -> anyhow::Result<BoxedStream> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(ControlMsg::OpenStream(tx))
            .await
            .map_err(|_| anyhow::anyhow!("yamux transport driver stopped"))?;
        let stream = rx.await.context("yamux open stream cancelled")??;
        Ok(Box::new(stream.compat()))
    }

    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    async fn close(&self) -> anyhow::Result<()> {
        let _ = self.cmd_tx.send(ControlMsg::Close).await;
        self.alive.store(false, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn yamux_client_multiplexes_streams_over_duplex() {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);

        let server_task = tokio::spawn(async move {
            let mut conn = Connection::new(server_io.compat(), build_yamux_config(), Mode::Server);
            while let Some(Ok(stream)) = poll_fn(|cx| conn.poll_next_inbound(cx)).await {
                tokio::spawn(async move {
                    let mut s = stream.compat();
                    let mut buf = [0u8; 64];
                    if let Ok(n) = s.read(&mut buf).await {
                        let _ = s.write_all(&buf[..n]).await;
                        let _ = s.shutdown().await;
                    }
                });
            }
        });

        let client = YamuxClient::spawn(client_io, || {});
        assert!(client.is_alive());

        for i in 0..4 {
            let mut stream = client.open_stream().await.unwrap();
            let msg = format!("ping-{i}");
            stream.write_all(msg.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
            let mut resp = String::new();
            stream.read_to_string(&mut resp).await.unwrap();
            assert_eq!(resp, msg);
        }

        client.close().await.unwrap();
        assert!(!client.is_alive());
        server_task.abort();
    }
}
