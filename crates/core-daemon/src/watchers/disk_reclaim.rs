use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use shared_types::DaemonEvent;
use tokio::sync::Notify;
use tracing::{info, warn};
use wsl_bridge::{trim_distro, WslExecutor};

use crate::app_context::AppContext;
use crate::watchers::traits::{DockerEventHook, DockerEventPayload};

const TRIM_DEBOUNCE: Duration = Duration::from_secs(5);

/// Triggers `fstrim -av` with a 5-second debounce after container `destroy`,
/// image `delete`, or `prune` events so `.vhdx` shrinks immediately.
pub struct DiskReclaimHook {
    trigger: Arc<Notify>,
}

impl DiskReclaimHook {
    pub fn spawn<E: WslExecutor + 'static>(ctx: Arc<AppContext<E>>) -> Arc<Self> {
        let trigger = Arc::new(Notify::new());
        let trigger_rx = Arc::clone(&trigger);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = ctx.shutdown.cancelled() => break,
                    _ = trigger_rx.notified() => {
                        // Debounce burst deletions for 5 seconds.
                        loop {
                            tokio::select! {
                                _ = ctx.shutdown.cancelled() => return,
                                _ = trigger_rx.notified() => continue,
                                _ = tokio::time::sleep(TRIM_DEBOUNCE) => break,
                            }
                        }

                        let (auto_trim, distro_name) = {
                            let cfg = ctx.config.read().await;
                            (cfg.auto_trim_on_events, cfg.distro_name.clone())
                        };
                        if !auto_trim {
                            continue;
                        }

                        info!(distro = %distro_name, "Running debounced fstrim -av");
                        match trim_distro(ctx.wsl.as_ref(), &distro_name).await {
                            Ok(output) => {
                                info!(output = %output, "Disk trim completed");
                                let _ = ctx.events.send(DaemonEvent::DiskTrimmed { reclaimed_bytes: None });
                            }
                            Err(err) => {
                                warn!(error = %err, "Disk trim failed");
                            }
                        }
                    }
                }
            }
        });

        Arc::new(Self { trigger })
    }
}

#[async_trait]
impl DockerEventHook for DiskReclaimHook {
    fn name(&self) -> &'static str {
        "disk_reclaim"
    }

    async fn on_event(&self, event: &DockerEventPayload) {
        if is_reclaim_trigger(event) {
            self.trigger.notify_one();
        }
    }
}

/// Returns `true` for Docker events that free disk blocks inside the WSL2 ext4 filesystem.
pub fn is_reclaim_trigger(event: &DockerEventPayload) -> bool {
    matches!(
        (event.kind.as_str(), event.action.as_str()),
        ("container", "destroy")
            | ("image", "delete")
            | ("volume", "prune")
            | ("builder", "prune")
            | ("system", "prune")
    ) || event.action == "prune"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_destroy_delete_and_prune_events() {
        assert!(is_reclaim_trigger(&DockerEventPayload {
            kind: "container".into(),
            action: "destroy".into(),
        }));
        assert!(is_reclaim_trigger(&DockerEventPayload {
            kind: "image".into(),
            action: "delete".into(),
        }));
        assert!(is_reclaim_trigger(&DockerEventPayload {
            kind: "volume".into(),
            action: "prune".into(),
        }));
        assert!(!is_reclaim_trigger(&DockerEventPayload {
            kind: "container".into(),
            action: "start".into(),
        }));
    }
}
