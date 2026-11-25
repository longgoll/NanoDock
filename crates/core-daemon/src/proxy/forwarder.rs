use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;
use wsl_bridge::WslExecutor;

use crate::proxy::path_translator::translate_container_create_json;
use crate::transport::TransportManager;

const MAX_TRANSLATE_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Bridges an incoming Windows Named Pipe client stream to `/var/run/docker.sock`
/// via `TransportManager`, translating Windows bind-mount paths on `POST /containers/create`.
pub async fn handle_client_stream<S, E>(
    mut client: S,
    transport_mgr: Arc<TransportManager<E>>,
    active_streams: Arc<AtomicUsize>,
) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
    E: WslExecutor,
{
    active_streams.fetch_add(1, Ordering::SeqCst);
    let result = forward_inner(&mut client, &transport_mgr).await;
    active_streams.fetch_sub(1, Ordering::SeqCst);
    result
}

async fn forward_inner<S, E>(
    client: &mut S,
    transport_mgr: &TransportManager<E>,
) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
    E: WslExecutor,
{
    let mut upstream = transport_mgr.open_docker_stream().await?;

    let mut buf = vec![0u8; 8192];
    let n = client.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    buf.truncate(n);

    if let Some(rewritten) = try_rewrite_container_create(client, &mut buf).await? {
        upstream.write_all(&rewritten).await?;
    } else {
        upstream.write_all(&buf).await?;
    }

    let _ = tokio::io::copy_bidirectional(client, &mut upstream).await;
    Ok(())
}

async fn try_rewrite_container_create<S>(
    client: &mut S,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Option<Vec<u8>>>
where
    S: AsyncRead + Unpin,
{
    // Read until end of HTTP headers if needed (bounded at 64 KB).
    while find_header_end(buf).is_none() && buf.len() < 65_536 {
        let mut tmp = [0u8; 4096];
        let n = client.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }

    let Some(header_end) = find_header_end(buf) else {
        return Ok(None);
    };

    let header_str = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let first_line = header_str.lines().next().unwrap_or("");
    if !is_container_create_request(first_line) {
        return Ok(None);
    }

    let Some(content_len) = parse_content_length(&header_str) else {
        return Ok(None);
    };
    if content_len == 0 || content_len > MAX_TRANSLATE_BODY_BYTES {
        return Ok(None);
    }

    let total_needed = header_end + content_len;
    while buf.len() < total_needed {
        let mut tmp = vec![0u8; (total_needed - buf.len()).min(16_384)];
        let n = client.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }

    if buf.len() < total_needed {
        return Ok(None);
    }

    let body = &buf[header_end..total_needed];
    let Some(new_body) = translate_container_create_json(body) else {
        return Ok(None);
    };

    debug!(
        old_len = body.len(),
        new_len = new_body.len(),
        "Translated Windows bind-mount paths in /containers/create"
    );

    let updated_header = replace_content_length(&header_str, new_body.len());
    let mut out = Vec::with_capacity(updated_header.len() + new_body.len() + (buf.len() - total_needed));
    out.extend_from_slice(updated_header.as_bytes());
    out.extend_from_slice(&new_body);
    out.extend_from_slice(&buf[total_needed..]);
    Ok(Some(out))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

fn is_container_create_request(first_line: &str) -> bool {
    let mut parts = first_line.split_whitespace();
    let Some(method) = parts.next() else {
        return false;
    };
    let Some(path) = parts.next() else {
        return false;
    };
    let clean_path = path.split('?').next().unwrap_or(path);
    method.eq_ignore_ascii_case("POST") && clean_path.ends_with("/containers/create")
}

fn parse_content_length(header_str: &str) -> Option<usize> {
    for line in header_str.lines() {
        if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return val.trim().parse::<usize>().ok();
            }
        }
    }
    None
}

fn replace_content_length(header_str: &str, new_len: usize) -> String {
    let mut out = String::with_capacity(header_str.len() + 16);
    for line in header_str.split_inclusive("\r\n") {
        if let Some((name, _)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                out.push_str(&format!("Content-Length: {new_len}\r\n"));
                continue;
            }
        }
        out.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_container_create_endpoints() {
        assert!(is_container_create_request("POST /v1.45/containers/create HTTP/1.1"));
        assert!(is_container_create_request("post /containers/create?name=web HTTP/1.1"));
        assert!(!is_container_create_request("GET /containers/json HTTP/1.1"));
        assert!(!is_container_create_request("POST /containers/abc/start HTTP/1.1"));
    }

    #[test]
    fn parses_and_replaces_content_length() {
        let hdr = "POST /containers/create HTTP/1.1\r\nHost: localhost\r\nContent-Length: 42\r\n\r\n";
        assert_eq!(parse_content_length(hdr), Some(42));
        let updated = replace_content_length(hdr, 99);
        assert!(updated.contains("Content-Length: 99\r\n"));
        assert!(!updated.contains("Content-Length: 42"));
    }

    #[tokio::test]
    async fn rewrites_container_create_body_and_updates_content_length() {
        let body = r#"{"Image":"alpine","HostConfig":{"Binds":["C:\\work\\app:/app:rw"]}}"#;
        let req = format!(
            "POST /v1.45/containers/create HTTP/1.1\r\nHost: docker\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let mut buf = req.as_bytes()[..20].to_vec();
        let (mut client, mut writer) = tokio::io::duplex(1024);
        writer.write_all(&req.as_bytes()[20..]).await.unwrap();
        drop(writer);

        let rewritten = try_rewrite_container_create(&mut client, &mut buf)
            .await
            .unwrap()
            .expect("expected rewritten request");
        let text = String::from_utf8(rewritten).unwrap();
        assert!(text.contains("/mnt/c/work/app:/app:rw"));
        let header_end = find_header_end(text.as_bytes()).unwrap();
        let new_len = parse_content_length(&text[..header_end]).unwrap();
        assert_eq!(new_len, text.len() - header_end);
    }
}
