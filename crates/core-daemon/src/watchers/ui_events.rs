use std::sync::Arc;

use async_trait::async_trait;
use shared_types::DaemonEvent;
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::watchers::traits::{DockerEventHook, DockerEventPayload};

/// Broadcasts relevant Docker lifecycle events (`container`, `image`, `volume`, `network`)
/// over `AppContext::events` so subscribed UI and tray clients refresh immediately.
pub struct UiEventHook<E: WslExecutor> {
    ctx: Arc<AppContext<E>>,
}

impl<E: WslExecutor> UiEventHook<E> {
    pub fn new(ctx: Arc<AppContext<E>>) -> Arc<Self> {
        Arc::new(Self { ctx })
    }
}

#[async_trait]
impl<E: WslExecutor + 'static> DockerEventHook for UiEventHook<E> {
    fn name(&self) -> &'static str {
        "ui_event_hook"
    }

    async fn on_event(&self, event: &DockerEventPayload) {
        let relevant = matches!(
            (event.kind.as_str(), event.action.as_str()),
            (
                "container",
                "create" | "start" | "stop" | "die" | "kill" | "restart" | "pause" | "unpause"
                    | "destroy" | "rename"
            ) | ("image", "pull" | "import" | "load" | "tag" | "untag" | "delete" | "prune")
                | ("volume", "create" | "destroy" | "prune")
                | ("network", "create" | "destroy" | "connect" | "disconnect" | "prune")
        );

        if relevant {
            if event.kind == "container" {
                crate::lifecycle::docker_api::invalidate_containers_cache();
                self.ctx.local_proxy_notify.notify_one();
            }
            let _ = self.ctx.events.send(DaemonEvent::DockerChanged {
                resource: event.kind.clone(),
                action: event.action.clone(),
            });
        }
    }
}
