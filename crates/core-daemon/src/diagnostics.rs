use std::backtrace::Backtrace;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use shared_types::{redact_sensitive_text, EngineState};
use tracing::error;
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::log_store;

pub fn crash_report_path() -> PathBuf {
    log_store::default_log_dir().join("last-crash.txt")
}

/// Installs a process-wide panic hook that persists redacted panic details + backtrace
/// to `last-crash.txt` so users can report crashes even without a debugger attached.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        let bt = Backtrace::force_capture();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let raw = format!(
            "timestamp_unix: {ts}\nversion: {}\nos: {}/{}\nlocation: {loc}\nmessage: {msg}\n\nbacktrace:\n{bt}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let clean = redact_sensitive_text(&raw);
        let path = crash_report_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, &clean);
        error!(location = %loc, message = %msg, "core-daemon panicked; crash report saved");
        prev(info);
    }));
}

pub async fn build_diagnostic_report<E: WslExecutor + 'static>(
    ctx: &AppContext<E>,
    lines: usize,
) -> (String, bool) {
    let tail_lines = lines.clamp(20, 500);
    let state = *ctx.state.read().await;
    let cfg = ctx.config.read().await.clone();
    let metrics = ctx.metrics();

    let daemon_log = redact_sensitive_text(&log_store::read_tail_lines(
        &log_store::default_log_path(),
        tail_lines,
    ));

    let crash_path = crash_report_path();
    let crash_report = read_redacted_file(&crash_path);
    let has_crash = crash_report.is_some();

    let dockerd_log = if state == EngineState::Running {
        let cmd = format!("tail -n {tail_lines} /var/log/nanodock/dockerd.log 2>/dev/null || true");
        ctx.wsl
            .run(&["-d", &cfg.distro_name, "-e", "sh", "-c", &cmd])
            .await
            .map(|out| redact_sensitive_text(&out.stdout))
            .unwrap_or_default()
    } else {
        String::new()
    };

    let markdown = format_diagnostic_markdown(
        state,
        &cfg,
        &metrics,
        crash_report.as_deref(),
        &daemon_log,
        &dockerd_log,
    );
    (markdown, has_crash)
}

fn read_redacted_file(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(redact_sensitive_text(trimmed))
}

fn format_diagnostic_markdown(
    state: EngineState,
    cfg: &shared_types::EngineConfig,
    metrics: &shared_types::EngineMetrics,
    crash_report: Option<&str>,
    daemon_log: &str,
    dockerd_log: &str,
) -> String {
    let mut out = String::new();
    out.push_str("## NanoDock Diagnostic Report\n\n");
    out.push_str(&format!(
        "- **Version:** `core-daemon {}`\n- **OS / Arch:** `{}/{}`\n- **Engine State:** `{state:?}`\n- **Uptime:** `{}s` | **Hibernates:** `{}` | **Reclaimed:** `{} bytes`\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        metrics.uptime_secs,
        metrics.hibernate_count,
        metrics.total_reclaimed_bytes,
    ));
    out.push_str(&format!(
        "- **Distro:** `{}` | **Turbo Mounts:** `{}` | **Local Proxy:** `{}` (port `{}`)\n",
        cfg.distro_name,
        cfg.turbo_mounts.len(),
        cfg.local_proxy_enabled,
        cfg.local_proxy_port,
    ));

    if let Some(crash) = crash_report {
        out.push_str("\n### Last Crash Report\n<details>\n<summary>last-crash.txt</summary>\n\n```\n");
        out.push_str(crash);
        out.push_str("\n```\n</details>\n");
    }

    out.push_str("\n### core-daemon.log\n<details>\n<summary>Recent daemon logs</summary>\n\n```\n");
    out.push_str(if daemon_log.is_empty() {
        "(empty)"
    } else {
        daemon_log
    });
    out.push_str("\n```\n</details>\n");

    if !dockerd_log.trim().is_empty() {
        out.push_str("\n### dockerd.log\n<details>\n<summary>Recent dockerd logs</summary>\n\n```\n");
        out.push_str(dockerd_log.trim());
        out.push_str("\n```\n</details>\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_markdown_report_with_crash_and_logs() {
        let md = format_diagnostic_markdown(
            EngineState::Running,
            &shared_types::EngineConfig::default(),
            &shared_types::EngineMetrics {
                hibernate_count: 1,
                total_reclaimed_bytes: 1024,
                uptime_secs: 42,
                keep_awake: false,
                idle_remaining_secs: None,
            },
            Some("panic at main.rs:10"),
            "INFO daemon started",
            "INFO dockerd ready",
        );
        assert!(md.contains("## NanoDock Diagnostic Report"));
        assert!(md.contains("Last Crash Report"));
        assert!(md.contains("panic at main.rs:10"));
        assert!(md.contains("INFO daemon started"));
        assert!(md.contains("INFO dockerd ready"));
    }
}
