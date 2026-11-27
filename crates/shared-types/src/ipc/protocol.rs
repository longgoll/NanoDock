use serde::{Deserialize, Serialize};

use crate::{config::EngineConfig, events::EngineState};

use super::models::{
    BuildCacheSummary, ContainerActionKind, ContainerFileEntry, ContainerInspect, ContainerStats,
    ContainerSummary, EngineMetrics, ImageLayer, ImageSummary, NetworkSummary, RunContainerSpec,
    VolumeSummary, WslDistroInfo,
};

/// Requests sent from the Tauri UI or CLI to `core-daemon` over `\\.\pipe\nanodock_ctl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]
pub enum ControlRequest {
    GetStatus,
    SubscribeEvents,
    StartEngine,
    StopEngine,
    ShutdownDaemon,
    TrimDisk,
    GetConfig,
    UpdateConfig(EngineConfig),
    GetEngineLogs {
        lines: usize,
    },
    GetEngineMetrics,
    SetKeepAwake {
        enabled: bool,
    },
    ListWslDistros,
    SetWslDistroIntegration {
        distro: String,
        enabled: bool,
    },
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
    ExecContainer {
        id: String,
        command: Vec<String>,
    },
    StartExecSession {
        id: String,
        shell: Option<String>,
        cols: u16,
        rows: u16,
    },
    StreamContainerLogs {
        id: String,
        tail: usize,
    },
    StreamContainerStats {
        id: String,
    },
    ListContainerFiles {
        id: String,
        path: String,
    },
    ReadContainerFile {
        id: String,
        path: String,
    },
    RunContainer(RunContainerSpec),
    ListImages,
    PullImage {
        reference: String,
    },
    StreamPullImage {
        reference: String,
    },
    RemoveImage {
        id: String,
    },
    PruneImages,
    GetBuildCache,
    PruneBuildCache,
    InspectImageLayers {
        id: String,
    },
    ListVolumes,
    CreateVolume {
        name: String,
        driver: Option<String>,
    },
    RemoveVolume {
        name: String,
    },
    PruneVolumes,
    ExportVolume {
        name: String,
        dest_path: String,
    },
    ImportVolume {
        name: String,
        src_path: String,
    },
    ListNetworks,
    CreateNetwork {
        name: String,
        driver: Option<String>,
        internal: bool,
    },
    RemoveNetwork {
        id: String,
    },
    PruneNetworks,
}

/// Responses returned by `core-daemon` over the control pipe.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum ControlResponse {
    Status {
        state: EngineState,
        running_containers: usize,
        vhdx_size_bytes: Option<u64>,
    },
    Config(EngineConfig),
    EngineLogs {
        daemon_log: String,
        dockerd_log: String,
    },
    EngineMetrics(EngineMetrics),
    WslDistros(Vec<WslDistroInfo>),
    Containers(Vec<ContainerSummary>),
    ContainerLogs {
        logs: String,
    },
    ContainerInspect(ContainerInspect),
    ContainerStats(ContainerStats),
    ExecOutput {
        exit_code: i64,
        output: String,
    },
    ContainerFiles(Vec<ContainerFileEntry>),
    ContainerFileContent {
        path: String,
        content: String,
        truncated: bool,
    },
    ContainerCreated {
        id: String,
    },
    Images(Vec<ImageSummary>),
    BuildCache(BuildCacheSummary),
    ImageLayers(Vec<ImageLayer>),
    Volumes(Vec<VolumeSummary>),
    Networks(Vec<NetworkSummary>),
    PruneResult {
        reclaimed_bytes: u64,
    },
    Ack,
    Error {
        message: String,
    },
}

/// Frames sent from a Tauri/CLI client to `core-daemon` during an interactive exec session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecClientFrame {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
    Close,
}

/// Frames sent from `core-daemon` to a Tauri/CLI client during an interactive exec session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecServerFrame {
    Started { exec_id: String },
    Output { data: String },
    Exit { code: i64 },
    Error { message: String },
}

/// Frames sent during real-time container log streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogStreamFrame {
    Line { text: String },
    Error { message: String },
    Ended,
}

/// Frames sent during real-time Docker image pull streaming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PullProgressFrame {
    Progress {
        id: Option<String>,
        status: String,
        progress: Option<String>,
        current: Option<u64>,
        total: Option<u64>,
    },
    Done,
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
