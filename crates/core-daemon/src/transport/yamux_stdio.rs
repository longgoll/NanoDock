use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};

use anyhow::Context as _;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::warn;

use crate::transport::traits::{BoxedStream, StreamTransport};
use crate::transport::yamux_client::YamuxClient;

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
    client: YamuxClient,
}

impl YamuxStdioTransport {
    pub fn spawn(distro_name: &str) -> anyhow::Result<Self> {
        let mut cmd = Command::new("wsl.exe");
        cmd.args(["-d", distro_name, "-e", "/usr/local/bin/nanodock-agent"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        warn!(target: "guest_agent", "{trimmed}");
                    }
                }
            });
        }

        let stdio = ChildStdioStream { stdin, stdout };
        let client = YamuxClient::spawn(stdio, move || {
            let _ = child.start_kill();
        });

        Ok(Self { client })
    }
}

#[async_trait]
impl StreamTransport for YamuxStdioTransport {
    async fn open_stream(&self) -> anyhow::Result<BoxedStream> {
        self.client.open_stream().await
    }

    fn is_alive(&self) -> bool {
        self.client.is_alive()
    }

    async fn close(&self) -> anyhow::Result<()> {
        self.client.close().await
    }
}
