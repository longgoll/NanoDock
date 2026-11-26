use shared_types::WslDistroInfo;

use crate::error::WslError;
use crate::traits::WslExecutor;

const PROFILE_SCRIPT_PATH: &str = "/etc/profile.d/nanodock.sh";
const PROFILE_SCRIPT_CONTENT: &str =
    "export DOCKER_HOST=\"unix:///mnt/wsl/nanodock/docker.sock\"\n";

/// Parses `wsl.exe -l -v` output into `WslDistroInfo` entries, excluding NanoDock's internal
/// micro-distro and Docker Desktop helper distros.
pub fn parse_wsl_list_verbose(raw: &str, nanodock_distro: &str) -> Vec<WslDistroInfo> {
    let mut distros = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("NAME") {
            continue;
        }
        let is_default = trimmed.starts_with('*');
        let clean = trimmed.trim_start_matches('*').trim();
        let parts: Vec<&str> = clean.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let version: u8 = match parts.last().and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => continue,
        };
        let state = parts[parts.len() - 2].to_string();
        let name = parts[..parts.len() - 2].join(" ");
        if name.eq_ignore_ascii_case(nanodock_distro)
            || name.eq_ignore_ascii_case("docker-desktop")
            || name.eq_ignore_ascii_case("docker-desktop-data")
        {
            continue;
        }
        distros.push(WslDistroInfo {
            name,
            state,
            version,
            is_default,
            integrated: false,
        });
    }
    distros
}

/// Lists user WSL2 distros and checks whether each has `/etc/profile.d/nanodock.sh` configured.
pub async fn list_user_wsl_distros<E: WslExecutor>(
    exec: &E,
    nanodock_distro: &str,
) -> Result<Vec<WslDistroInfo>, WslError> {
    let out = exec.run(&["-l", "-v"]).await?;
    if !out.success {
        return Err(WslError::CommandFailed {
            code: out.exit_code,
            stderr: out.stderr,
        });
    }

    let mut distros = parse_wsl_list_verbose(&out.stdout, nanodock_distro);
    for distro in &mut distros {
        // Avoid booting stopped WSL2 distributions just to inspect profile scripts.
        if distro.version != 2 || !distro.state.eq_ignore_ascii_case("Running") {
            continue;
        }
        let check = exec
            .run(&[
                "-d",
                &distro.name,
                "-e",
                "sh",
                "-c",
                "test -f /etc/profile.d/nanodock.sh && grep -q nanodock /etc/profile.d/nanodock.sh",
            ])
            .await;
        if let Ok(res) = check {
            distro.integrated = res.success;
        }
    }
    Ok(distros)
}

/// Enables or disables `DOCKER_HOST=unix:///mnt/wsl/nanodock/docker.sock` inside a target WSL2
/// distribution by managing `/etc/profile.d/nanodock.sh`.
pub async fn set_distro_docker_integration<E: WslExecutor>(
    exec: &E,
    distro: &str,
    enabled: bool,
) -> Result<(), WslError> {
    let cmd = if enabled {
        format!(
            "mkdir -p /etc/profile.d && printf '%s' '{PROFILE_SCRIPT_CONTENT}' > {PROFILE_SCRIPT_PATH} && chmod 644 {PROFILE_SCRIPT_PATH}"
        )
    } else {
        format!("rm -f {PROFILE_SCRIPT_PATH}")
    };

    let out = exec
        .run(&["-d", distro, "-u", "root", "-e", "sh", "-c", &cmd])
        .await?;
    if !out.success {
        return Err(WslError::CommandFailed {
            code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wsl_list_verbose_filtering_internal_distros() {
        let sample = "\
  NAME                   STATE           VERSION
* Ubuntu-22.04           Running         2
  nanodock               Stopped         2
  docker-desktop         Stopped         2
  Debian                 Stopped         2
  LegacyDistro           Stopped         1
";
        let list = parse_wsl_list_verbose(sample, "nanodock");
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "Ubuntu-22.04");
        assert!(list[0].is_default);
        assert_eq!(list[0].state, "Running");
        assert_eq!(list[0].version, 2);
        assert_eq!(list[1].name, "Debian");
        assert!(!list[1].is_default);
        assert_eq!(list[2].name, "LegacyDistro");
        assert_eq!(list[2].version, 1);
    }
}
