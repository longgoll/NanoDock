use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentStatus {
    pub wsl_installed: bool,
    pub wsl_status_detail: String,
    pub install_dir: String,
    pub nanodock_cli_available: bool,
    pub nanodock_in_path: bool,
    pub docker_shim_active: bool,
    pub external_docker_path: Option<String>,
    pub creds_store_warning: Option<String>,
    pub wincred_available: bool,
}

fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn current_install_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Failed to locate executable: {e}"))?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Failed to resolve executable directory".to_string())
}

fn paths_equal_ci(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_ascii_lowercase();
    norm(a) == norm(b)
}

fn check_wsl_status() -> (bool, String) {
    match hidden_command("wsl.exe").arg("--status").output() {
        Ok(out) => {
            let stdout = wsl_bridge::decode_wsl_output(&out.stdout);
            let stderr = wsl_bridge::decode_wsl_output(&out.stderr);
            let detail = if !stdout.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            (out.status.success(), detail)
        }
        Err(err) => (false, format!("wsl.exe not available: {err}")),
    }
}

fn read_user_path_entries() -> Vec<String> {
    let script = "[Environment]::GetEnvironmentVariable('Path', 'User')";
    let Ok(out) = hidden_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn write_user_path_entries(entries: &[String]) -> Result<(), String> {
    let joined = entries.join(";");
    let escaped = joined.replace('\'', "''");
    let script = format!("[Environment]::SetEnvironmentVariable('Path', '{escaped}', 'User')");
    let out = hidden_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("Failed to update User PATH: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Failed to update User PATH: {}", stderr.trim()));
    }
    Ok(())
}

fn find_external_docker_exe(install_dir: &Path) -> Option<String> {
    let install_str = install_dir.to_string_lossy();
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if paths_equal_ci(&dir.to_string_lossy(), &install_str) {
            continue;
        }
        let candidate = dir.join("docker.exe");
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn is_nanodock_docker_shim(install_dir: &Path) -> bool {
    let shim = install_dir.join("docker.exe");
    let cli = install_dir.join("nanodock.exe");
    if !shim.is_file() {
        return false;
    }
    match (std::fs::metadata(&shim), std::fs::metadata(&cli)) {
        (Ok(s_meta), Ok(c_meta)) => s_meta.len() == c_meta.len(),
        _ => false,
    }
}

fn apply_user_path(install_dir: &Path, enabled: bool) -> Result<(), String> {
    let target = install_dir.to_string_lossy().into_owned();
    let mut entries = read_user_path_entries();
    let already_present = entries.iter().any(|e| paths_equal_ci(e, &target));

    if enabled && !already_present {
        entries.push(target);
        write_user_path_entries(&entries)?;
    } else if !enabled && already_present {
        entries.retain(|e| !paths_equal_ci(e, &target));
        write_user_path_entries(&entries)?;
    }
    Ok(())
}

fn apply_docker_shim(install_dir: &Path, enabled: bool) -> Result<(), String> {
    let cli = install_dir.join("nanodock.exe");
    let shim = install_dir.join("docker.exe");

    if enabled {
        if !cli.is_file() {
            return Err("nanodock.exe was not found in the application folder.".to_string());
        }
        if is_nanodock_docker_shim(install_dir) {
            return Ok(());
        }
        if shim.exists() {
            return Err("A non-NanoDock docker.exe already exists in this folder.".to_string());
        }
        if std::fs::hard_link(&cli, &shim).is_err() {
            std::fs::copy(&cli, &shim)
                .map_err(|e| format!("Failed to create docker.exe shim: {e}"))?;
        }
    } else if is_nanodock_docker_shim(install_dir) {
        std::fs::remove_file(&shim)
            .map_err(|e| format!("Failed to remove docker.exe shim: {e}"))?;
    }
    Ok(())
}

pub fn collect_environment_status() -> Result<EnvironmentStatus, String> {
    let install_dir = current_install_dir()?;
    let install_dir_str = install_dir.to_string_lossy().into_owned();
    let (wsl_installed, wsl_status_detail) = check_wsl_status();
    let nanodock_cli_available = install_dir.join("nanodock.exe").is_file();
    let nanodock_in_path = read_user_path_entries()
        .iter()
        .any(|e| paths_equal_ci(e, &install_dir_str));
    let docker_shim_active = is_nanodock_docker_shim(&install_dir);
    let external_docker_path = find_external_docker_exe(&install_dir);
    let creds_store_warning = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .and_then(|profile| wsl_bridge::check_user_docker_config(&profile))
        .map(|w| w.message);
    let wincred_available = wsl_bridge::detect_wincred_available();

    Ok(EnvironmentStatus {
        wsl_installed,
        wsl_status_detail,
        install_dir: install_dir_str,
        nanodock_cli_available,
        nanodock_in_path,
        docker_shim_active,
        external_docker_path,
        creds_store_warning,
        wincred_available,
    })
}

/// On first launch of an installed/bundled NanoDock build (where `nanodock.exe` sits beside
/// `NanoDock.exe`), automatically add NanoDock to User `PATH` and create the `docker.exe` shim
/// if no external `docker.exe` is already installed. A marker file ensures user overrides in
/// Settings are respected on subsequent launches.
pub fn auto_configure_cli_on_first_run() {
    let Ok(install_dir) = current_install_dir() else {
        return;
    };
    if !install_dir.join("nanodock.exe").is_file() {
        return;
    }

    let marker_dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| install_dir.clone())
        .join("NanoDock");
    let marker = marker_dir.join(".cli-auto-configured");
    if marker.exists() {
        return;
    }

    let _ = apply_user_path(&install_dir, true);
    if find_external_docker_exe(&install_dir).is_none() {
        let _ = apply_docker_shim(&install_dir, true);
    }

    let _ = std::fs::create_dir_all(&marker_dir);
    let _ = std::fs::write(&marker, "1");
}

#[tauri::command]
pub async fn get_environment_status() -> Result<EnvironmentStatus, String> {
    tokio::task::spawn_blocking(collect_environment_status)
        .await
        .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn install_wsl2() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        let script =
            "Start-Process wsl.exe -ArgumentList '--install','--no-distribution' -Verb RunAs";
        let out = hidden_command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|e| format!("Failed to launch WSL2 installer: {e}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(format!("WSL2 installation cancelled or failed: {}", stderr.trim()));
        }
        Ok("WSL2 installation started. Complete the UAC prompt and restart Windows if requested.".to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn set_user_path_integration(enabled: bool) -> Result<EnvironmentStatus, String> {
    tokio::task::spawn_blocking(move || {
        let install_dir = current_install_dir()?;
        apply_user_path(&install_dir, enabled)?;
        collect_environment_status()
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn set_docker_cli_shim(enabled: bool) -> Result<EnvironmentStatus, String> {
    tokio::task::spawn_blocking(move || {
        let install_dir = current_install_dir()?;
        apply_docker_shim(&install_dir, enabled)?;
        collect_environment_status()
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn fix_docker_creds_store() -> Result<EnvironmentStatus, String> {
    tokio::task::spawn_blocking(|| {
        let profile = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| "USERPROFILE environment variable is not set".to_string())?;
        wsl_bridge::fix_user_docker_creds_store(&profile)
            .map_err(|e| format!("Failed to update Docker config.json: {e}"))?;
        collect_environment_status()
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}
