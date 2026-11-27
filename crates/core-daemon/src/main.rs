mod app_context;
mod config_store;
mod control_rpc;
mod lifecycle;
mod log_store;
mod pipe_server;
mod proxy;
mod startup;
mod transport;
mod watchers;

use std::path::PathBuf;
use std::sync::Arc;

use tracing::{error, info, warn};
use wsl_bridge::{ensure_distro_imported, ensure_wslconfig, BootstrapOutcome, SystemWslExecutor};

use crate::app_context::AppContext;
use crate::startup::WindowsRegistryStartup;
use crate::watchers::{DiskReclaimHook, DockerEventHook, UiEventHook};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_writer = log_store::TeeLogWriter::open(&log_store::default_log_path());
    tracing_subscriber::fmt()
        .with_writer(log_writer)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "core_daemon=info,wsl_bridge=info".into()),
        )
        .init();

    let config_path = config_store::default_config_path();
    let wslconfig_path = config_store::wslconfig_path();
    let config = config_store::load_or_default(&config_path);
    sync_wslconfig_on_startup(&config, wslconfig_path.as_deref());

    let wsl = Arc::new(SystemWslExecutor);
    let startup = Arc::new(WindowsRegistryStartup);
    let ctx = AppContext::with_dependencies(config, config_path, wslconfig_path, wsl, startup);

    auto_bootstrap_distro(&ctx).await;

    let disk_hook: Arc<dyn DockerEventHook> = DiskReclaimHook::spawn(Arc::clone(&ctx));
    let ui_hook: Arc<dyn DockerEventHook> = UiEventHook::new(Arc::clone(&ctx));
    let hooks = vec![disk_hook, ui_hook];

    let docker_task = tokio::spawn(pipe_server::serve_docker_pipe(Arc::clone(&ctx)));
    let control_task = tokio::spawn(control_rpc::serve_control_pipe(Arc::clone(&ctx)));
    let hibernate_task = tokio::spawn(lifecycle::run_hibernate_watcher(Arc::clone(&ctx)));
    let events_task = tokio::spawn(watchers::run_event_watchers(Arc::clone(&ctx), hooks));

    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received");
    ctx.shutdown.cancel();

    for handle in [docker_task, control_task] {
        if let Err(err) = handle.await {
            error!(error = %err, "Pipe server task panicked");
        }
    }
    let _ = hibernate_task.await;
    let _ = events_task.await;

    Ok(())
}

fn sync_wslconfig_on_startup(
    config: &shared_types::EngineConfig,
    wslconfig_path: Option<&std::path::Path>,
) {
    if let Some(wsl_cfg_path) = wslconfig_path {
        let supports_win11 = config_store::supports_win11_wsl2_features();
        if let Err(err) = ensure_wslconfig(wsl_cfg_path, &config.wsl, supports_win11) {
            warn!(error = %err, path = %wsl_cfg_path.display(), "Failed to sync .wslconfig on startup");
        }
    }
}

async fn auto_bootstrap_distro(ctx: &AppContext<SystemWslExecutor>) {
    let distro_name = ctx.config.read().await.distro_name.clone();
    let install_dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("NanoDock")
        .join("distro");
    let rootfs = resolve_rootfs_path();

    match ensure_distro_imported(ctx.wsl.as_ref(), &distro_name, &install_dir, &rootfs).await {
        Ok(BootstrapOutcome::AlreadyReady) => {
            info!(distro = %distro_name, "WSL2 micro-distro is ready");
        }
        Ok(BootstrapOutcome::ImportedAndConfigured) => {
            info!(distro = %distro_name, dir = %install_dir.display(), "Imported WSL2 micro-distro");
        }
        Err(err) => {
            warn!(error = %err, rootfs = %rootfs.display(), "Auto-bootstrap skipped or failed");
        }
    }
}

fn resolve_rootfs_path() -> PathBuf {
    if let Some(env_path) = std::env::var_os("NANODOCK_ROOTFS_PATH") {
        return PathBuf::from(env_path);
    }

    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("micro-distro.tar.gz"));
            candidates.push(dir.join("resources").join("micro-distro.tar.gz"));
        }
    }
    candidates.push(PathBuf::from("rootfs-builder/out/micro-distro.tar.gz"));

    candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("micro-distro.tar.gz"))
}
