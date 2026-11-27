use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;
use wsl_bridge::WslExecutor;

use crate::lifecycle::docker_api::decode_chunked_body;
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

    if let Some(rewritten) = prepare_initial_request(client, &mut buf).await? {
        upstream.write_all(&rewritten).await?;
    } else {
        upstream.write_all(&buf).await?;
    }

    bridge_streams(client, &mut upstream).await;
    Ok(())
}

/// Bridges `client` (Windows Named Pipe) and `upstream` (`yamux::Stream`).
///
/// Windows Named Pipes do not support TCP-style half-close (`shutdown()`), so
/// `tokio::io::copy_bidirectional` can deadlock waiting for `client` read EOF
/// after `upstream` has already closed. When `upstream -> client` reaches EOF,
/// flushing and dropping the pipe immediately unblocks `docker.exe`.
async fn bridge_streams<C, U>(client: &mut C, upstream: &mut U)
where
    C: AsyncRead + AsyncWrite + Unpin,
    U: AsyncRead + AsyncWrite + Unpin,
{
    let (mut cr, mut cw) = tokio::io::split(client);
    let (mut ur, mut uw) = tokio::io::split(upstream);

    let mut c2u = std::pin::pin!(async {
        let res = tokio::io::copy(&mut cr, &mut uw).await;
        let _ = uw.shutdown().await;
        res
    });
    let mut u2c = std::pin::pin!(async {
        let res = tokio::io::copy(&mut ur, &mut cw).await;
        let _ = cw.flush().await;
        res
    });

    tokio::select! {
        _ = &mut u2c => {
            // Upstream closed its write side; drop pipe so client sees EOF.
        }
        _ = &mut c2u => {
            // Client finished writing; wait for upstream to finish responding.
            let _ = u2c.await;
        }
    }
}

async fn prepare_initial_request<S>(
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
    if is_upgrade_request(&header_str) {
        return Ok(None);
    }

    let first_line = header_str.lines().next().unwrap_or("");
    if is_container_create_request(first_line) {
        if is_chunked_request(&header_str) {
            while decode_chunked_body(&buf[header_end..]).is_none()
                && buf.len() - header_end < MAX_TRANSLATE_BODY_BYTES
            {
                let mut tmp = [0u8; 4096];
                let n = client.read(&mut tmp).await?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            if let Some(body) = decode_chunked_body(&buf[header_end..]) {
                if let Some(new_body) = translate_container_create_json(&body) {
                    debug!(
                        old_len = body.len(),
                        new_len = new_body.len(),
                        "Translated Windows bind-mount paths in chunked /containers/create"
                    );
                    let updated_header =
                        force_connection_close(&replace_chunked_with_content_length(
                            &header_str,
                            new_body.len(),
                        ));
                    let mut out = Vec::with_capacity(updated_header.len() + new_body.len());
                    out.extend_from_slice(updated_header.as_bytes());
                    out.extend_from_slice(&new_body);
                    return Ok(Some(out));
                }
            }
        } else if let Some(content_len) = parse_content_length(&header_str) {
            if content_len > 0 && content_len <= MAX_TRANSLATE_BODY_BYTES {
                let total_needed = header_end + content_len;
                while buf.len() < total_needed {
                    let mut tmp = vec![0u8; (total_needed - buf.len()).min(16_384)];
                    let n = client.read(&mut tmp).await?;
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                }

                if buf.len() >= total_needed {
                    let body = &buf[header_end..total_needed];
                    if let Some(new_body) = translate_container_create_json(body) {
                        debug!(
                            old_len = body.len(),
                            new_len = new_body.len(),
                            "Translated Windows bind-mount paths in /containers/create"
                        );
                        let updated_header = force_connection_close(&replace_content_length(
                            &header_str,
                            new_body.len(),
                        ));
                        let mut out = Vec::with_capacity(
                            updated_header.len() + new_body.len() + (buf.len() - total_needed),
                        );
                        out.extend_from_slice(updated_header.as_bytes());
                        out.extend_from_slice(&new_body);
                        out.extend_from_slice(&buf[total_needed..]);
                        return Ok(Some(out));
                    }
                }
            }
        }
    }

    // Force Connection: close on non-upgrade requests so HTTP/1.1 Keep-Alive
    // pooling cannot bypass /containers/create inspection on subsequent calls.
    if has_connection_close(&header_str) {
        return Ok(None);
    }
    let updated_header = force_connection_close(&header_str);
    let mut out = Vec::with_capacity(updated_header.len() + (buf.len() - header_end));
    out.extend_from_slice(updated_header.as_bytes());
    out.extend_from_slice(&buf[header_end..]);
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

fn is_chunked_request(header_str: &str) -> bool {
    header_str.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, val)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                && val.to_ascii_lowercase().contains("chunked")
        })
    })
}

