use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use shared_types::{redact_sensitive_text, ControlRequest, ControlResponse};

use crate::engine_cmds::send_control_request;

/// Generates a redacted Markdown diagnostic report (online via `core-daemon` or offline from disk).
pub async fn cmd_report(args: &[String]) -> anyhow::Result<u8> {
    let mut lines = 120usize;
    let mut out_path: Option<PathBuf> = None;
    let mut copy_to_clipboard = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--lines" => {
                i += 1;
                if let Some(val) = args.get(i) {
                    lines = val.parse::<usize>().unwrap_or(120).clamp(20, 500);
                }
            }
            "--out" => {
                i += 1;
                if let Some(val) = args.get(i) {
                    out_path = Some(PathBuf::from(val));
                }
            }
            "--copy" => {
                copy_to_clipboard = true;
            }
            other => {
                anyhow::bail!("unknown option for `nanodock report`: {other}");
            }
        }
        i += 1;
    }

    let markdown = match send_control_request(ControlRequest::GetDiagnosticReport {
        lines: Some(lines),
    })
    .await
    {
        Ok(ControlResponse::DiagnosticReport { markdown, .. }) => markdown,
        _ => build_offline_report(lines),
    };

    if let Some(path) = out_path {
        fs::write(&path, &markdown)?;
        println!("Diagnostic report written to {}", path.display());
    } else if copy_to_clipboard {
        copy_clipboard(&markdown)?;
        println!("Diagnostic report copied to clipboard ({} bytes).", markdown.len());
    } else {
        println!("{markdown}");
    }

    Ok(0)
}

fn default_log_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("NanoDock")
            .join("logs")
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nanodock")
            .join("logs")
    }
}

fn read_tail(path: &Path, max_lines: usize) -> String {
    let Ok(content) = fs::read_to_string(path) else {
        return String::new();
    };
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(max_lines);
    redact_sensitive_text(&all[start..].join("\n"))
}

fn build_offline_report(lines: usize) -> String {
    let dir = default_log_dir();
    let crash = fs::read_to_string(dir.join("last-crash.txt"))
        .ok()
        .map(|s| redact_sensitive_text(s.trim()))
        .filter(|s| !s.is_empty());
    let daemon_log = read_tail(&dir.join("core-daemon.log"), lines);

    let mut out = String::new();
    out.push_str("## NanoDock Diagnostic Report (Offline)\n\n");
    out.push_str(&format!(
        "- **CLI Version:** `nanodock {}`\n- **OS / Arch:** `{}/{}`\n- **Daemon:** `not running`\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    if let Some(c) = crash {
        out.push_str("\n### Last Crash Report\n<details>\n<summary>last-crash.txt</summary>\n\n```\n");
        out.push_str(&c);
        out.push_str("\n```\n</details>\n");
    }
    out.push_str("\n### core-daemon.log\n<details>\n<summary>Recent daemon logs</summary>\n\n```\n");
    out.push_str(if daemon_log.is_empty() { "(empty)" } else { &daemon_log });
    out.push_str("\n```\n</details>\n");
    out
}

fn copy_clipboard(text: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    let candidates: &[(&str, &[&str])] = &[("clip.exe", &[])];
    #[cfg(not(windows))]
    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];

    for (bin, args) in candidates {
        if let Ok(mut child) = Command::new(bin)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok_and(|s| s.success()) {
                return Ok(());
            }
        }
    }
    anyhow::bail!("no clipboard utility found; re-run without --copy or pass --out <file>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_report_includes_header_and_os() {
        let md = build_offline_report(50);
        assert!(md.contains("NanoDock Diagnostic Report (Offline)"));
        assert!(md.contains(std::env::consts::OS));
    }
}
