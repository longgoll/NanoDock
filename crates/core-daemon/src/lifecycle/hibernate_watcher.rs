use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use shared_types::EngineState;
use tracing::{debug, info, warn};
use wsl_bridge::WslExecutor;

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
                    ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
                    idle_since = None;
                    continue;
                }

                if ctx.keep_awake.load(Ordering::Relaxed) {
                    ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
                    idle_since = None;
                    continue;
                }

                let (distro_name, idle_timeout) = {
                    let cfg = ctx.config.read().await;
                    (cfg.distro_name.clone(), cfg.idle_timeout_secs)
                };

                let Some(timeout_secs) = idle_timeout else {
                    ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
                    idle_since = None;
                    continue;
                };

                let active_streams = ctx.active_streams.load(Ordering::SeqCst);
                if active_streams > 0 {
                    ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
                    idle_since = None;
                    continue;
                }

                match count_running_containers(&ctx.transport).await {
                    Ok(0) => {
                        let start = idle_since.get_or_insert_with(Instant::now);
                        let elapsed = start.elapsed();
                        let remaining = timeout_secs.saturating_sub(elapsed.as_secs());
                        ctx.idle_remaining_secs.store(remaining, Ordering::Relaxed);
                        debug!(elapsed_secs = elapsed.as_secs(), remaining, "Engine idle");
                        if elapsed >= Duration::from_secs(timeout_secs) {
                            info!(distro = %distro_name, "Idle timeout reached; hibernating WSL2 distro");
                            ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
                            if let Err(err) = ctx.transport.hibernate(&distro_name).await {
                                warn!(error = %err, "Graceful hibernate failed");
                            } else {
                                ctx.hibernate_count.fetch_add(1, Ordering::Relaxed);
                            }
                            idle_since = None;
                        }
                    }
                    Ok(n) => {
                        debug!(running = n, "Containers running; resetting idle timer");
                        ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
                        idle_since = None;
                    }
                    Err(err) => {
                        debug!(error = %err, "Failed to query running containers");
                        ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
                        idle_since = None;
                    }
                }
            }
        }
    }
}
