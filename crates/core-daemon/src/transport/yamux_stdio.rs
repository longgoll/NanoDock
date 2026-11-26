use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Context as _;
use async_trait::async_trait;
use futures::future::poll_fn;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use tracing::{debug, error};
use yamux::{Config, Connection, Mode};

use crate::transport::traits::{BoxedStream, StreamTransport};

enum ControlMsg {
    OpenStream(oneshot::Sender<anyhow::Result<yamux::Stream>>),
    Close,
}

struct ChildStdioStream {
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl AsyncRead for ChildStdioStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ChildStdioStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

/// Persistent `yamux` client over a single `wsl.exe -d <distro> -e /usr/local/bin/nanodock-agent` process.
pub struct YamuxStdioTransport {
    cmd_tx: mpsc::Sender<ControlMsg>,
    alive: Arc<AtomicBool>,
}

impl YamuxStdioTransport {
    pub fn spawn(distro_name: &str) -> anyhow::Result<Self> {
        let mut cmd = Command::new("wsl.exe");
        cmd.args(["-d", distro_name, "-e", "/usr/local/bin/nanodock-agent"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child: Child = cmd
            .spawn()
            .context("failed to spawn wsl.exe nanodock-agent")?;

        let stdin = child.stdin.take().context("missing child stdin")?;
        let stdout = child.stdout.take().context("missing child stdout")?;
        let stdio = ChildStdioStream { stdin, stdout };

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<ControlMsg>(64);
        let alive = Arc::new(AtomicBool::new(true));
        let alive_flag = Arc::clone(&alive);

        tokio::spawn(async move {
            let mut conn = Connection::new(stdio.compat(), Config::default(), Mode::Client);
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
            let _ = child.kill().await;
        });

        Ok(Self { cmd_tx, alive })
    }
}

#[async_trait]
impl StreamTransport for YamuxStdioTransport {
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
