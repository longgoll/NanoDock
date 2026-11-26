use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use shared_types::EngineState;
use tracing::{debug, info, warn};
use wsl_bridge::{graceful_hibernate, WslExecutor};

use crate::app_context::AppContext;
use crate::lifecycle::docker_api::count_running_containers;

const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Periodically checks whether the WSL2 micro-distro is idle (0 running containers and
/// 0 active proxy streams) and gracefully hibernates it back to 0 MB RAM after `idle_timeout_secs`.
pub async fn run_hibernate_watcher<E: WslExecutor + 'static>(ctx: Arc<AppContext<E>>) {
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    let mut idle_since: Option<Instant> = None;

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => {
                info!("Shutting down Auto-Hibernate watcher");
                break;
            }
            _ = interval.tick() => {
                let state = *ctx.state.read().await;
                if state != EngineState::Running {
                    idle_since = None;
                    continue;
                }

                let (distro_name, idle_timeout) = {
                    let cfg = ctx.config.read().await;
                    (cfg.distro_name.clone(), cfg.idle_timeout_secs)
                };

                let Some(timeout_secs) = idle_timeout else {
                    idle_since = None;
                    continue;
                };

                let active_streams = ctx.active_streams.load(Ordering::SeqCst);
                if active_streams > 0 {
                    idle_since = None;
                    continue;
                }

                match count_running_containers(&ctx.transport).await {
                    Ok(0) => {
                        let start = idle_since.get_or_insert_with(Instant::now);
                        debug!(elapsed_secs = start.elapsed().as_secs(), "Engine idle");
                        if start.elapsed() >= Duration::from_secs(timeout_secs) {
                            info!(distro = %distro_name, "Idle timeout reached; hibernating WSL2 distro");
                            ctx.set_state(EngineState::Stopping).await;
                            ctx.transport.close_transport().await;
                            if let Err(err) = graceful_hibernate(ctx.wsl.as_ref(), &distro_name).await {
                                warn!(error = %err, "Graceful hibernate failed");
                                ctx.set_state(EngineState::Error).await;
                            } else {
                                ctx.hibernate_count.fetch_add(1, Ordering::Relaxed);
                                ctx.set_state(EngineState::Stopped).await;
                            }
                            idle_since = None;
                        }
                    }
                    Ok(n) => {
                        debug!(running = n, "Containers running; resetting idle timer");
                        idle_since = None;
                    }
                    Err(err) => {
                        debug!(error = %err, "Failed to query running containers");
                        idle_since = None;
                    }
                }
            }
        }
    }
}
