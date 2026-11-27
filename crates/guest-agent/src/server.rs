#[cfg(unix)]
use std::sync::OnceLock;

#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use futures::future::poll_fn;
#[cfg(unix)]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio::process::Command;
#[cfg(unix)]
use tokio::sync::Mutex;
#[cfg(unix)]
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
#[cfg(unix)]
use yamux::{Config, Connection, Mode};

#[cfg(unix)]
use crate::control::handle_control_stream;

#[cfg(unix)]
const NDK_MAGIC: &[u8; 4] = b"\x00NDK";

#[cfg(unix)]
fn init_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(unix)]
async fn connect_docker_socket(socket_path: &str) -> anyhow::Result<UnixStream> {
    if let Ok(stream) = UnixStream::connect(socket_path).await {
        return Ok(stream);
    }

    // Serialize /init.sh recovery across concurrent streams if dockerd crashed.
    let _guard = init_lock().lock().await;
    if let Ok(stream) = UnixStream::connect(socket_path).await {
        return Ok(stream);
    }

    if std::path::Path::new("/init.sh").exists() {
        let status = Command::new("/init.sh").status().await;
        if let Err(err) = status {
            eprintln!("nanodock-agent /init.sh recovery failed: {err}");
        }
    }

    UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to {socket_path}"))
}

/// Runs a `yamux` server over any bidirectional stream (stdio or AF_VSOCK),
/// routing `\x00NDK` control streams to `control.rs` and all other streams to `socket_path`.
#[cfg(unix)]
pub async fn run_yamux_server<IO>(io: IO, socket_path: &str) -> anyhow::Result<()>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut conn = Connection::new(io.compat(), build_yamux_config(), Mode::Server);

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
    let mut compat_stream = stream.compat();
    let mut prefix = [0u8; 4];
    let mut filled = 0usize;

    while filled < 4 {
        let n = compat_stream.read(&mut prefix[filled..]).await?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    if filled == 0 {
        return Ok(());
    }

    if filled == 4 && &prefix == NDK_MAGIC {
        return handle_control_stream(compat_stream).await;
    }

    let mut unix = connect_docker_socket(socket_path).await?;
    unix.write_all(&prefix[..filled]).await?;
    tokio::io::copy_bidirectional(&mut compat_stream, &mut unix).await?;
    Ok(())
}

#[cfg(unix)]
fn build_yamux_config() -> Config {
    let mut cfg = Config::default();
    cfg.set_max_num_streams(1024);
    // Must be >= 256 KiB * max_num_streams (256 MiB for 1024 streams) per yamux 0.13 invariant.
    cfg.set_max_connection_receive_window(Some(256 * 1024 * 1024));
    cfg
}
