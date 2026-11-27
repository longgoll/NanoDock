use anyhow::Context as _;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wsl_bridge::WslExecutor;

use crate::lifecycle::docker_api::{
    decode_docker_logs, docker_http_json_request, docker_http_request, sanitize_path_segment,
};
use crate::transport::TransportManager;

/// Executes a non-interactive command inside a running container (`exec create` + `exec start`).
pub async fn exec_container<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    command: &[String],
) -> anyhow::Result<(i64, String)> {
    let clean_id = sanitize_path_segment(id)?;
    if command.is_empty() {
        anyhow::bail!("command cannot be empty");
    }

    let create_body = serde_json::json!({
        "AttachStdout": true,
        "AttachStderr": true,
        "Tty": false,
        "Cmd": command,
    });
    let (_, create_resp) = docker_http_json_request(
        transport,
        "POST",
        &format!("/containers/{clean_id}/exec"),
        Some(&create_body),
    )
    .await?;

    let exec_val: Value =
        serde_json::from_slice(&create_resp).context("failed to parse exec create response")?;
    let exec_id = exec_val
        .get("Id")
        .and_then(Value::as_str)
        .context("missing exec Id")?;

    let start_body = serde_json::json!({ "Detach": false, "Tty": false });
    let (_, raw_output) = docker_http_json_request(
        transport,
        "POST",
        &format!("/exec/{exec_id}/start"),
        Some(&start_body),
    )
    .await?;

    let output = decode_docker_logs(&raw_output);
    let (_, inspect_resp) =
        docker_http_request(transport, "GET", &format!("/exec/{exec_id}/json")).await?;
    let inspect_val: Value = serde_json::from_slice(&inspect_resp).unwrap_or_default();
    let exit_code = inspect_val
        .get("ExitCode")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    Ok((exit_code, output))
}

/// Starts an interactive TTY exec session (`Connection: Upgrade`) and returns
/// `(exec_id, initial_bytes, hijacked_stream)`.
pub async fn start_interactive_exec<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    shell: Option<&str>,
    cols: u16,
    rows: u16,
) -> anyhow::Result<(String, Vec<u8>, Box<dyn crate::transport::traits::AsyncStream>)> {
    let clean_id = sanitize_path_segment(id)?;
    let cmd_shell = shell
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("/bin/sh");

    let create_body = serde_json::json!({
        "AttachStdin": true,
        "AttachStdout": true,
        "AttachStderr": true,
        "Tty": true,
        "Env": ["TERM=xterm-256color"],
        "Cmd": [cmd_shell],
    });
    let (_, create_resp) = docker_http_json_request(
        transport,
        "POST",
        &format!("/containers/{clean_id}/exec"),
        Some(&create_body),
    )
    .await?;

    let exec_val: Value =
        serde_json::from_slice(&create_resp).context("failed to parse exec create response")?;
    let exec_id = exec_val
        .get("Id")
        .and_then(Value::as_str)
        .context("missing exec Id")?
        .to_string();

    let start_body = serde_json::to_vec(&serde_json::json!({
        "Detach": false,
        "Tty": true,
    }))?;
    let mut stream = transport.open_docker_stream().await?;
    let req = format!(
        "POST /exec/{exec_id}/start HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nConnection: Upgrade\r\nUpgrade: tcp\r\nContent-Length: {}\r\n\r\n",
        start_body.len()
    );
    stream.write_all(req.as_bytes()).await?;
    stream.write_all(&start_body).await?;

    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 512];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 16_384 {
            anyhow::bail!("exec start response headers too large");
        }
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            anyhow::bail!("dockerd closed exec stream before upgrade");
        }
        buf.extend_from_slice(&tmp[..n]);
    };

    let status_line = std::str::from_utf8(&buf[..header_end])
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if status != 101 && status != 200 {
        anyhow::bail!("exec upgrade failed with HTTP {status}");
    }

    let _ = resize_exec(transport, &exec_id, cols.max(20), rows.max(5)).await;
    let leftover = buf.split_off(header_end);
    Ok((exec_id, leftover, stream))
}

/// Resizes an active TTY exec session (`POST /exec/{id}/resize`).
pub async fn resize_exec<E: WslExecutor>(
    transport: &TransportManager<E>,
    exec_id: &str,
    cols: u16,
    rows: u16,
) -> anyhow::Result<()> {
    let clean_exec = sanitize_path_segment(exec_id)?;
    let path = format!("/exec/{clean_exec}/resize?h={rows}&w={cols}");
    let _ = docker_http_request(transport, "POST", &path).await?;
    Ok(())
}

/// Returns the exit code of a finished exec instance.
pub async fn inspect_exec_exit_code<E: WslExecutor>(
    transport: &TransportManager<E>,
    exec_id: &str,
) -> i64 {
    let Ok(clean_exec) = sanitize_path_segment(exec_id) else {
        return 0;
    };
    let Ok((_, resp)) =
        docker_http_request(transport, "GET", &format!("/exec/{clean_exec}/json")).await
    else {
        return 0;
    };
    serde_json::from_slice::<Value>(&resp)
        .ok()
        .and_then(|v| v.get("ExitCode").and_then(Value::as_i64))
        .unwrap_or(0)
}
