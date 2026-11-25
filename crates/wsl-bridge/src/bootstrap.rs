use std::path::Path;

use tracing::{info, warn};

use crate::error::WslError;
use crate::traits::WslExecutor;

/// Result of first-run WSL2 and micro-distro bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapOutcome {
    AlreadyReady,
    ImportedAndConfigured,
}

/// Ensures WSL2 is available and the NanoDock micro-distro is imported with `sparseVhd` enabled.
pub async fn ensure_distro_imported<E: WslExecutor>(
    exec: &E,
    distro_name: &str,
    install_dir: &Path,
    rootfs_tar_gz: &Path,
) -> Result<BootstrapOutcome, WslError> {
    let status = exec.run(&["--status"]).await?;
    if !status.success {
        info!("WSL2 is not enabled; triggering wsl --install --no-distribution");
        let install = exec.run(&["--install", "--no-distribution"]).await?;
        if !install.success {
            return Err(WslError::CommandFailed {
                code: install.exit_code,
                stderr: install.stderr,
            });
        }
        return Err(WslError::RebootRequired);
    }

    let list = exec.run(&["--list", "--quiet"]).await?;
    if !list.success {
        return Err(WslError::CommandFailed {
            code: list.exit_code,
            stderr: list.stderr,
        });
    }

    if list_contains_distro(&list.stdout, distro_name) {
        return Ok(BootstrapOutcome::AlreadyReady);
    }

    if !rootfs_tar_gz.exists() {
        return Err(WslError::RootfsMissing(rootfs_tar_gz.display().to_string()));
    }

    std::fs::create_dir_all(install_dir)?;
    let install_str = install_dir.to_string_lossy();
    let rootfs_str = rootfs_tar_gz.to_string_lossy();

    info!(distro = distro_name, "Importing NanoDock WSL2 micro-distro");
    let import = exec
        .run(&[
            "--import",
            distro_name,
            install_str.as_ref(),
            rootfs_str.as_ref(),
            "--version",
            "2",
        ])
        .await?;
    if !import.success {
        return Err(WslError::CommandFailed {
            code: import.exit_code,
            stderr: import.stderr,
        });
    }

    enable_sparse_vhd(exec, distro_name).await;
    Ok(BootstrapOutcome::ImportedAndConfigured)
}

async fn enable_sparse_vhd<E: WslExecutor>(exec: &E, distro_name: &str) {
    let with_unsafe = exec
        .run(&[
            "--manage",
            distro_name,
            "--set-sparse",
            "true",
            "--allow-unsafe",
        ])
        .await;

    match with_unsafe {
        Ok(out) if out.success => {
            info!(distro = distro_name, "Enabled sparse VHD");
        }
        _ => {
            let fallback = exec
                .run(&["--manage", distro_name, "--set-sparse", "true"])
                .await;
            if let Ok(out) = fallback {
                if out.success {
                    info!(distro = distro_name, "Enabled sparse VHD (fallback)");
                    return;
                }
                warn!(stderr = %out.stderr, "Failed to enable sparse VHD");
            }
        }
    }
}

fn list_contains_distro(stdout: &str, distro_name: &str) -> bool {
    stdout
        .lines()
        .map(str::trim)
        .any(|line| line.eq_ignore_ascii_case(distro_name))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::traits::CommandOutput;

    struct MockExecutor {
        responses: Mutex<VecDeque<CommandOutput>>,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl MockExecutor {
        fn new(responses: Vec<CommandOutput>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl WslExecutor for MockExecutor {
        async fn run(&self, args: &[&str]) -> Result<CommandOutput, WslError> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            Ok(self.responses.lock().unwrap().pop_front().unwrap())
        }
    }

    fn ok(stdout: &str) -> CommandOutput {
        CommandOutput {
            success: true,
            exit_code: Some(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    fn fail(stderr: &str) -> CommandOutput {
        CommandOutput {
            success: false,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: stderr.to_string(),
        }
    }

    #[tokio::test]
    async fn returns_already_ready_if_distro_listed() {
        let mock = MockExecutor::new(vec![ok("Default Version: 2"), ok("Ubuntu\nnanodock\n")]);
        let res = ensure_distro_imported(
            &mock,
            "nanodock",
            Path::new("C:\\nanodock"),
            Path::new("rootfs.tar.gz"),
        )
        .await
        .unwrap();
        assert_eq!(res, BootstrapOutcome::AlreadyReady);
    }

    #[tokio::test]
    async fn triggers_install_and_reboot_when_wsl_missing() {
        let mock = MockExecutor::new(vec![fail("WSL not installed"), ok("Installed")]);
        let err = ensure_distro_imported(
            &mock,
            "nanodock",
            Path::new("C:\\nanodock"),
            Path::new("rootfs.tar.gz"),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, WslError::RebootRequired));
    }
}
