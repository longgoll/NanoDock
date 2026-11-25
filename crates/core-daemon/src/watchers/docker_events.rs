use std::sync::Arc;
use std::time::Duration;

use shared_types::{DaemonEvent, EngineState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, info, warn};
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::watchers::traits::{DockerEventHook, DockerEventPayload};

/// Subscribes to Docker `GET /events` whenever the engine is `Running` and fans out
/// parsed events to all registered `DockerEventHook`s without waking a sleeping VM.
pub async fn run_event_watchers<E: WslExecutor + 'static>(
    ctx: Arc<AppContext<E>>,
    hooks: Vec<Arc<dyn DockerEventHook>>,
) {
    let mut rx = ctx.events.subscribe();

    loop {
        if *ctx.state.read().await != EngineState::Running {
            tokio::select! {
                _ = ctx.shutdown.cancelled() => break,
                msg = rx.recv() => {
                    if let Ok(DaemonEvent::StateChanged(EngineState::Running)) = msg {
                        // Proceed to subscribe to /events.
                    }
                    continue;
                }
            }
        }

        if let Err(err) = stream_events(&ctx, &hooks).await {
            debug!(error = %err, "Docker /events stream ended");
        }

        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }

    info!("Shutting down Docker /events watcher");
}

async fn stream_events<E: WslExecutor>(
    ctx: &AppContext<E>,
    hooks: &[Arc<dyn DockerEventHook>],
) -> anyhow::Result<()> {
    let mut stream = ctx.transport.open_docker_stream().await?;
    stream
        .write_all(b"GET /events HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await?;

    let mut lines = BufReader::new(stream).lines();
    // Skip HTTP response headers until the empty line.
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            break;
        }
    }

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => break,
            line_res = lines.next_line() => {
                let Some(line) = line_res? else { break; };
                let trimmed = line.trim();
                // Ignore HTTP chunk-size hex lines or empty keepalives.
                if trimmed.is_empty() || !trimmed.starts_with('{') {
                    continue;
                }
                match serde_json::from_str::<DockerEventPayload>(trimmed) {
                    Ok(ev) => {
                        for hook in hooks {
                            debug!(hook = hook.name(), action = %ev.action, "Dispatching Docker event");
                            hook.on_event(&ev).await;
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, "Failed to parse Docker event JSON");
                    }
                }
            }
        }
    }

    Ok(())
}
