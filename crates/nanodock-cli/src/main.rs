mod completions;
mod doctor;
mod engine_cmds;
mod passthrough;
mod report;

use std::ffi::OsString;
use std::process::ExitCode;

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
    if passthrough::invoked_as_docker() {
        return passthrough::cmd_passthrough(&args).await;
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
        "status" if args.len() == 1 => engine_cmds::cmd_status().await,
        "start" if args.len() == 1 => engine_cmds::cmd_start().await,
        "stop" if args.len() == 1 => engine_cmds::cmd_stop().await,
        "trim" if args.len() == 1 => engine_cmds::cmd_trim().await,
        "config" if args.len() == 1 => engine_cmds::cmd_config().await,
        "logs" if args.len() <= 2 => {
            let lines = args
                .get(1)
                .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
                .unwrap_or(100);
            engine_cmds::cmd_logs(lines).await
        }
        "doctor" if args.len() == 1 => doctor::cmd_doctor().await,
        "report" => {
            let sub: Vec<String> = args[1..]
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            report::cmd_report(&sub).await
        }
        "turbo" => {
            let sub: Vec<String> = args[1..]
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            engine_cmds::cmd_turbo(&sub).await
        }
        "routes" if args.len() == 1 => engine_cmds::cmd_routes().await,
        "completions" if args.len() <= 2 => {
            let shell = args.get(1).map(|s| s.to_string_lossy().into_owned());
            completions::cmd_completions(shell.as_deref())
        }
        _ => passthrough::cmd_passthrough(&args).await,
    }
}

fn print_help() {
    println!(
        "\
NanoDock CLI - Lightweight WSL2 container engine & Docker-compatible CLI

USAGE:
    nanodock <COMMAND> [ARGS...]

ENGINE COMMANDS:
    start                  Start core-daemon (if needed) and wake the NanoDock WSL2 engine
    stop                   Gracefully hibernate the NanoDock WSL2 engine
    status                 Show engine state, active streams, and status
    trim                   Reclaim unused WSL2 virtual disk space (fstrim / compact)
    config                 Print the active engine configuration
    logs [lines]           Show recent core-daemon and dockerd logs
    doctor                 Run environment & WSL2 health diagnostics
    report [--copy|--out]  Generate redacted Markdown bug report (works offline too)
    turbo <subcommand>     Manage Turbo Mounts (list, add <win_path>, remove <id>, sync [id])
    routes                 Show active *.localhost reverse proxy routes
    completions <shell>    Generate shell completion script (powershell, bash, zsh)
    help                   Show this help message

CONTAINER COMMANDS:
    Any Docker CLI command works directly via NanoDock, for example:
      nanodock run --rm alpine echo hello
      nanodock ps -a
      nanodock images
      nanodock compose up -d"
    );
}
