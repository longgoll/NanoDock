use serde::{Deserialize, Serialize};

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

/// Layer entry returned when inspecting image history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageLayer {
    pub id: String,
    pub created: i64,
    pub created_by: String,
    pub size: i64,
    pub comment: String,
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

/// Summary of a Docker network returned to the UI/CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkSummary {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    #[serde(default)]
    pub internal: bool,
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

/// File or directory entry inside a container filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub mode: String,
    pub modified: i64,
}

/// Information about a user WSL2 distribution for Docker socket integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WslDistroInfo {
    pub name: String,
    pub state: String,
    pub version: u8,
    pub is_default: bool,
    pub integrated: bool,
}

/// Runtime resource-saving metrics tracked by `core-daemon`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineMetrics {
    pub hibernate_count: u64,
    pub total_reclaimed_bytes: u64,
    pub uptime_secs: u64,
}

/// Summary of Docker BuildKit / Builder cache usage returned by `/system/df`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildCacheSummary {
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub record_count: usize,
}
