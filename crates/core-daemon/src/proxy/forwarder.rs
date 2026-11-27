use std::sync::atomic::Ordering;
use std::sync::Arc;

use shared_types::TurboMountRule;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::lifecycle::docker_api::decode_chunked_body;
use crate::proxy::http_headers::{
    find_header_end, force_connection_close, has_connection_close, has_expect_100,
    is_chunked_message, is_container_create_request, is_upgrade_request, parse_content_length,
    replace_chunked_with_content_length, replace_content_length, response_supports_keep_alive,
    strip_connection_close,
};
use crate::proxy::path_translator::translate_container_create_json_with_turbo;

const MAX_TRANSLATE_BODY_BYTES: usize = 16 * 1024 * 1024;

struct ActiveStreamGuard<'a>(&'a std::sync::atomic::AtomicUsize);

impl<'a> ActiveStreamGuard<'a> {
    fn new(counter: &'a std::sync::atomic::AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for ActiveStreamGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Bridges an incoming Windows Named Pipe client stream to `/var/run/docker.sock`
/// via `TransportManager`, supporting HTTP/1.1 Keep-Alive and translating Windows
/// bind-mount paths (and Turbo Mounts) on `POST /*/containers/create`.
pub async fn handle_client_stream<S, E>(mut client: S, ctx: Arc<AppContext<E>>) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
    E: WslExecutor + 'static,
{
    let _guard = ActiveStreamGuard::new(&ctx.active_streams);
    forward_inner(&mut client, &ctx).await
}

async fn forward_inner<S, E>(client: &mut S, ctx: &AppContext<E>) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
    E: WslExecutor + 'static,
{
    let mut buf = Vec::with_capacity(8192);

    loop {
        while find_header_end(&buf).is_none() && buf.len() < 65_536 {
            let mut tmp = [0u8; 4096];
            let n = client.read(&mut tmp).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        if buf.is_empty() {
            return Ok(());
        }

        let Some(header_end) = find_header_end(&buf) else {
            let mut upstream = ctx.transport.open_docker_stream().await?;
            upstream.write_all(&buf).await?;
            bridge_streams(client, &mut upstream).await;
            return Ok(());
        };

        let header_str = String::from_utf8_lossy(&buf[..header_end]).into_owned();
        let first_line = header_str.lines().next().unwrap_or("");
        let is_create = is_container_create_request(first_line);
        let content_len = parse_content_length(&header_str);

        let passthrough = is_upgrade_request(&header_str)
            || has_expect_100(&header_str)
            || (is_chunked_message(&header_str) && !is_create)
            || content_len.is_some_and(|n| n > MAX_TRANSLATE_BODY_BYTES);

        if passthrough {
            let mut upstream = ctx.transport.open_docker_stream().await?;
            upstream.write_all(&buf).await?;
            bridge_streams(client, &mut upstream).await;
            return Ok(());
        }

        let (distro_name, turbo_mounts) = {
            let cfg = ctx.config.read().await;
            (cfg.distro_name.clone(), cfg.turbo_mounts.clone())
        };

        let client_asked_close = has_connection_close(&header_str);
        let prep = match prepare_request_with_leftover(client, &mut buf, &turbo_mounts).await? {
            PreparedOutcome::Framed(p) => p,
            PreparedOutcome::Passthrough(raw) => {
                let mut upstream = ctx.transport.open_docker_stream().await?;
                upstream.write_all(&raw).await?;
                bridge_streams(client, &mut upstream).await;
                return Ok(());
            }
        };

        let mut upstream = ctx.transport.open_docker_stream().await?;
        for matched_id in &prep.matched_turbo_ids {
            if let Some(rule) = turbo_mounts.iter().find(|r| r.id == *matched_id && r.enabled) {
                ctx.turbo_sync.sync_rule(&distro_name, rule).await;
            }
        }

        upstream.write_all(&prep.payload).await?;
        upstream.flush().await?;

        let keep_alive =
            relay_upstream_response(client, &mut upstream, client_asked_close).await?;
        if !keep_alive {
            return Ok(());
        }

        buf = prep.leftover;
    }
}

async fn relay_upstream_response<C, U>(
    client: &mut C,
    upstream: &mut U,
    client_asked_close: bool,
) -> anyhow::Result<bool>
where
    C: AsyncWrite + Unpin,
    U: AsyncRead + Unpin,
{
    let mut resp_buf = Vec::with_capacity(4096);
    while find_header_end(&resp_buf).is_none() && resp_buf.len() < 65_536 {
        let mut tmp = [0u8; 4096];
        let n = upstream.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        resp_buf.extend_from_slice(&tmp[..n]);
    }

    let Some(hdr_end) = find_header_end(&resp_buf) else {
        if !resp_buf.is_empty() {
            client.write_all(&resp_buf).await?;
        }
        let _ = tokio::io::copy(upstream, client).await;
        client.flush().await?;
        return Ok(false);
    };

    let resp_hdr = String::from_utf8_lossy(&resp_buf[..hdr_end]).into_owned();
    let keep_alive = response_supports_keep_alive(&resp_hdr, client_asked_close);
    let out_hdr = if keep_alive {
        strip_connection_close(&resp_hdr)
    } else {
        resp_hdr
    };

    client.write_all(out_hdr.as_bytes()).await?;
    if hdr_end < resp_buf.len() {
        client.write_all(&resp_buf[hdr_end..]).await?;
    }
    tokio::io::copy(upstream, client).await?;
    client.flush().await?;

    Ok(keep_alive)
}

/// Bridges `client` (Windows Named Pipe) and `upstream` (`yamux::Stream`) for upgrades/streaming.
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
        _ = &mut u2c => {}
        _ = &mut c2u => {
            let _ = u2c.await;
        }
    }
}

enum PreparedOutcome {
    Framed(PreparedPayload),
    Passthrough(Vec<u8>),
}

struct PreparedPayload {
    payload: Vec<u8>,
    leftover: Vec<u8>,
    matched_turbo_ids: Vec<String>,
}

async fn prepare_request_with_leftover<S>(
    client: &mut S,
    buf: &mut Vec<u8>,
    turbo_mounts: &[TurboMountRule],
) -> anyhow::Result<PreparedOutcome>
where
    S: AsyncRead + Unpin,
{
    while find_header_end(buf).is_none() && buf.len() < 65_536 {
        let mut tmp = [0u8; 4096];
        let n = client.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }

    let header_end = find_header_end(buf).unwrap_or(buf.len());
    let header_str = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let first_line = header_str.lines().next().unwrap_or("");

    if is_container_create_request(first_line) && is_chunked_message(&header_str) {
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
        let Some(body) = decode_chunked_body(&buf[header_end..]) else {
            return Ok(PreparedOutcome::Passthrough(std::mem::take(buf)));
        };
        let res = translate_container_create_json_with_turbo(&body, turbo_mounts);
        let final_body = res.body.unwrap_or(body);
        let updated_header = force_connection_close(&replace_chunked_with_content_length(
            &header_str,
            final_body.len(),
        ));
        let mut out = Vec::with_capacity(updated_header.len() + final_body.len());
        out.extend_from_slice(updated_header.as_bytes());
        out.extend_from_slice(&final_body);
        return Ok(PreparedOutcome::Framed(PreparedPayload {
            payload: out,
            leftover: Vec::new(),
            matched_turbo_ids: res.matched_turbo_ids,
        }));
    }

    let content_len = parse_content_length(&header_str).unwrap_or(0);
    let total_needed = header_end + content_len;
    while buf.len() < total_needed {
        let mut tmp = vec![0u8; (total_needed - buf.len()).min(16_384)];
        let n = client.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }

    let split_at = buf.len().min(total_needed);
    let leftover = buf.split_off(split_at);
    let mut matched_turbo_ids = Vec::new();

    if is_container_create_request(first_line) && content_len > 0 && buf.len() >= total_needed {
        let body = &buf[header_end..total_needed];
        let res = translate_container_create_json_with_turbo(body, turbo_mounts);
        matched_turbo_ids = res.matched_turbo_ids;
        if let Some(new_body) = res.body {
            debug!(
                old_len = body.len(),
                new_len = new_body.len(),
                "Translated Windows bind-mount paths in /containers/create"
            );
            let updated_header =
                force_connection_close(&replace_content_length(&header_str, new_body.len()));
            let mut out = Vec::with_capacity(updated_header.len() + new_body.len());
            out.extend_from_slice(updated_header.as_bytes());
            out.extend_from_slice(&new_body);
            return Ok(PreparedOutcome::Framed(PreparedPayload {
                payload: out,
                leftover,
                matched_turbo_ids,
            }));
        }
    }

    let updated_header = force_connection_close(&header_str);
    let mut out = Vec::with_capacity(updated_header.len() + (buf.len() - header_end));
    out.extend_from_slice(updated_header.as_bytes());
    out.extend_from_slice(&buf[header_end..]);
    Ok(PreparedOutcome::Framed(PreparedPayload {
        payload: out,
        leftover,
        matched_turbo_ids,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_container_create_endpoints() {
        assert!(is_container_create_request(
            "POST /v1.45/containers/create HTTP/1.1"
        ));
        assert!(is_container_create_request(
            "post /containers/create?name=web HTTP/1.1"
        ));
        assert!(!is_container_create_request("GET /containers/json HTTP/1.1"));
        assert!(!is_container_create_request(
            "POST /containers/abc/start HTTP/1.1"
        ));
    }

    #[tokio::test]
    async fn rewrites_container_create_body_and_preserves_keep_alive_leftover() {
        let body = r#"{"Image":"alpine","HostConfig":{"Binds":["C:\\work\\app:/app:rw"]}}"#;
        let req1 = format!(
            "POST /v1.45/containers/create HTTP/1.1\r\nHost: docker\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let req2 = "GET /_ping HTTP/1.1\r\nHost: docker\r\n\r\n";
        let combined = format!("{req1}{req2}");
        let mut buf = combined.as_bytes()[..20].to_vec();
        let (mut client, mut writer) = tokio::io::duplex(2048);
        writer.write_all(&combined.as_bytes()[20..]).await.unwrap();
        drop(writer);

        let PreparedOutcome::Framed(prep) = prepare_request_with_leftover(&mut client, &mut buf, &[])
            .await
            .unwrap()
        else {
            panic!("expected framed outcome");
        };
        let text = String::from_utf8(prep.payload).unwrap();
        assert!(text.contains("/mnt/c/work/app:/app:rw"));
        assert!(text.contains("Connection: close\r\n"));
        assert_eq!(String::from_utf8(prep.leftover).unwrap(), req2);
    }

    #[tokio::test]
    async fn relay_strips_connection_close_for_keep_alive_client() {
        let (mut client_read, mut client_write) = tokio::io::duplex(1024);
        let (mut up_read, mut up_write) = tokio::io::duplex(1024);

        up_write
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
            .await
            .unwrap();
        drop(up_write);

        let keep_alive = relay_upstream_response(&mut client_write, &mut up_read, false)
            .await
            .unwrap();
        assert!(keep_alive);
        drop(client_write);

        let mut out = String::new();
        client_read.read_to_string(&mut out).await.unwrap();
        assert!(!out.contains("Connection: close"));
        assert!(out.ends_with("\r\n\r\nOK"));
    }
}
