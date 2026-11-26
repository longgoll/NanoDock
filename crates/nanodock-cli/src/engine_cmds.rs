use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::Context as _;
use shared_types::{ControlRequest, ControlResponse, EngineConfig, EngineState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

pub const CONTROL_PIPE_NAME: &str = r"\\.\pipe\nanodock_ctl";

pub async fn cmd_status() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::GetStatus).await? {
        ControlResponse::Status {
            state,
            running_containers,
            vhdx_size_bytes,
        } => {
            println!("State:              {state:?}");
            println!("Active streams:     {running_containers}");
            if let Some(bytes) = vhdx_size_bytes {
                let mb = bytes as f64 / (1024.0 * 1024.0);
                println!("VHDX size:          {mb:.1} MB");
            }
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

pub async fn cmd_start() -> anyhow::Result<u8> {
    ensure_daemon_running().await?;
    match send_control_request(ControlRequest::StartEngine).await? {
        ControlResponse::Ack => {
            println!("NanoDock engine started.");
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

pub async fn cmd_stop() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::StopEngine).await? {
        ControlResponse::Ack => {
            println!("NanoDock engine stopped.");
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

pub async fn cmd_trim() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::TrimDisk).await? {
        ControlResponse::Ack => {
            println!("Disk trim completed.");
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

pub async fn cmd_config() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::GetConfig).await? {
        ControlResponse::Config(cfg) => {
            println!("{}", serde_json::to_string_pretty(&cfg)?);
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

pub async fn cmd_logs(lines: usize) -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::GetEngineLogs { lines }).await? {
        ControlResponse::EngineLogs {
            daemon_log,
            dockerd_log,
        } => {
            println!("=== core-daemon.log ===");
            if daemon_log.trim().is_empty() {
                println!("(no daemon logs available)");
            } else {
                println!("{daemon_log}");
            }
            println!("\n=== dockerd.log (WSL2) ===");
            if dockerd_log.trim().is_empty() {
                println!("(no dockerd logs available or engine is sleeping)");
            } else {
                println!("{dockerd_log}");
            }
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

pub async fn ensure_daemon_running() -> anyhow::Result<()> {
    if Path::new(CONTROL_PIPE_NAME).exists() {
        return Ok(());
    }

    if let Some(daemon_exe) = find_sibling_exe("core-daemon.exe") {
        let mut cmd = Command::new(&daemon_exe);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to start {}", daemon_exe.display()))?;

        for _ in 0..30 {
            if send_control_request(ControlRequest::GetStatus).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        anyhow::bail!("core-daemon started but control pipe did not become ready in time");
    }

    anyhow::bail!(
        "core-daemon is not running. Start NanoDock UI or run `cargo run -p core-daemon` first."
    )
}

pub async fn ensure_engine_ready() -> anyhow::Result<EngineConfig> {
    ensure_daemon_running().await?;

    if let ControlResponse::Status { state, .. } =
        send_control_request(ControlRequest::GetStatus).await?
    {
        if state != EngineState::Running {
            match send_control_request(ControlRequest::StartEngine).await? {
                ControlResponse::Ack => {}
                ControlResponse::Error { message } => anyhow::bail!(message),
                other => anyhow::bail!("unexpected response starting engine: {other:?}"),
            }
        }
    }

    match send_control_request(ControlRequest::GetConfig).await? {
        ControlResponse::Config(cfg) => Ok(cfg),
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response fetching config: {other:?}"),
    }
}

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

async fn open_control_pipe_with_retry(
) -> anyhow::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    const ERROR_PIPE_BUSY: i32 = 231;
    for attempt in 0..20 {
        match ClientOptions::new().open(CONTROL_PIPE_NAME) {
            Ok(client) => return Ok(client),
            Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempt < 19 => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err) => {
                return Err(err).context("core-daemon is not running (failed to open control pipe)");
            }
        }
    }
    anyhow::bail!("core-daemon control pipe remained busy")
}

fn find_sibling_exe(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(name);
    candidate.exists().then_some(candidate)
}