fn replace_chunked_with_content_length(header_str: &str, new_len: usize) -> String {
    let mut out = String::with_capacity(header_str.len() + 32);
    for line in header_str.split_inclusive("\r\n") {
        if let Some((name, _)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("transfer-encoding") {
                out.push_str(&format!("Content-Length: {new_len}\r\n"));
                continue;
            }
        }
        out.push_str(line);
    }
    out
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

fn is_upgrade_request(header_str: &str) -> bool {
    for line in header_str.lines() {
        if let Some((name, val)) = line.split_once(':') {
            let name = name.trim();
            if name.eq_ignore_ascii_case("upgrade") {
                return true;
            }
            if name.eq_ignore_ascii_case("connection")
                && val.to_ascii_lowercase().contains("upgrade")
            {
                return true;
            }
        }
    }
    false
}

fn has_connection_close(header_str: &str) -> bool {
    header_str.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, val)| {
            name.trim().eq_ignore_ascii_case("connection")
                && val
                    .split(',')
                    .any(|token| token.trim().eq_ignore_ascii_case("close"))
        })
    })
}

fn force_connection_close(header_str: &str) -> String {
    let mut out = String::with_capacity(header_str.len() + 20);
    let mut replaced = false;
    for line in header_str.split_inclusive("\r\n") {
        if line == "\r\n" {
            if !replaced {
                out.push_str("Connection: close\r\n");
                replaced = true;
            }
            out.push_str(line);
            continue;
        }
        if let Some((name, _)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("connection") {
                if !replaced {
                    out.push_str("Connection: close\r\n");
                    replaced = true;
                }
                continue;
            }
        }
        out.push_str(line);
    }
    out
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

        let rewritten = prepare_initial_request(&mut client, &mut buf)
            .await
            .unwrap()
            .expect("expected rewritten request");
        let text = String::from_utf8(rewritten).unwrap();
        assert!(text.contains("/mnt/c/work/app:/app:rw"));
        assert!(text.contains("Connection: close\r\n"));
        let header_end = find_header_end(text.as_bytes()).unwrap();
        let new_len = parse_content_length(&text[..header_end]).unwrap();
        assert_eq!(new_len, text.len() - header_end);
    }

    #[tokio::test]
    async fn forces_connection_close_on_non_upgrade_and_skips_upgrade() {
        let ping = b"GET /_ping HTTP/1.1\r\nHost: docker\r\n\r\n";
        let mut buf = ping.to_vec();
        let (mut client, _writer) = tokio::io::duplex(64);
        let rewritten = prepare_initial_request(&mut client, &mut buf)
            .await
            .unwrap()
            .expect("expected Connection: close");
        let text = String::from_utf8(rewritten).unwrap();
        assert!(text.contains("Connection: close\r\n"));

        let upgrade = b"POST /containers/abc/attach HTTP/1.1\r\nConnection: Upgrade\r\nUpgrade: tcp\r\n\r\n";
        let mut up_buf = upgrade.to_vec();
        assert!(prepare_initial_request(&mut client, &mut up_buf)
            .await
            .unwrap()
            .is_none());

        let already_close = b"GET /_ping HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n\r\n";
        let mut close_buf = already_close.to_vec();
        assert!(prepare_initial_request(&mut client, &mut close_buf)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn rewrites_chunked_container_create_body_to_content_length() {
        let body = r#"{"Image":"alpine","HostConfig":{"Binds":["D:\\src:/app:rw"]}}"#;
        let chunked = format!("{:x}\r\n{}\r\n0\r\n\r\n", body.len(), body);
        let req = format!(
            "POST /containers/create HTTP/1.1\r\nHost: docker\r\nTransfer-Encoding: chunked\r\n\r\n{chunked}"
        );
        let mut buf = req.as_bytes()[..24].to_vec();
        let (mut client, mut writer) = tokio::io::duplex(1024);
        writer.write_all(&req.as_bytes()[24..]).await.unwrap();
        drop(writer);

        let rewritten = prepare_initial_request(&mut client, &mut buf)
            .await
            .unwrap()
            .expect("expected rewritten chunked request");
        let text = String::from_utf8(rewritten).unwrap();
        assert!(text.contains("/mnt/d/src:/app:rw"));
        assert!(!text.to_ascii_lowercase().contains("transfer-encoding"));
        assert!(text.contains("Connection: close\r\n"));
        let header_end = find_header_end(text.as_bytes()).unwrap();
        let new_len = parse_content_length(&text[..header_end]).unwrap();
        assert_eq!(new_len, text.len() - header_end);
    }
}
