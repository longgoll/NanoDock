use serde::{Deserialize, Serialize};

use crate::ipc::TurboMountStatus;

/// Lifecycle state of the WSL2 micro-distro engine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

/// Real-time status events emitted by `core-daemon` to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum DaemonEvent {
    StateChanged(EngineState),
    DockerChanged { resource: String, action: String },
    DiskTrimmed { reclaimed_bytes: Option<u64> },
    TurboMountUpdated(TurboMountStatus),
    Warning { message: String },
}
