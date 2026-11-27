use std::time::Duration;

use anyhow::Context as _;
use shared_types::{ControlRequest, ControlResponse};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

pub const CONTROL_PIPE_NAME: &str = r"\\.\pipe\nanodock_ctl";

/// Sends a single JSON-line `ControlRequest` to `core-daemon` and awaits the `ControlResponse`.
pub async fn send_control_request(req: ControlRequest) -> anyhow::Result<ControlResponse> {
    let client = open_control_pipe_with_retry().await?;

    let (reader, mut writer) = tokio::io::split(client);
    let mut encoded = serde_json::to_string(&req)?;
    encoded.push('\n');
    writer.write_all(encoded.as_bytes()).await?;

    let mut lines = BufReader::new(reader).lines();
    let line = lines
        .next_line()
        .await?
        .context("control pipe closed before response")?;
    let resp: ControlResponse = serde_json::from_str(&line)?;
    Ok(resp)
}

pub(crate) async fn open_control_pipe_with_retry() -> anyhow::Result<NamedPipeClient> {
    const ERROR_PIPE_BUSY: i32 = 231;
    for attempt in 0..20 {
        match ClientOptions::new().open(CONTROL_PIPE_NAME) {
            Ok(client) => return Ok(client),
            Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempt < 19 => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err) => {
                return Err(err)
                    .context("core-daemon is not running (failed to open control pipe)");
            }
        }
    }
    anyhow::bail!("core-daemon control pipe remained busy")
}
