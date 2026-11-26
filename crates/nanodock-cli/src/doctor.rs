use std::path::{Path, PathBuf};
use std::process::Command;

use shared_types::{ControlRequest, ControlResponse};
use wsl_bridge::{check_user_docker_config, decode_wsl_output, detect_wincred_available};

use crate::engine_cmds::{send_control_request, CONTROL_PIPE_NAME};
use crate::passthrough::find_real_docker_exe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckLevel {
    Ok,
    Warn,
    Fail,
}

pub struct DoctorCheck {
    pub name: &'static str,
    pub level: CheckLevel,
    pub detail: String,
}

pub async fn cmd_doctor() -> anyhow::Result<u8> {
    let mut checks = Vec::new();
    checks.push(check_wsl2_installed());
    checks.push(check_nanodock_distro());
    checks.push(check_daemon_pipe().await);
    checks.push(check_docker_pipe_ownership().await);
    checks.push(check_docker_cli());
    checks.push(check_creds_store());
    checks.push(check_wincred());
    checks.push(check_wslconfig());

    println!("NanoDock Environment Diagnostics");
    println!("================================");
    let mut failures = 0u8;
    let mut warnings = 0u8;
    for check in &checks {
        let tag = match check.level {
            CheckLevel::Ok => "[OK]  ",
            CheckLevel::Warn => {
                warnings += 1;
                "[WARN]"
            }
            CheckLevel::Fail => {
                failures += 1;
                "[FAIL]"
            }
        };
        println!("{tag} {:<24} {}", check.name, check.detail);
    }

    println!();
    if failures > 0 {
        println!("Result: {failures} failed, {warnings} warnings.");
        Ok(1)
    } else if warnings > 0 {
        println!("Result: all critical checks passed ({warnings} warnings).");
        Ok(0)
    } else {
        println!("Result: all checks passed.");
        Ok(0)
    }
}

fn check_wsl2_installed() -> DoctorCheck {
    match Command::new("wsl.exe").arg("--status").output() {
        Ok(out) if out.status.success() => DoctorCheck {
            name: "WSL2 Runtime",
            level: CheckLevel::Ok,
            detail: "wsl.exe --status succeeded".to_string(),
        },
        Ok(out) => DoctorCheck {
            name: "WSL2 Runtime",
            level: CheckLevel::Fail,
            detail: format!("wsl.exe --status failed: {}", decode_wsl_output(&out.stderr).trim()),
        },
        Err(err) => DoctorCheck {
            name: "WSL2 Runtime",
            level: CheckLevel::Fail,
            detail: format!("wsl.exe not found: {err}"),
        },
    }
}

fn check_nanodock_distro() -> DoctorCheck {
    match Command::new("wsl.exe").args(["-l", "-q"]).output() {
        Ok(out) if out.status.success() => {
            let decoded = decode_wsl_output(&out.stdout);
            let found = decoded.lines().any(|l| l.trim().eq_ignore_ascii_case("nanodock"));
            if found {
                DoctorCheck {
                    name: "NanoDock Distro",
                    level: CheckLevel::Ok,
                    detail: "WSL2 distro 'nanodock' is registered".to_string(),
                }
            } else {
                DoctorCheck {
                    name: "NanoDock Distro",
                    level: CheckLevel::Warn,
                    detail: "Distro 'nanodock' not found yet (will auto-import on daemon start)".to_string(),
                }
            }
        }
        _ => DoctorCheck {
            name: "NanoDock Distro",
            level: CheckLevel::Warn,
            detail: "Unable to query WSL2 distributions".to_string(),
        },
    }
}

async fn check_daemon_pipe() -> DoctorCheck {
    if !Path::new(CONTROL_PIPE_NAME).exists() {
        return DoctorCheck {
            name: "Core Daemon",
            level: CheckLevel::Warn,
            detail: "core-daemon is not currently running".to_string(),
        };
    }
    match send_control_request(ControlRequest::GetStatus).await {
        Ok(ControlResponse::Status { state, .. }) => DoctorCheck {
            name: "Core Daemon",
            level: CheckLevel::Ok,
            detail: format!("Reachable on {CONTROL_PIPE_NAME} (state: {state:?})"),
        },
        Ok(other) => DoctorCheck {
            name: "Core Daemon",
            level: CheckLevel::Warn,
            detail: format!("Unexpected response: {other:?}"),
        },
        Err(err) => DoctorCheck {
            name: "Core Daemon",
            level: CheckLevel::Fail,
            detail: format!("Pipe exists but RPC failed: {err}"),
        },
    }
}

async fn check_docker_pipe_ownership() -> DoctorCheck {
    let docker_pipe = Path::new(r"\\.\pipe\docker_engine");
    let daemon_running = Path::new(CONTROL_PIPE_NAME).exists();
    if docker_pipe.exists() && !daemon_running {
        DoctorCheck {
            name: "Docker Named Pipe",
            level: CheckLevel::Warn,
            detail: "docker_engine pipe is held by another engine (e.g. Docker Desktop)".to_string(),
        }
    } else if docker_pipe.exists() {
        DoctorCheck {
            name: "Docker Named Pipe",
            level: CheckLevel::Ok,
            detail: r"\\.\pipe\docker_engine is active".to_string(),
        }
    } else {
        DoctorCheck {
            name: "Docker Named Pipe",
            level: CheckLevel::Warn,
            detail: "docker_engine pipe is inactive (start core-daemon to bind)".to_string(),
        }
    }
}

fn check_docker_cli() -> DoctorCheck {
    if let Some(ext) = find_real_docker_exe() {
        DoctorCheck {
            name: "Docker CLI",
            level: CheckLevel::Ok,
            detail: format!("External docker.exe found at {}", ext.display()),
        }
    } else {
        DoctorCheck {
            name: "Docker CLI",
            level: CheckLevel::Ok,
            detail: "Using NanoDock WSL2 built-in docker CLI / shim".to_string(),
        }
    }
}

fn check_creds_store() -> DoctorCheck {
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return DoctorCheck {
            name: "Docker CredsStore",
            level: CheckLevel::Warn,
            detail: "USERPROFILE not set".to_string(),
        };
    };
    if let Some(warn) = check_user_docker_config(&profile) {
        DoctorCheck {
            name: "Docker CredsStore",
            level: CheckLevel::Warn,
            detail: warn.message,
        }
    } else {
        DoctorCheck {
            name: "Docker CredsStore",
            level: CheckLevel::Ok,
            detail: "No Docker Desktop credsStore conflict".to_string(),
        }
    }
}

fn check_wincred() -> DoctorCheck {
    if detect_wincred_available() {
        DoctorCheck {
            name: "WinCred Helper",
            level: CheckLevel::Ok,
            detail: "docker-credential-wincred.exe found on PATH".to_string(),
        }
    } else {
        DoctorCheck {
            name: "WinCred Helper",
            level: CheckLevel::Warn,
            detail: "docker-credential-wincred.exe not on PATH (optional for private registries)".to_string(),
        }
    }
}

fn check_wslconfig() -> DoctorCheck {
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return DoctorCheck {
            name: ".wslconfig",
            level: CheckLevel::Warn,
            detail: "USERPROFILE not set".to_string(),
        };
    };
    let path = profile.join(".wslconfig");
    if path.is_file() {
        DoctorCheck {
            name: ".wslconfig",
            level: CheckLevel::Ok,
            detail: format!("Present at {}", path.display()),
        }
    } else {
        DoctorCheck {
            name: ".wslconfig",
            level: CheckLevel::Warn,
            detail: "Not created yet (generated automatically on core-daemon start)".to_string(),
        }
    }
}
