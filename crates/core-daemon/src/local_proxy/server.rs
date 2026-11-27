use std::sync::Arc;
use std::time::Duration;

use shared_types::{EngineState, LocalProxyRoute};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{info, warn};
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::lifecycle::docker_api::list_containers;
use crate::local_proxy::router::{extract_host_header, resolve_routes_from_containers};

const ROUTE_FALLBACK_INTERVAL: Duration = Duration::from_secs(30);

/// Runs the lightweight `*.localhost` HTTP/WebSocket TCP reverse proxy on `127.0.0.1:<port>`.
pub async fn run_local_proxy_server<E: WslExecutor + 'static>(ctx: Arc<AppContext<E>>) {
    let (enabled, port) = {
        let cfg = ctx.config.read().await;
        (cfg.local_proxy_enabled, cfg.local_proxy_port)
    };
    if !enabled || port == 0 {
        info!("Local *.localhost reverse proxy is disabled");
        return;
    }

    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(err) => {
            warn!(port, error = %err, "Failed to bind local *.localhost proxy; continuing without it");
            return;
        }
    };

    info!(port, "Local *.localhost reverse proxy listening");

    let routes_ref = Arc::clone(&ctx.local_proxy_routes);
    let refresh_ctx = Arc::clone(&ctx);
    let refresh_task = tokio::spawn(async move {
        refresh_routes_loop(refresh_ctx, routes_ref).await;
    });

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => {
                info!("Shutting down local *.localhost reverse proxy");
                break;
            }
            res = listener.accept() => {
                match res {
                    Ok((stream, _)) => {
                        let routes = Arc::clone(&ctx.local_proxy_routes);
                        tokio::spawn(async move {
                            let _ = handle_proxy_connection(stream, routes).await;
                        });
                    }
                    Err(err) => {
                        warn!(error = %err, "Local proxy accept error");
                        break;
                    }
                }
            }
        }
    }

    refresh_task.abort();
}

async fn refresh_routes_loop<E: WslExecutor>(
    ctx: Arc<AppContext<E>>,
    routes: Arc<RwLock<Vec<LocalProxyRoute>>>,
) {
    let mut interval = tokio::time::interval(ROUTE_FALLBACK_INTERVAL);
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            _ = ctx.local_proxy_notify.notified() => {}
            _ = interval.tick() => {}
        }
        if *ctx.state.read().await != EngineState::Running {
            let mut guard = routes.write().await;
            if !guard.is_empty() {
                guard.clear();
            }
            continue;
        }
        if let Ok(containers) = list_containers(&ctx.transport).await {
            let next = resolve_routes_from_containers(&containers);
            *routes.write().await = next;
        }
    }
}

async fn handle_proxy_connection(
    mut client: TcpStream,
    routes: Arc<RwLock<Vec<LocalProxyRoute>>>,
) -> anyhow::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    let read_res = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let n = client.read(&mut tmp).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() >= 16_384 {
                break;
            }
        }
        Ok::<(), std::io::Error>(())
    })
    .await;

    if read_res.is_err() || buf.is_empty() {
        return Ok(());
    }

    let host = extract_host_header(&buf).unwrap_or_default();
    let route_snapshot = routes.read().await.clone();

    if let Some(matched) = route_snapshot.iter().find(|r| r.host == host) {
        match TcpStream::connect(("127.0.0.1", matched.target_port)).await {
            Ok(mut upstream) => {
                upstream.write_all(&buf).await?;
                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            }
            Err(err) => {
                let body = format!(
                    "NanoDock Local Proxy: upstream 127.0.0.1:{} unreachable ({err})",
                    matched.target_port
                );
                write_http_response(&mut client, "502 Bad Gateway", "text/plain", &body).await?;
            }
        }
        return Ok(());
    }

    if host == "localhost" || host == "127.0.0.1" {
        let json = serde_json::to_string_pretty(&route_snapshot).unwrap_or_else(|_| "[]".into());
        write_http_response(&mut client, "200 OK", "application/json", &json).await?;
    } else {
        let body = format!(
            "NanoDock Local Proxy: no running container matched host '{host}'.\nActive routes: {}",
            route_snapshot.len()
        );
        write_http_response(&mut client, "404 Not Found", "text/plain", &body).await?;
    }
    Ok(())
}

async fn write_http_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> anyhow::Result<()> {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}
