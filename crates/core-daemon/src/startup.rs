use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;

const RUN_KEY_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE_NAME: &str = "NanoDock";

/// Abstraction over OS login auto-start registration for unit testability.
pub trait StartupManager: Send + Sync {
    fn set_enabled(&self, enabled: bool) -> anyhow::Result<()>;
}

/// No-op startup manager for unit tests.
pub struct NoopStartup;

impl StartupManager for NoopStartup {
    fn set_enabled(&self, _enabled: bool) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Registers or removes NanoDock in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
pub struct WindowsRegistryStartup;

impl StartupManager for WindowsRegistryStartup {
    fn set_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        if enabled {
            let exe = std::env::current_exe().context("failed to resolve current executable")?;
            let target = resolve_startup_target(&exe);
            let args = build_reg_add_args(&target);
            run_reg(&args)
        } else {
            let args = build_reg_delete_args();
            // `reg delete` exits non-zero when the value does not exist; ignore that case.
            let _ = run_reg(&args);
            Ok(())
        }
    }
}

/// Prefers `NanoDock.exe` alongside `core-daemon.exe` so the tray + daemon launch together.
pub fn resolve_startup_target(current_exe: &Path) -> PathBuf {
    if let Some(dir) = current_exe.parent() {
        let ui_exe = dir.join("NanoDock.exe");
        if ui_exe.exists() {
            return ui_exe;
        }
    }
    current_exe.to_path_buf()
}

fn build_reg_add_args(target: &Path) -> Vec<String> {
    let quoted = format!("\"{}\"", target.display());
    vec![
        "add".to_string(),
        RUN_KEY_PATH.to_string(),
        "/v".to_string(),
        RUN_VALUE_NAME.to_string(),
        "/t".to_string(),
        "REG_SZ".to_string(),
        "/d".to_string(),
        quoted,
        "/f".to_string(),
    ]
}

fn build_reg_delete_args() -> Vec<String> {
    vec![
        "delete".to_string(),
        RUN_KEY_PATH.to_string(),
        "/v".to_string(),
        RUN_VALUE_NAME.to_string(),
        "/f".to_string(),
    ]
}

fn run_reg(args: &[String]) -> anyhow::Result<()> {
    let mut cmd = Command::new("reg.exe");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().context("failed to execute reg.exe")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("reg.exe failed ({}): {}", out.status, stderr.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_registry_add_and_delete_args() {
        let add = build_reg_add_args(Path::new(r"C:\Program Files\NanoDock\NanoDock.exe"));
        assert_eq!(add[0], "add");
        assert_eq!(add[1], RUN_KEY_PATH);
        assert_eq!(add[3], RUN_VALUE_NAME);
        assert_eq!(add[7], r#""C:\Program Files\NanoDock\NanoDock.exe""#);

        let del = build_reg_delete_args();
        assert_eq!(del[0], "delete");
        assert_eq!(del[1], RUN_KEY_PATH);
        assert_eq!(del[3], RUN_VALUE_NAME);
    }

    #[test]
    fn resolves_fallback_to_current_exe_when_ui_missing() {
        let current = PathBuf::from(r"C:\nonexistent-nanodock-dir\core-daemon.exe");
        assert_eq!(resolve_startup_target(&current), current);
    }
}
