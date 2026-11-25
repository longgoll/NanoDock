use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;
use shared_types::EngineConfig;
use tracing::warn;

const WIN11_MIN_BUILD: u32 = 22_000;

/// Returns `%LOCALAPPDATA%\NanoDock\config.json` (or `./NanoDock/config.json` as fallback).
pub fn default_config_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("NanoDock")
        .join("config.json")
}

/// Returns `%USERPROFILE%\.wslconfig` when `USERPROFILE` is set.
pub fn wslconfig_path() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".wslconfig"))
}

/// Loads `EngineConfig` from `path`, falling back to `EngineConfig::default()` if missing or invalid.
pub fn load_or_default(path: &Path) -> EngineConfig {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return EngineConfig::default();
    };
    match serde_json::from_str::<EngineConfig>(&raw) {
        Ok(cfg) => cfg,
        Err(err) => {
            warn!(path = %path.display(), error = %err, "Failed to parse config.json, using defaults");
            EngineConfig::default()
        }
    }
}

/// Atomically persists `config` to `path` as pretty-printed JSON.
pub fn save_config(path: &Path, config: &EngineConfig) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config dir {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(config)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)
        .with_context(|| format!("failed to write temp config {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("failed to replace config {}", path.display()))?;
    Ok(())
}

/// Returns `true` on Windows 11+ (build >= 22000) where `networkingMode=mirrored` and
/// `autoMemoryReclaim` are supported by WSL2.
pub fn supports_win11_wsl2_features() -> bool {
    detect_windows_build_number()
        .map(|build| build >= WIN11_MIN_BUILD)
        .unwrap_or(false)
}

fn detect_windows_build_number() -> Option<u32> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/c", "ver"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse_windows_build_number(&text)
}

/// Parses the Windows build number from `cmd /c ver` output,
/// e.g. `Microsoft Windows [Version 10.0.22631.4460]` -> `Some(22631)`.
pub fn parse_windows_build_number(ver_output: &str) -> Option<u32> {
    let start = ver_output.find('[')?;
    let end = ver_output[start..].find(']')? + start;
    let inside = &ver_output[start + 1..end];
    let version_token = inside.split_whitespace().last()?;
    let mut parts = version_token.split('.');
    let _major = parts.next()?;
    let _minor = parts.next()?;
    let build = parts.next()?;
    build.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_win10_and_win11_build_numbers() {
        assert_eq!(
            parse_windows_build_number("Microsoft Windows [Version 10.0.19045.5131]"),
            Some(19045)
        );
        assert_eq!(
            parse_windows_build_number("Microsoft Windows [Version 10.0.22631.4460]"),
            Some(22631)
        );
        assert_eq!(parse_windows_build_number("invalid"), None);
    }

    #[test]
    fn saves_and_loads_config_roundtrip() {
        let dir = std::env::temp_dir().join(format!("nanodock-cfg-{}", std::process::id()));
        let path = dir.join("config.json");

        let mut cfg = EngineConfig::default();
        cfg.idle_timeout_secs = Some(600);
        cfg.auto_start_on_login = true;
        cfg.wsl.memory_mb = 6144;

        save_config(&path, &cfg).unwrap();
        let loaded = load_or_default(&path);
        assert_eq!(loaded, cfg);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_corrupt_config_falls_back_to_default() {
        let dir = std::env::temp_dir().join(format!("nanodock-cfg-bad-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let missing = dir.join("missing.json");
        assert_eq!(load_or_default(&missing), EngineConfig::default());

        let corrupt = dir.join("corrupt.json");
        std::fs::write(&corrupt, "{not valid json").unwrap();
        assert_eq!(load_or_default(&corrupt), EngineConfig::default());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
