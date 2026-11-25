use async_trait::async_trait;
use serde::Deserialize;

/// Subset of a Docker `/events` JSON line needed by event hooks.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DockerEventPayload {
    #[serde(rename = "Type", default)]
    pub kind: String,
    #[serde(rename = "Action", default)]
    pub action: String,
}

/// Extensible hook interface invoked when `dockerd` emits an event on `GET /events`.
#[async_trait]
pub trait DockerEventHook: Send + Sync {
    fn name(&self) -> &'static str;
    async fn on_event(&self, event: &DockerEventPayload);
}
