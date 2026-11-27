use std::sync::atomic::Ordering;

use shared_types::{ControlRequest, ControlResponse, DaemonEvent, EngineState};
use tracing::{info, warn};
use wsl_bridge::{
    ensure_wslconfig, graceful_hibernate, list_user_wsl_distros, set_distro_docker_integration,
    trim_distro, wake_distro, WslExecutor,
};

use crate::app_context::AppContext;
use crate::config_store;
use crate::log_store;
use crate::watchers::disk_reclaim::parse_fstrim_bytes;

use super::docker_dispatch::dispatch_docker_request;

pub(super) async fn dispatch_request<E: WslExecutor>(
    req: ControlRequest,
    ctx: &AppContext<E>,
) -> ControlResponse {
    match req {
        ControlRequest::GetStatus => {
            let state = *ctx.state.read().await;
            let active = ctx.active_streams.load(Ordering::SeqCst);
            ControlResponse::Status {
                state,
                running_containers: active,
                vhdx_size_bytes: None,
            }
        }
        ControlRequest::StartEngine => {
            let distro = ctx.config.read().await.distro_name.clone();
            ctx.set_state(EngineState::Starting).await;
            match wake_distro(ctx.wsl.as_ref(), &distro).await {
                Ok(()) => {
                    ctx.set_state(EngineState::Running).await;
                    check_guest_agent_version(ctx, &distro).await;
                    ControlResponse::Ack
                }
                Err(err) => {
                    ctx.set_state(EngineState::Error).await;
                    ControlResponse::Error {
                        message: err.to_string(),
                    }
                }
            }
        }
        ControlRequest::StopEngine => {
            let distro = ctx.config.read().await.distro_name.clone();
            ctx.set_state(EngineState::Stopping).await;
            ctx.transport.close_transport().await;
            match graceful_hibernate(ctx.wsl.as_ref(), &distro).await {
                Ok(()) => {
                    ctx.hibernate_count.fetch_add(1, Ordering::Relaxed);
                    ctx.set_state(EngineState::Stopped).await;
                    ControlResponse::Ack
                }
                Err(err) => {
                    ctx.set_state(EngineState::Error).await;
                    ControlResponse::Error {
                        message: err.to_string(),
                    }
                }
            }
        }
        ControlRequest::ShutdownDaemon => {
            let current_state = *ctx.state.read().await;
            if current_state != EngineState::Stopped {
                let distro = ctx.config.read().await.distro_name.clone();
                ctx.set_state(EngineState::Stopping).await;
                ctx.transport.close_transport().await;
                if let Err(err) = graceful_hibernate(ctx.wsl.as_ref(), &distro).await {
                    warn!(error = %err, "Failed to gracefully hibernate during daemon shutdown");
                }
                ctx.set_state(EngineState::Stopped).await;
            }
            info!("Graceful daemon shutdown requested via Control RPC");
            ctx.shutdown.cancel();
            ControlResponse::Ack
        }
        ControlRequest::TrimDisk => {
            let distro = ctx.config.read().await.distro_name.clone();
            match trim_distro(ctx.wsl.as_ref(), &distro).await {
                Ok(output) => {
                    if let Some(bytes) = parse_fstrim_bytes(&output) {
                        ctx.total_reclaimed_bytes.fetch_add(bytes, Ordering::Relaxed);
                    }
                    ControlResponse::Ack
                }
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::GetConfig => ControlResponse::Config(ctx.config.read().await.clone()),
        ControlRequest::UpdateConfig(next) => {
            if let Err(err) = config_store::save_config(&ctx.config_path, &next) {
                return ControlResponse::Error {
                    message: format!("failed to persist config: {err}"),
                };
            }
            if let Some(wsl_cfg_path) = ctx.wslconfig_path.as_deref() {
                let supports_win11 = config_store::supports_win11_wsl2_features();
                if let Err(err) = ensure_wslconfig(wsl_cfg_path, &next.wsl, supports_win11) {
                    return ControlResponse::Error {
                        message: format!("failed to update .wslconfig: {err}"),
                    };
                }
            }
            if let Err(err) = ctx.startup.set_enabled(next.auto_start_on_login) {
                return ControlResponse::Error {
                    message: format!("failed to update startup setting: {err}"),
                };
            }
            *ctx.config.write().await = next;
            ControlResponse::Ack
        }
        ControlRequest::GetEngineLogs { lines } => {
            let daemon_log = log_store::read_tail_lines(&log_store::default_log_path(), lines);
            let dockerd_log = if *ctx.state.read().await == EngineState::Running {
                let distro = ctx.config.read().await.distro_name.clone();
                let n = lines.max(1);
                let cmd = format!("tail -n {n} /var/log/nanodock/dockerd.log 2>/dev/null || true");
                ctx.wsl
                    .run(&["-d", &distro, "-e", "sh", "-c", &cmd])
                    .await
                    .map(|o| o.stdout)
                    .unwrap_or_default()
            } else {
                String::new()
            };
            ControlResponse::EngineLogs {
                daemon_log,
                dockerd_log,
            }
        }
        ControlRequest::GetEngineMetrics => ControlResponse::EngineMetrics(ctx.metrics()),
        ControlRequest::SetKeepAwake { enabled } => {
            ctx.keep_awake.store(enabled, Ordering::Relaxed);
            if enabled {
                ctx.idle_remaining_secs.store(u64::MAX, Ordering::Relaxed);
            }
            ControlResponse::Ack
        }
        ControlRequest::ListWslDistros => {
            let distro = ctx.config.read().await.distro_name.clone();
            match list_user_wsl_distros(ctx.wsl.as_ref(), &distro).await {
                Ok(items) => ControlResponse::WslDistros(items),
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::SetWslDistroIntegration { distro, enabled } => {
            match set_distro_docker_integration(ctx.wsl.as_ref(), &distro, enabled).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        other => dispatch_docker_request(other, ctx).await,
    }
}

async fn check_guest_agent_version<E: WslExecutor>(ctx: &AppContext<E>, distro: &str) {
    let expected = env!("CARGO_PKG_VERSION");
    if let Ok(out) = ctx
        .wsl
        .run(&["-d", distro, "-e", "/usr/local/bin/nanodock-agent", "--version"])
        .await
    {
        let actual = out.stdout.trim();
        if out.success && !actual.is_empty() && actual != expected {
            let msg = format!(
                "WSL2 guest-agent version ({actual}) differs from daemon ({expected}). Consider updating RootFS."
            );
            warn!(%actual, %expected, "Guest-agent version mismatch");
            let _ = ctx.events.send(DaemonEvent::Warning { message: msg });
        }
    }
}
