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
}

/// Summary of a Docker volume returned to the UI/CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created_at: Option<String>,
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
    ListImages,
    PullImage {
        reference: String,
    },
    RemoveImage {
        id: String,
    },
    PruneImages,
    ListVolumes,
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
