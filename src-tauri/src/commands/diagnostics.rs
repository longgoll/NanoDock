use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use super::system::{collect_environment_status, EnvironmentStatus};

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheckItem {
    pub name: String,
    pub level: String,
    pub detail: String,
}

fn hidden_ps(script: &str) -> Result<std::process::Output, String> {
    let mut cmd = Command::new("powershell.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
        .output()
        .map_err(|e| format!("Failed to invoke PowerShell: {e}"))
}

#[tauri::command]
pub async fn run_doctor_checks() -> Result<Vec<DoctorCheckItem>, String> {
    tokio::task::spawn_blocking(|| {
        let env = collect_environment_status()?;
        let mut checks = Vec::new();

        checks.push(DoctorCheckItem {
            name: "WSL2 Runtime".to_string(),
            level: if env.wsl_installed { "ok" } else { "fail" }.to_string(),
            detail: if env.wsl_installed {
                "WSL2 is installed and responding".to_string()
            } else {
                env.wsl_status_detail
            },
        });

        let daemon_ok = Path::new(r"\\.\pipe\nanodock_ctl").exists();
        checks.push(DoctorCheckItem {
            name: "Core Daemon".to_string(),
            level: if daemon_ok { "ok" } else { "warn" }.to_string(),
            detail: if daemon_ok {
                r"Control pipe \\.\pipe\nanodock_ctl is active".to_string()
            } else {
                "core-daemon is not listening on control pipe".to_string()
            },
        });

        let docker_pipe_ok = Path::new(r"\\.\pipe\docker_engine").exists();
        checks.push(DoctorCheckItem {
            name: "Docker Named Pipe".to_string(),
            level: if docker_pipe_ok && daemon_ok {
                "ok"
            } else {
                "warn"
            }
            .to_string(),
            detail: if docker_pipe_ok && daemon_ok {
                r"\\.\pipe\docker_engine is active".to_string()
            } else if docker_pipe_ok {
                "docker_engine pipe is held by another Docker engine".to_string()
            } else {
                "docker_engine pipe is inactive".to_string()
            },
        });

        checks.push(DoctorCheckItem {
            name: "CLI & PATH".to_string(),
            level: if env.nanodock_in_path { "ok" } else { "warn" }.to_string(),
            detail: if env.nanodock_in_path {
                format!("NanoDock is in User PATH ({})", env.install_dir)
            } else {
                "NanoDock is not in User PATH".to_string()
            },
        });

        checks.push(DoctorCheckItem {
            name: "Docker CredsStore".to_string(),
            level: if env.creds_store_warning.is_some() {
                "warn"
            } else {
                "ok"
            }
            .to_string(),
            detail: env
                .creds_store_warning
                .unwrap_or_else(|| "No Docker Desktop credsStore conflict".to_string()),
        });

        checks.push(DoctorCheckItem {
            name: "WinCred Helper".to_string(),
            level: if env.wincred_available { "ok" } else { "warn" }.to_string(),
            detail: if env.wincred_available {
                "docker-credential-wincred.exe detected on PATH".to_string()
            } else {
                "docker-credential-wincred.exe not found (optional)".to_string()
            },
        });

        let wslconfig_exists = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .is_some_and(|p| p.join(".wslconfig").is_file());
        checks.push(DoctorCheckItem {
            name: ".wslconfig".to_string(),
            level: if wslconfig_exists { "ok" } else { "warn" }.to_string(),
            detail: if wslconfig_exists {
                "Managed .wslconfig is present".to_string()
            } else {
                ".wslconfig not created yet".to_string()
            },
        });

        Ok(checks)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn set_wincred_creds_store() -> Result<EnvironmentStatus, String> {
    tokio::task::spawn_blocking(|| {
        let profile = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| "USERPROFILE environment variable is not set".to_string())?;
        wsl_bridge::set_user_docker_wincred(&profile)
            .map_err(|e| format!("Failed to update Docker config.json: {e}"))?;
        collect_environment_status()
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn pick_archive_file(mode: String, default_name: String) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        let safe_name = default_name.replace('\'', "");
        let script = if mode == "save" {
            format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 $d = New-Object System.Windows.Forms.SaveFileDialog; \
                 $d.Filter = 'Gzip Tar Archive (*.tar.gz)|*.tar.gz|All Files (*.*)|*.*'; \
                 $d.FileName = '{safe_name}'; \
                 if ($d.ShowDialog() -eq 'OK') {{ Write-Output $d.FileName }}"
            )
        } else {
            "Add-Type -AssemblyName System.Windows.Forms; \
             $d = New-Object System.Windows.Forms.OpenFileDialog; \
             $d.Filter = 'Gzip Tar Archive (*.tar.gz)|*.tar.gz|All Files (*.*)|*.*'; \
             if ($d.ShowDialog() -eq 'OK') { Write-Output $d.FileName }"
                .to_string()
        };
        let out = hidden_ps(&script)?;
        if !out.status.success() {
            return Ok(None);
        }
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(path))
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}
