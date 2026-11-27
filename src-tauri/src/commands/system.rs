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

#[derive(Debug, Clone, Serialize)]
pub struct HostCapabilities {
    pub total_memory_mb: u64,
    pub cpu_cores: u32,
    pub windows_build: Option<u32>,
    pub windows_label: String,
    pub wsl_version: Option<String>,
    pub supports_mirrored_networking: bool,
    pub supports_sparse_vhd: bool,
    pub recommended_memory_mb: u64,
    pub recommended_swap_mb: u64,
    pub recommended_processors: Option<u32>,
    pub recommended_mirrored_networking: bool,
    pub recommended_sparse_vhd: bool,
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

#[cfg(windows)]
fn detect_total_memory_mb() -> u64 {
    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(lp_buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut status = MemoryStatusEx {
        dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
        dw_memory_load: 0,
        ull_total_phys: 0,
        ull_avail_phys: 0,
        ull_total_page_file: 0,
        ull_avail_page_file: 0,
        ull_total_virtual: 0,
        ull_avail_virtual: 0,
        ull_avail_extended_virtual: 0,
    };

    // SAFETY: `status` is properly initialized with its exact struct size in `dw_length`.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok != 0 {
        status.ull_total_phys / (1024 * 1024)
    } else {
        0
    }
}

#[cfg(not(windows))]
fn detect_total_memory_mb() -> u64 {
    0
}

fn parse_windows_build_number(ver_output: &str) -> Option<u32> {
    let start = ver_output.find('[')?;
    let end = ver_output[start..].find(']')? + start;
    let inside = &ver_output[start + 1..end];
    let version_token = inside.split_whitespace().last()?;
    let mut parts = version_token.split('.');
    let _major = parts.next()?;
    let _minor = parts.next()?;
    parts.next()?.parse::<u32>().ok()
}

fn detect_windows_build_number() -> Option<u32> {
    let out = hidden_command("cmd").args(["/c", "ver"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse_windows_build_number(&text)
}

fn parse_wsl_version(output: &str) -> Option<String> {
    let first_line = output.lines().find(|l| !l.trim().is_empty())?;
    for token in first_line.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        let mut parts = cleaned.split('.');
        if let (Some(maj), Some(min)) = (parts.next(), parts.next()) {
            if maj.parse::<u32>().is_ok() && min.parse::<u32>().is_ok() {
                let patch = parts.next().and_then(|p| p.parse::<u32>().ok());
                return Some(match patch {
                    Some(p) => format!("{maj}.{min}.{p}"),
                    None => format!("{maj}.{min}"),
                });
            }
        }
    }
    None
}

fn detect_wsl_version() -> Option<String> {
    let out = hidden_command("wsl.exe").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let decoded = wsl_bridge::decode_wsl_output(&out.stdout);
    parse_wsl_version(&decoded)
}

fn recommend_vm_memory_mb(total_memory_mb: u64) -> u64 {
    if total_memory_mb == 0 {
        return 4096;
    }
    if total_memory_mb <= 8192 {
        2048
    } else if total_memory_mb <= 16384 {
        4096
    } else if total_memory_mb <= 32768 {
        8192
    } else {
        12288
    }
}

fn build_host_capabilities(
    total_memory_mb: u64,
    cpu_cores: u32,
    windows_build: Option<u32>,
    wsl_version: Option<String>,
) -> HostCapabilities {
    const WIN11_MIN_BUILD: u32 = 22_000;

    let is_win11 = windows_build.is_some_and(|b| b >= WIN11_MIN_BUILD);
    let windows_label = match windows_build {
        Some(b) if b >= WIN11_MIN_BUILD => "Windows 11".to_string(),
        Some(_) => "Windows 10".to_string(),
        None => "Windows".to_string(),
    };

    let wsl_major = wsl_version
        .as_deref()
        .and_then(|v| v.split('.').next())
        .and_then(|m| m.parse::<u32>().ok())
        .unwrap_or(0);

    let supports_mirrored_networking = is_win11 && wsl_major >= 2;
    let supports_sparse_vhd = wsl_major >= 2;

    let recommended_memory_mb = recommend_vm_memory_mb(total_memory_mb);
    let recommended_swap_mb = recommended_memory_mb.clamp(2048, 4096);

    HostCapabilities {
        total_memory_mb,
        cpu_cores,
        windows_build,
        windows_label,
        wsl_version,
        supports_mirrored_networking,
        supports_sparse_vhd,
        recommended_memory_mb,
        recommended_swap_mb,
        recommended_processors: None,
        recommended_mirrored_networking: supports_mirrored_networking,
        recommended_sparse_vhd: supports_sparse_vhd,
    }
}

/// Collects host hardware and WSL2 feature capabilities for VM resource recommendations.
pub fn collect_host_capabilities() -> HostCapabilities {
    let total_memory_mb = detect_total_memory_mb();
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);
    let windows_build = detect_windows_build_number();
    let wsl_version = detect_wsl_version();
    build_host_capabilities(total_memory_mb, cpu_cores, windows_build, wsl_version)
}

#[tauri::command]
pub async fn get_host_capabilities() -> Result<HostCapabilities, String> {
    tokio::task::spawn_blocking(collect_host_capabilities)
        .await
        .map_err(|e| format!("Task join error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wsl_version_from_localized_output() {
        assert_eq!(
            parse_wsl_version("WSL version: 2.3.26.0\nKernel version: 5.15.167.4-1"),
            Some("2.3.26".to_string())
        );
        assert_eq!(
            parse_wsl_version("Phiên bản WSL: 2.4.10.0\nPhiên bản nhân: 5.15.167.4"),
            Some("2.4.10".to_string())
        );
        assert_eq!(parse_wsl_version("no version here"), None);
    }

    #[test]
    fn builds_recommendations_for_low_mid_and_high_ram_hosts() {
        let low = build_host_capabilities(8192, 8, Some(19045), None);
        assert_eq!(low.recommended_memory_mb, 2048);
        assert_eq!(low.recommended_swap_mb, 2048);
        assert!(!low.supports_mirrored_networking);
        assert!(!low.recommended_mirrored_networking);
        assert_eq!(low.windows_label, "Windows 10");

        let mid = build_host_capabilities(16384, 12, Some(22631), Some("2.3.26".to_string()));
        assert_eq!(mid.recommended_memory_mb, 4096);
        assert_eq!(mid.recommended_swap_mb, 4096);
        assert!(mid.supports_mirrored_networking);
        assert!(mid.supports_sparse_vhd);
        assert_eq!(mid.windows_label, "Windows 11");

        let high = build_host_capabilities(65536, 24, Some(26100), Some("2.4.10".to_string()));
        assert_eq!(high.recommended_memory_mb, 12288);
        assert_eq!(high.recommended_swap_mb, 4096);
        assert!(high.recommended_mirrored_networking);
    }
}
