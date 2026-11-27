use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use shared_types::{
    ControlResponse, DaemonEvent, ExecClientFrame, ExecServerFrame, LogStreamFrame,
    PullProgressFrame,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, Lines};
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::control_rpc::stream_codecs::{DockerLogStreamDecoder, Utf8CarryDecoder};
use crate::lifecycle::container_exec::{
    inspect_exec_exit_code, resize_exec, start_interactive_exec,
};
use crate::lifecycle::docker_api::{
    parse_container_stats_json, percent_encode_query, sanitize_path_segment,
};

struct ActiveStreamGuard<'a>(&'a AtomicUsize);

impl<'a> ActiveStreamGuard<'a> {
    fn new(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for ActiveStreamGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn send_json_line<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    msg: &T,
) -> anyhow::Result<()> {
    let mut encoded = serde_json::to_string(msg)?;
    encoded.push('\n');
    writer.write_all(encoded.as_bytes()).await?;
    Ok(())
}

/// Streams `DaemonEvent` JSON lines to a subscribed client until disconnect or shutdown.
pub(super) async fn stream_daemon_events<E: WslExecutor, W: AsyncWrite + Unpin>(
    writer: &mut W,
    ctx: &AppContext<E>,
) -> anyhow::Result<()> {
    let mut rx = ctx.events.subscribe();
    let initial = DaemonEvent::StateChanged(*ctx.state.read().await);
    send_json_line(writer, &initial).await?;

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            recv = rx.recv() => {
                match recv {
                    Ok(ev) => {
                        if send_json_line(writer, &ev).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let resync = DaemonEvent::StateChanged(*ctx.state.read().await);
                        if send_json_line(writer, &resync).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    Ok(())
}

/// Runs a bidirectional interactive TTY exec session over the control Named Pipe.
pub(super) async fn stream_exec_session<E, R, W>(
    lines: &mut Lines<BufReader<R>>,
    writer: &mut W,
    ctx: &Arc<AppContext<E>>,
    id: &str,
    shell: Option<&str>,
    cols: u16,
    rows: u16,
) -> anyhow::Result<()>
where
    E: WslExecutor,
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let _active_guard = ActiveStreamGuard::new(&ctx.active_streams);
    let (exec_id, leftover, stream) =
        match start_interactive_exec(&ctx.transport, id, shell, cols, rows).await {
            Ok(res) => res,
            Err(err) => {
                let _ = send_json_line(
                    writer,
                    &ExecServerFrame::Error {
                        message: err.to_string(),
                    },
                )
                .await;
                return Ok(());
            }
        };

    send_json_line(
        writer,
        &ExecServerFrame::Started {
            exec_id: exec_id.clone(),
        },
    )
    .await?;

    let mut utf8 = Utf8CarryDecoder::default();
    if !leftover.is_empty() {
        let text = utf8.push(&leftover);
        if !text.is_empty() {
            let _ = send_json_line(writer, &ExecServerFrame::Output { data: text }).await;
        }
    }

    let (mut ur, mut uw) = tokio::io::split(stream);
    let mut read_buf = [0u8; 4096];

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            upstream_res = ur.read(&mut read_buf) => {
                match upstream_res {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = utf8.push(&read_buf[..n]);
                        if !text.is_empty()
                            && send_json_line(writer, &ExecServerFrame::Output { data: text }).await.is_err()
                        {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = send_json_line(writer, &ExecServerFrame::Error { message: err.to_string() }).await;
                        break;
                    }
                }
            }
            client_res = lines.next_line() => {
                match client_res {
                    Ok(Some(raw)) => {
                        if let Ok(frame) = serde_json::from_str::<ExecClientFrame>(&raw) {
                            match frame {
                                ExecClientFrame::Input { data } => {
                                    if uw.write_all(data.as_bytes()).await.is_err() {
                                        break;
                                    }
                                }
                                ExecClientFrame::Resize { cols, rows } => {
                                    let _ = resize_exec(&ctx.transport, &exec_id, cols, rows).await;
                                }
                                ExecClientFrame::Close => break,
                            }
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }

    let _ = uw.shutdown().await;
    let code = inspect_exec_exit_code(&ctx.transport, &exec_id).await;
    let _ = send_json_line(writer, &ExecServerFrame::Exit { code }).await;
    Ok(())
}

/// Streams real-time container logs (`GET /containers/{id}/logs?follow=1`) as `LogStreamFrame` lines.
pub(super) async fn stream_container_logs<E: WslExecutor, W: AsyncWrite + Unpin>(
    writer: &mut W,
    ctx: &AppContext<E>,
    id: &str,
    tail: usize,
) -> anyhow::Result<()> {
    let _active_guard = ActiveStreamGuard::new(&ctx.active_streams);
    let clean_id = match sanitize_path_segment(id) {
        Ok(c) => c,
        Err(err) => {
            let _ = send_json_line(
                writer,
                &LogStreamFrame::Error {
                    message: err.to_string(),
                },
            )
            .await;
            return Ok(());
        }
    };

    let mut stream = ctx.transport.open_docker_stream().await?;
    let req = format!(
        "GET /containers/{clean_id}/logs?follow=1&stdout=1&stderr=1&tail={tail} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let Some(is_chunked) = skip_http_headers_or_report(&mut reader, writer).await? else {
        return Ok(());
    };

    let mut decoder = DockerLogStreamDecoder::new(is_chunked);
    let mut buf = [0u8; 8192];
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            res = reader.read(&mut buf) => {
                match res {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = decoder.push(&buf[..n]);
                        if !text.is_empty()
                            && send_json_line(writer, &LogStreamFrame::Line { text }).await.is_err()
                        {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = send_json_line(writer, &LogStreamFrame::Error { message: err.to_string() }).await;
                        break;
                    }
                }
            }
        }
    }

    let tail_text = decoder.flush();
    if !tail_text.is_empty() {
        let _ = send_json_line(writer, &LogStreamFrame::Line { text: tail_text }).await;
    }
    let _ = send_json_line(writer, &LogStreamFrame::Ended).await;
    Ok(())
}

/// Streams real-time container stats (`GET /containers/{id}/stats?stream=1`) as `ControlResponse::ContainerStats` lines.
pub(super) async fn stream_container_stats<E: WslExecutor, W: AsyncWrite + Unpin>(
    writer: &mut W,
    ctx: &AppContext<E>,
    id: &str,
) -> anyhow::Result<()> {
    let _active_guard = ActiveStreamGuard::new(&ctx.active_streams);
    let clean_id = match sanitize_path_segment(id) {
        Ok(c) => c,
        Err(err) => {
            let _ = send_json_line(
                writer,
                &ControlResponse::Error {
                    message: err.to_string(),
                },
            )
            .await;
            return Ok(());
        }
    };

    let mut stream = ctx.transport.open_docker_stream().await?;
    let req = format!(
        "GET /containers/{clean_id}/stats?stream=1 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await?;

    let mut lines = BufReader::new(stream).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            break;
        }
    }

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            res = lines.next_line() => {
                match res {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if !trimmed.starts_with('{') {
                            continue;
                        }
                        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                            let stats = parse_container_stats_json(clean_id, &val);
                            if send_json_line(writer, &ControlResponse::ContainerStats(stats)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }

    Ok(())
}

/// Streams real-time Docker image pull progress (`POST /images/create`) as `PullProgressFrame` lines.
pub(super) async fn stream_pull_image<E: WslExecutor, W: AsyncWrite + Unpin>(
    writer: &mut W,
    ctx: &AppContext<E>,
    reference: &str,
) -> anyhow::Result<()> {
    let _active_guard = ActiveStreamGuard::new(&ctx.active_streams);
    let trimmed_ref = reference.trim();
    if trimmed_ref.is_empty() || trimmed_ref.contains([' ', '\r', '\n']) {
        let _ = send_json_line(
            writer,
            &PullProgressFrame::Error {
                message: "invalid image reference".to_string(),
            },
        )
        .await;
        return Ok(());
    }

    let encoded = percent_encode_query(trimmed_ref);
    let mut stream = ctx.transport.open_docker_stream().await?;
    let req = format!(
        "POST /images/create?fromImage={encoded} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await?;

    let mut lines = BufReader::new(stream).lines();
    let mut status_code: u16 = 0;
    while let Some(line) = lines.next_line().await? {
        if status_code == 0 {
            status_code = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
        if line.trim().is_empty() {
            break;
        }
    }

    if status_code >= 400 {
        let _ = send_json_line(
            writer,
            &PullProgressFrame::Error {
                message: format!("Docker API error ({status_code})"),
            },
        )
        .await;
        return Ok(());
    }

    let mut had_error = false;
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            res = lines.next_line() => {
                match res {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if !trimmed.starts_with('{') {
                            continue;
                        }
                        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                            if let Some(err_msg) = val.get("error").and_then(Value::as_str) {
                                let _ = send_json_line(
                                    writer,
                                    &PullProgressFrame::Error {
                                        message: err_msg.to_string(),
                                    },
                                )
                                .await;
                                had_error = true;
                                break;
                            }
                            let frame = parse_pull_progress_json(&val);
                            if send_json_line(writer, &frame).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        let _ = send_json_line(
                            writer,
                            &PullProgressFrame::Error {
                                message: err.to_string(),
                            },
                        )
                        .await;
                        had_error = true;
                        break;
                    }
                }
            }
        }
    }

    if !had_error {
        let _ = send_json_line(writer, &PullProgressFrame::Done).await;
    }
    Ok(())
}

fn parse_pull_progress_json(val: &Value) -> PullProgressFrame {
    let status = val
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let id = val.get("id").and_then(Value::as_str).map(ToString::to_string);
    let progress = val
        .get("progress")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let detail = val.get("progressDetail");
    let current = detail
        .and_then(|d| d.get("current"))
        .and_then(Value::as_u64);
    let total = detail.and_then(|d| d.get("total")).and_then(Value::as_u64);
    PullProgressFrame::Progress {
        id,
        status,
        progress,
        current,
        total,
    }
}

async fn skip_http_headers_or_report<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut BufReader<R>,
    writer: &mut W,
) -> anyhow::Result<Option<bool>> {
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut chunked = false;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("transfer-encoding")
                && val.to_ascii_lowercase().contains("chunked")
            {
                chunked = true;
            }
        }
    }
    if status >= 400 {
        let _ = send_json_line(
            writer,
            &LogStreamFrame::Error {
                message: format!("Docker API error ({status})"),
            },
        )
        .await;
        return Ok(None);
    }
    Ok(Some(chunked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_stream_guard_increments_and_decrements_on_drop() {
        let counter = AtomicUsize::new(0);
        {
            let _g1 = ActiveStreamGuard::new(&counter);
            assert_eq!(counter.load(Ordering::SeqCst), 1);
            {
                let _g2 = ActiveStreamGuard::new(&counter);
                assert_eq!(counter.load(Ordering::SeqCst), 2);
            }
            assert_eq!(counter.load(Ordering::SeqCst), 1);
        }
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
