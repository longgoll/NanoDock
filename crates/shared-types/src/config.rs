use serde::{Deserialize, Serialize};

/// Persistent configuration for the NanoDock daemon and WSL2 runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineConfig {
    pub distro_name: String,
    pub idle_timeout_secs: Option<u64>,
    pub auto_start_on_login: bool,
    pub auto_trim_on_events: bool,
    pub wsl: WslConfigSpec,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            distro_name: "nanodock".to_string(),
            idle_timeout_secs: Some(180),
            auto_start_on_login: false,
            auto_trim_on_events: true,
            wsl: WslConfigSpec::default(),
        }
    }
}

/// Resource limits and networking flags written to `%USERPROFILE%\.wslconfig`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WslConfigSpec {
    pub memory_mb: u64,
    pub swap_mb: u64,
    pub sparse_vhd: bool,
    pub mirrored_networking: bool,
}

impl Default for WslConfigSpec {
    fn default() -> Self {
        Self {
            memory_mb: 4096,
            swap_mb: 4096,
            sparse_vhd: true,
            mirrored_networking: true,
        }
    }
}
