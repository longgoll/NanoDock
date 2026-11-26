use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use anyhow::Context as _;
use shared_types::{ControlRequest, ControlResponse, EngineConfig, EngineState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

const CONTROL_PIPE_NAME: &str = r"\\.\pipe\nanodock_ctl";
const DOCKER_NPIPE_HOST: &str = "npipe:////./pipe/docker_engine";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match run(args).await {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("nanodock: {err:#}");
            ExitCode::from(1)
        }
    }
}

async fn run(args: Vec<OsString>) -> anyhow::Result<u8> {
    if invoked_as_docker() {
        return cmd_passthrough(&args).await;
    }

    let Some(first) = args.first() else {
        print_help();
        return Ok(0);
    };

    let cmd = first.to_string_lossy();
    match cmd.as_ref() {
        "-h" | "--help" | "help" if args.len() == 1 => {
            print_help();
            Ok(0)
        }
        "-V" | "--version" | "version" if args.len() == 1 => {
            println!("nanodock {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        "status" if args.len() == 1 => cmd_status().await,
        "start" if args.len() == 1 => cmd_start().await,
        "stop" if args.len() == 1 => cmd_stop().await,
        "trim" if args.len() == 1 => cmd_trim().await,
        "config" if args.len() == 1 => cmd_config().await,
        _ => cmd_passthrough(&args).await,
    }
}

fn print_help() {
    println!(
        "\
NanoDock CLI - Lightweight WSL2 container engine & Docker-compatible CLI

USAGE:
    nanodock <COMMAND> [ARGS...]

ENGINE COMMANDS:
    start       Start core-daemon (if needed) and wake the NanoDock WSL2 engine
    stop        Gracefully hibernate the NanoDock WSL2 engine
    status      Show engine state, active streams, and status
    trim        Reclaim unused WSL2 virtual disk space (fstrim / compact)
    config      Print the active engine configuration
    help        Show this help message

CONTAINER COMMANDS:
    Any Docker CLI command works directly via NanoDock, for example:
      nanodock run --rm alpine echo hello
      nanodock ps -a
      nanodock images
      nanodock compose up -d"
    );
}

async fn cmd_status() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::GetStatus).await? {
        ControlResponse::Status {
            state,
            running_containers,
            vhdx_size_bytes,
        } => {
            println!("State:              {state:?}");
            println!("Active streams:     {running_containers}");
            if let Some(bytes) = vhdx_size_bytes {
                let mb = bytes as f64 / (1024.0 * 1024.0);
                println!("VHDX size:          {mb:.1} MB");
            }
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

async fn cmd_start() -> anyhow::Result<u8> {
    ensure_daemon_running().await?;
    match send_control_request(ControlRequest::StartEngine).await? {
        ControlResponse::Ack => {
            println!("NanoDock engine started.");
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

async fn cmd_stop() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::StopEngine).await? {
        ControlResponse::Ack => {
            println!("NanoDock engine stopped.");
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

async fn cmd_trim() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::TrimDisk).await? {
        ControlResponse::Ack => {
            println!("Disk trim completed.");
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

async fn cmd_config() -> anyhow::Result<u8> {
    match send_control_request(ControlRequest::GetConfig).await? {
        ControlResponse::Config(cfg) => {
            println!("{}", serde_json::to_string_pretty(&cfg)?);
            Ok(0)
        }
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response from core-daemon: {other:?}"),
    }
}

async fn cmd_passthrough(args: &[OsString]) -> anyhow::Result<u8> {
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

    // Fallback when docker.exe is not installed on Windows:
    // run the docker CLI bundled inside the NanoDock WSL2 micro-distro.
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

async fn ensure_daemon_running() -> anyhow::Result<()> {
    if Path::new(CONTROL_PIPE_NAME).exists() {
        return Ok(());
    }

    if let Some(daemon_exe) = find_sibling_exe("core-daemon.exe") {
        let mut cmd = Command::new(&daemon_exe);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to start {}", daemon_exe.display()))?;

        for _ in 0..30 {
            if send_control_request(ControlRequest::GetStatus).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        anyhow::bail!("core-daemon started but control pipe did not become ready in time");
    }

    anyhow::bail!(
        "core-daemon is not running. Start NanoDock UI or run `cargo run -p core-daemon` first."
    )
}

async fn ensure_engine_ready() -> anyhow::Result<EngineConfig> {
    ensure_daemon_running().await?;

    if let ControlResponse::Status { state, .. } =
        send_control_request(ControlRequest::GetStatus).await?
    {
        if state != EngineState::Running {
            match send_control_request(ControlRequest::StartEngine).await? {
                ControlResponse::Ack => {}
                ControlResponse::Error { message } => anyhow::bail!(message),
                other => anyhow::bail!("unexpected response starting engine: {other:?}"),
            }
        }
    }

    match send_control_request(ControlRequest::GetConfig).await? {
        ControlResponse::Config(cfg) => Ok(cfg),
        ControlResponse::Error { message } => anyhow::bail!(message),
        other => anyhow::bail!("unexpected response fetching config: {other:?}"),
    }
}

async fn send_control_request(req: ControlRequest) -> anyhow::Result<ControlResponse> {
    let client = open_control_pipe_with_retry().await?;

    let (reader, mut writer) = tokio::io::split(client);
    let mut encoded = serde_json::to_string(&req)?;
    encoded.push('\n');
    writer.write_all(encoded.as_bytes()).await?;

    let mut lines = BufReader::new(reader).lines();
    let line = lines
        .next_line()
        .await?
        .context("control pipe closed before response")?;
    let resp: ControlResponse = serde_json::from_str(&line)?;
    Ok(resp)
}

async fn open_control_pipe_with_retry(
) -> anyhow::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    const ERROR_PIPE_BUSY: i32 = 231;
    for attempt in 0..20 {
        match ClientOptions::new().open(CONTROL_PIPE_NAME) {
            Ok(client) => return Ok(client),
            Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempt < 19 => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err) => {
                return Err(err).context("core-daemon is not running (failed to open control pipe)");
            }
        }
    }
    anyhow::bail!("core-daemon control pipe remained busy")
}

fn invoked_as_docker() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().eq_ignore_ascii_case("docker")))
        .unwrap_or(false)
}

fn find_sibling_exe(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(name);
    candidate.exists().then_some(candidate)
}

/// Finds an external `docker.exe` binary on `PATH`, skipping NanoDock's own directory
/// so a `docker.exe` shim (hardlink/copy of `nanodock.exe`) never recurses into itself.
fn find_real_docker_exe() -> Option<PathBuf> {
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

/// Converts a Windows drive path (`C:\Users\dev`) to `/mnt/c/Users/dev`.
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

/// Rewrites `C:\host\dir:/container/dir[:ro]` -> `/mnt/c/host/dir:/container/dir[:ro]`.
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

/// Translates Windows paths in common Docker CLI flags when invoking `docker` inside WSL2.
fn translate_args_for_wsl(args: &[OsString]) -> Vec<OsString> {
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
