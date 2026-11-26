use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context as _;

use crate::engine_cmds::ensure_engine_ready;

const DOCKER_NPIPE_HOST: &str = "npipe:////./pipe/docker_engine";

pub fn invoked_as_docker() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().eq_ignore_ascii_case("docker"))
        })
        .unwrap_or(false)
}

pub async fn cmd_passthrough(args: &[OsString]) -> anyhow::Result<u8> {
    let config = ensure_engine_ready().await?;

    if let Some(docker_exe) = find_real_docker_exe() {
        let status = Command::new(docker_exe)
            .env("DOCKER_HOST", DOCKER_NPIPE_HOST)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("failed to execute docker.exe")?;
        return Ok(exit_code_to_u8(status.code()));
    }

    let translated = translate_args_for_wsl(args);
    let mut cmd = Command::new("wsl.exe");
    cmd.args(["-d", &config.distro_name, "--", "docker"]);
    cmd.args(&translated);
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd
        .status()
        .context("failed to execute docker inside NanoDock WSL2 distro")?;
    Ok(exit_code_to_u8(status.code()))
}

/// Finds an external `docker.exe` binary on `PATH`, skipping NanoDock's own directory
/// so a `docker.exe` shim (hardlink/copy of `nanodock.exe`) never recurses into itself.
pub fn find_real_docker_exe() -> Option<PathBuf> {
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if let Some(ref own_dir) = self_dir {
            if paths_equal_ci(&dir, own_dir) {
                continue;
            }
        }
        let candidate = dir.join("docker.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn paths_equal_ci(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

fn exit_code_to_u8(code: Option<i32>) -> u8 {
    match code {
        Some(c) if (0..=255).contains(&c) => c as u8,
        Some(_) => 1,
        None => 1,
    }
}

fn windows_path_to_wsl(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = input[3..].replace('\\', "/");
        let trimmed = rest.trim_start_matches('/');
        if trimmed.is_empty() {
            return Some(format!("/mnt/{drive}"));
        }
        return Some(format!("/mnt/{drive}/{trimmed}"));
    }
    None
}

fn rewrite_bind_spec(spec: &str) -> Option<String> {
    let bytes = spec.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let rest = &spec[2..];
        if let Some(colon_offset) = rest.find(':') {
            let host_part = &spec[..2 + colon_offset];
            let target_and_opts = &spec[2 + colon_offset..];
            let wsl_host = windows_path_to_wsl(host_part)?;
            return Some(format!("{wsl_host}{target_and_opts}"));
        }
    }
    None
}

pub fn translate_args_for_wsl(args: &[OsString]) -> Vec<OsString> {
    let mut out = Vec::with_capacity(args.len());
    let mut expect_volume_value = false;
    let mut expect_path_value = false;

    for arg in args {
        let s = arg.to_string_lossy();

        if expect_volume_value {
            expect_volume_value = false;
            let rewritten = rewrite_bind_spec(&s).unwrap_or_else(|| s.into_owned());
            out.push(OsString::from(rewritten));
            continue;
        }

        if expect_path_value {
            expect_path_value = false;
            let rewritten = windows_path_to_wsl(&s).unwrap_or_else(|| s.into_owned());
            out.push(OsString::from(rewritten));
            continue;
        }

        if s == "-v" || s == "--volume" {
            expect_volume_value = true;
            out.push(arg.clone());
            continue;
        }

        if s == "-f" || s == "--file" || s == "--env-file" {
            expect_path_value = true;
            out.push(arg.clone());
            continue;
        }

        if let Some(val) = s.strip_prefix("--volume=").or_else(|| s.strip_prefix("-v=")) {
            let prefix = if s.starts_with("--volume=") {
                "--volume="
            } else {
                "-v="
            };
            let rewritten = rewrite_bind_spec(val).unwrap_or_else(|| val.to_string());
            out.push(OsString::from(format!("{prefix}{rewritten}")));
            continue;
        }

        if let Some(wsl_path) = windows_path_to_wsl(&s) {
            out.push(OsString::from(wsl_path));
            continue;
        }

        out.push(arg.clone());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_windows_paths_and_volumes_for_wsl() {
        let input: Vec<OsString> = vec![
            "run",
            "-v",
            r"D:\HoangLong\Dev\app:/app:ro",
            r"--volume=C:\data:/data",
            "-f",
            r"C:\proj\Dockerfile",
            "alpine",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();

        let translated = translate_args_for_wsl(&input);
        let strings: Vec<String> = translated
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            strings,
            vec![
                "run",
                "-v",
                "/mnt/d/HoangLong/Dev/app:/app:ro",
                "--volume=/mnt/c/data:/data",
                "-f",
                "/mnt/c/proj/Dockerfile",
                "alpine",
            ]
        );
    }

    #[test]
    fn leaves_non_windows_args_untouched() {
        let input: Vec<OsString> = vec!["run", "--rm", "-v", "myvol:/data", "nginx:alpine"]
            .into_iter()
            .map(OsString::from)
            .collect();

        let translated = translate_args_for_wsl(&input);
        assert_eq!(translated, input);
    }
}
