use serde::{Deserialize, Serialize};

/// Persistent configuration for the NanoDock daemon and WSL2 runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineConfig {
    pub distro_name: String,
    pub idle_timeout_secs: Option<u64>,
    pub auto_start_on_login: bool,
    pub auto_trim_on_events: bool,
    pub wsl: WslConfigSpec,
    #[serde(default)]
    pub turbo_mounts: Vec<TurboMountRule>,
    #[serde(default = "default_local_proxy_enabled")]
    pub local_proxy_enabled: bool,
    #[serde(default = "default_local_proxy_port")]
    pub local_proxy_port: u16,
}

fn default_local_proxy_enabled() -> bool {
    true
}

fn default_local_proxy_port() -> u16 {
    8090
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            distro_name: "nanodock".to_string(),
            idle_timeout_secs: Some(180),
            auto_start_on_login: false,
            auto_trim_on_events: true,
            wsl: WslConfigSpec::default(),
            turbo_mounts: Vec::new(),
            local_proxy_enabled: true,
            local_proxy_port: 8090,
        }
    }
}

/// Bind-mount acceleration rule syncing a Windows folder into native WSL2 ext4.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurboMountRule {
    pub id: String,
    pub windows_path: String,
    pub wsl_path: String,
    #[serde(default = "default_turbo_excludes")]
    pub excludes: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

pub fn default_turbo_excludes() -> Vec<String> {
    vec![
        "node_modules".to_string(),
        ".git".to_string(),
        "target".to_string(),
        "dist".to_string(),
        ".next".to_string(),
        "__pycache__".to_string(),
        ".venv".to_string(),
    ]
}

/// Resource limits and networking flags written to `%USERPROFILE%\.wslconfig`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WslConfigSpec {
    pub memory_mb: u64,
    pub swap_mb: u64,
    pub sparse_vhd: bool,
    pub mirrored_networking: bool,
    #[serde(default)]
    pub processors: Option<u32>,
    #[serde(default = "default_true")]
    pub page_reporting: bool,
}

impl Default for WslConfigSpec {
    fn default() -> Self {
        Self {
            memory_mb: 4096,
            swap_mb: 4096,
            sparse_vhd: true,
            mirrored_networking: true,
            processors: None,
            page_reporting: true,
        }
    }
}
