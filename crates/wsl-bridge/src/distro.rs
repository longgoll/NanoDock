use tracing::{info, warn};

use crate::error::WslError;
use crate::traits::WslExecutor;

/// Wakes the micro-distro and runs `/init.sh` (idempotent).
pub async fn wake_distro<E: WslExecutor>(exec: &E, distro_name: &str) -> Result<(), WslError> {
    let out = exec.run(&["-d", distro_name, "-e", "/init.sh"]).await?;
    if !out.success {
        return Err(WslError::CommandFailed {
            code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(())
}

/// Gracefully stops `dockerd` and `containerd` inside the distro using fast PID/process polling
/// and flushes filesystem buffers (`sync`) before terminating the VM, preventing `overlay2` corruption.
pub async fn graceful_hibernate<E: WslExecutor>(
    exec: &E,
    distro_name: &str,
) -> Result<(), WslError> {
    info!(distro = distro_name, "Stopping dockerd/containerd before hibernate");
    let shutdown_script = "\
        pkill -TERM dockerd 2>/dev/null || true; \
        i=0; while [ -f /var/run/docker.pid ] && kill -0 \"$(cat /var/run/docker.pid 2>/dev/null)\" 2>/dev/null && [ \"$i\" -lt 80 ]; do i=$((i+1)); sleep 0.05; done; \
        pkill -TERM containerd 2>/dev/null || true; \
        j=0; while pgrep -x containerd >/dev/null 2>&1 && [ \"$j\" -lt 60 ]; do j=$((j+1)); sleep 0.05; done; \
        sync";
    let _ = exec
        .run(&["-d", distro_name, "-e", "sh", "-c", shutdown_script])
        .await;

    let term = exec.run(&["--terminate", distro_name]).await?;
    if !term.success {
        warn!(stderr = %term.stderr, "wsl --terminate returned non-zero");
        return Err(WslError::CommandFailed {
            code: term.exit_code,
            stderr: term.stderr,
        });
    }
    Ok(())
}

/// Runs `fstrim -av` inside the micro-distro to reclaim unused `.vhdx` blocks.
pub async fn trim_distro<E: WslExecutor>(exec: &E, distro_name: &str) -> Result<String, WslError> {
    let out = exec.run(&["-d", distro_name, "-e", "/sbin/fstrim", "-av"]).await?;
    if !out.success {
        return Err(WslError::CommandFailed {
            code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out.stdout)
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
    async fn wake_distro_succeeds_and_propagates_failure() {
        let mock = MockExecutor::new(vec![ok(""), fail("distro missing")]);
        wake_distro(&mock, "nanodock").await.unwrap();
        let err = wake_distro(&mock, "nanodock").await.unwrap_err();
        assert!(matches!(err, WslError::CommandFailed { code: Some(1), .. }));
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls[0], vec!["-d", "nanodock", "-e", "/init.sh"]);
    }

    #[tokio::test]
    async fn trim_distro_returns_stdout_on_success() {
        let mock = MockExecutor::new(vec![ok("/: 128 MiB trimmed\n")]);
        let out = trim_distro(&mock, "nanodock").await.unwrap();
        assert!(out.contains("128 MiB"));
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls[0], vec!["-d", "nanodock", "-e", "/sbin/fstrim", "-av"]);
    }

    #[tokio::test]
    async fn graceful_hibernate_stops_daemons_then_terminates() {
        let mock = MockExecutor::new(vec![ok(""), ok("")]);
        graceful_hibernate(&mock, "nanodock").await.unwrap();
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1], vec!["--terminate", "nanodock"]);
    }
}
