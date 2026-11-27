#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use futures::future::poll_fn;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
#[cfg(unix)]
use yamux::{Config, Connection, Mode};

#[cfg(unix)]
use crate::stdio::StdioStream;

/// Runs a `yamux` server over `StdioStream`, forwarding each multiplexed stream
/// to the Docker Unix socket at `socket_path`.
#[cfg(unix)]
pub async fn run_yamux_server(stdio: StdioStream, socket_path: &str) -> anyhow::Result<()> {
    let mut conn = Connection::new(stdio.compat(), build_yamux_config(), Mode::Server);

    while let Some(stream) = poll_fn(|cx| conn.poll_next_inbound(cx))
        .await
        .transpose()
        .context("yamux inbound stream error")?
    {
        let path = socket_path.to_string();
        tokio::spawn(async move {
            if let Err(err) = forward_stream(stream, &path).await {
                eprintln!("nanodock-agent stream error: {err:#}");
            }
        });
    }

    Ok(())
}

#[cfg(unix)]
async fn forward_stream(stream: yamux::Stream, socket_path: &str) -> anyhow::Result<()> {
    let mut unix = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to {socket_path}"))?;
    let mut compat_stream = stream.compat();

    tokio::io::copy_bidirectional(&mut compat_stream, &mut unix).await?;
    Ok(())
}

#[cfg(unix)]
fn build_yamux_config() -> Config {
    let mut cfg = Config::default();
    cfg.set_max_num_streams(1024);
    cfg.set_max_connection_receive_window(Some(16 * 1024 * 1024));
    cfg
}
