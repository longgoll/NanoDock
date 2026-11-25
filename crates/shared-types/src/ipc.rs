use serde::{Deserialize, Serialize};

use crate::{config::EngineConfig, events::EngineState};

/// Action to perform on a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerActionKind {
    Start,
    Stop,
    Restart,
    Remove,
}

/// Summary of a Docker container returned to the UI/CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSummary {
    pub id: String,
    pub names: Vec<String>,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    pub ports: Vec<String>,
    pub compose_project: Option<String>,
}

/// Summary of a Docker image returned to the UI/CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageSummary {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: i64,
    pub created: i64,
    #[serde(default)]
    pub in_use: bool,
    #[serde(default)]
    pub used_by: Vec<String>,
}

/// Summary of a Docker volume returned to the UI/CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created_at: Option<String>,
    #[serde(default)]
    pub in_use: bool,
    #[serde(default)]
    pub used_by: Vec<String>,
}

/// Mount entry inside a container inspect payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerMount {
    pub mount_type: String,
    pub source: String,
    pub destination: String,
    pub mode: String,
}

/// Detailed container inspection payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerInspect {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: String,
    pub restart_policy: String,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub env: Vec<String>,
    pub mounts: Vec<ContainerMount>,
    pub networks: Vec<String>,
    pub ip_address: String,
    pub ports: Vec<String>,
}

/// Live CPU and memory usage snapshot for a running container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerStats {
    pub id: String,
    pub cpu_percent: f64,
    pub mem_usage_bytes: u64,
    pub mem_limit_bytes: u64,
}

/// Parameters to create and start a container from an image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Vec<String>,
    pub env: Vec<String>,
}

/// Requests sent from the Tauri UI or CLI to `core-daemon` over `\\.\pipe\nanodock_ctl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]
pub enum ControlRequest {
    GetStatus,
    StartEngine,
    StopEngine,
    TrimDisk,
    GetConfig,
    UpdateConfig(EngineConfig),
    ListContainers,
    ContainerAction {
        id: String,
        action: ContainerActionKind,
    },
    GetContainerLogs {
        id: String,
        tail: Option<usize>,
    },
    InspectContainer {
        id: String,
    },
    GetContainerStats {
        id: String,
    },
    RunContainer(RunContainerSpec),
    ListImages,
    PullImage {
        reference: String,
    },
    RemoveImage {
        id: String,
    },
    PruneImages,
    ListVolumes,
    CreateVolume {
        name: String,
        driver: Option<String>,
    },
    RemoveVolume {
        name: String,
    },
    PruneVolumes,
}

/// Responses returned by `core-daemon` over the control pipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum ControlResponse {
    Status {
        state: EngineState,
        running_containers: usize,
        vhdx_size_bytes: Option<u64>,
    },
    Config(EngineConfig),
    Containers(Vec<ContainerSummary>),
    ContainerLogs {
        logs: String,
    },
    ContainerInspect(ContainerInspect),
    ContainerStats(ContainerStats),
    ContainerCreated {
        id: String,
    },
    Images(Vec<ImageSummary>),
    Volumes(Vec<VolumeSummary>),
    PruneResult {
        reclaimed_bytes: u64,
    },
    Ack,
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_request_roundtrips_with_tagged_json() {
        let req = ControlRequest::UpdateConfig(EngineConfig::default());
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""cmd":"update_config""#));
        let parsed: ControlRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            ControlRequest::UpdateConfig(cfg) => assert_eq!(cfg, EngineConfig::default()),
            other => panic!("unexpected variant: {other:?}"),
        }

        let status_req = serde_json::to_string(&ControlRequest::GetStatus).unwrap();
        assert_eq!(status_req, r#"{"cmd":"get_status"}"#);
    }

    #[test]
    fn control_response_roundtrips_with_tagged_json() {
        let resp = ControlResponse::Status {
            state: EngineState::Running,
            running_containers: 3,
            vhdx_size_bytes: Some(1024),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""status":"status""#));
        let parsed: ControlResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            ControlResponse::Status {
                state,
                running_containers,
                vhdx_size_bytes,
            } => {
                assert_eq!(state, EngineState::Running);
                assert_eq!(running_containers, 3);
                assert_eq!(vhdx_size_bytes, Some(1024));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
