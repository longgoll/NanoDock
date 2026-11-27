use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use wsl_bridge::WslExecutor;

use crate::transport::TransportManager;

const CONTAINER_LIST_CACHE_TTL: Duration = Duration::from_secs(2);

struct CachedContainers {
    fetched_at: Instant,
    items: Vec<Value>,
}

fn containers_cache() -> &'static Mutex<Option<CachedContainers>> {
    static CACHE: OnceLock<Mutex<Option<CachedContainers>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Clears the short-lived `/containers/json` cache so the next query reflects Docker events immediately.
pub(crate) fn invalidate_containers_cache() {
    if let Ok(mut guard) = containers_cache().try_lock() {
        *guard = None;
    }
}

/// Fetches `/containers/json?all=1`, coalescing concurrent UI refresh calls within a 2s TTL.
pub(super) async fn fetch_all_containers_raw<E: WslExecutor>(
    transport: &TransportManager<E>,
    force_fresh: bool,
) -> anyhow::Result<Vec<Value>> {
    let mut guard = containers_cache().lock().await;
    if !force_fresh {
        if let Some(cached) = guard.as_ref() {
            if cached.fetched_at.elapsed() < CONTAINER_LIST_CACHE_TTL {
                return Ok(cached.items.clone());
            }
        }
    }

    let (_, body) = docker_http_request(transport, "GET", "/containers/json?all=1").await?;
    let items: Vec<Value> =
        serde_json::from_slice(&body).context("failed to parse /containers/json?all=1")?;
    *guard = Some(CachedContainers {
        fetched_at: Instant::now(),
        items: items.clone(),
    });
    Ok(items)
}

pub(in crate::lifecycle) async fn docker_http_request<E: WslExecutor>(
    transport: &TransportManager<E>,
    method: &str,
    path: &str,
) -> anyhow::Result<(u16, Vec<u8>)> {
    docker_http_json_request(transport, method, path, None).await
}

pub(in crate::lifecycle) async fn docker_http_json_request<E: WslExecutor>(
    transport: &TransportManager<E>,
    method: &str,
    path: &str,
    json_body: Option<&Value>,
) -> anyhow::Result<(u16, Vec<u8>)> {
    let mut stream = transport.open_docker_stream().await?;
    let body_bytes = match json_body {
        Some(v) => serde_json::to_vec(v)?,
        None => Vec::new(),
    };
    let req = if json_body.is_some() {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body_bytes.len()
        )
    } else {
        format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
    };
    stream.write_all(req.as_bytes()).await?;
    if !body_bytes.is_empty() {
        stream.write_all(&body_bytes).await?;
    }

    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(300), stream.read_to_end(&mut raw))
        .await
        .context("timed out waiting for dockerd HTTP response")??;

    let status = parse_http_status(&raw).context("malformed HTTP status from dockerd")?;
    let body = extract_http_body(&raw).context("malformed HTTP response from dockerd")?;

    if status >= 400 {
        let msg = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|v| v.get("message").and_then(Value::as_str).map(ToString::to_string))
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        anyhow::bail!("Docker API error ({status}): {msg}");
    }

    Ok((status, body))
}

fn parse_http_status(raw: &[u8]) -> Option<u16> {
    let line_end = raw.windows(2).position(|w| w == b"\r\n")?;
    let status_line = std::str::from_utf8(&raw[..line_end]).ok()?;
    status_line.split_whitespace().nth(1)?.parse().ok()
}

pub(crate) fn sanitize_path_segment(input: &str) -> anyhow::Result<&str> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.contains(['/', '\\', ' ', '\r', '\n', '?']) {
        anyhow::bail!("invalid identifier: {input}");
    }
    Ok(trimmed)
}

pub(crate) fn percent_encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Decodes Docker's 8-byte multiplexed log frames, falling back to plain UTF-8 if unframed.
pub(crate) fn decode_docker_logs(raw: &[u8]) -> String {
    let mut pos = 0;
    let mut out = String::new();
    while pos + 8 <= raw.len() {
        let stream_type = raw[pos];
        let padding_ok = raw[pos + 1] == 0 && raw[pos + 2] == 0 && raw[pos + 3] == 0;
        if !matches!(stream_type, 0..=2) || !padding_ok {
            return String::from_utf8_lossy(raw).into_owned();
        }
        let size = u32::from_be_bytes([raw[pos + 4], raw[pos + 5], raw[pos + 6], raw[pos + 7]])
            as usize;
        pos += 8;
        if pos + size > raw.len() {
            return String::from_utf8_lossy(raw).into_owned();
        }
        out.push_str(&String::from_utf8_lossy(&raw[pos..pos + size]));
        pos += size;
    }
    if pos == raw.len() {
        out
    } else {
        String::from_utf8_lossy(raw).into_owned()
    }
}

/// Extracts the HTTP response body, decoding `Transfer-Encoding: chunked` if present.
pub fn extract_http_body(raw: &[u8]) -> Option<Vec<u8>> {
    let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let headers = String::from_utf8_lossy(&raw[..header_end]);
    let body = &raw[header_end..];

    let is_chunked = headers
        .lines()
        .any(|l| l.to_ascii_lowercase().contains("transfer-encoding: chunked"));

    if is_chunked {
        decode_chunked_body(body)
    } else {
        Some(body.to_vec())
    }
}

pub(crate) fn decode_chunked_body(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let crlf = data.windows(2).position(|w| w == b"\r\n")?;
        let size_str = std::str::from_utf8(&data[..crlf]).ok()?.trim();
        let size_hex = size_str.split(';').next()?.trim();
        let size = usize::from_str_radix(size_hex, 16).ok()?;
        data = &data[crlf + 2..];
        if size == 0 {
            break;
        }
        if data.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{decode_docker_logs, extract_http_body};

    #[test]
    fn decodes_plain_json_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]";
        assert_eq!(extract_http_body(raw).as_deref(), Some(b"[]".as_slice()));
    }

    #[test]
    fn decodes_chunked_json_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n[]\r\n0\r\n\r\n";
        assert_eq!(extract_http_body(raw).as_deref(), Some(b"[]".as_slice()));
    }

    #[test]
    fn decodes_multiplexed_docker_log_frames() {
        let mut framed = vec![1, 0, 0, 0, 0, 0, 0, 6];
        framed.extend_from_slice(b"hello\n");
        framed.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 4]);
        framed.extend_from_slice(b"err\n");
        assert_eq!(decode_docker_logs(&framed), "hello\nerr\n");
    }
}
