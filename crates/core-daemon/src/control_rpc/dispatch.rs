use std::sync::atomic::Ordering;

use shared_types::{
    default_turbo_excludes, ControlRequest, ControlResponse, DaemonEvent, EngineState,
    TurboMountRule,
};
use tracing::{info, warn};
use wsl_bridge::{
    ensure_wslconfig, list_user_wsl_distros, set_distro_docker_integration, trim_distro,
    WslExecutor,
};

use crate::app_context::AppContext;
use crate::config_store;
use crate::lifecycle::docker_api::count_running_containers;
use crate::log_store;
use crate::watchers::disk_reclaim::parse_fstrim_bytes;

use super::docker_dispatch::dispatch_docker_request;

pub(super) async fn dispatch_request<E: WslExecutor + 'static>(
    req: ControlRequest,
    ctx: &AppContext<E>,
) -> ControlResponse {
    match req {
        ControlRequest::GetStatus => {
            let state = *ctx.state.read().await;
            let running_containers = if state == EngineState::Running {
                count_running_containers(&ctx.transport).await.unwrap_or(0)
            } else {
                0
            };
            ControlResponse::Status {
                state,
                running_containers,
                vhdx_size_bytes: None,
            }
        }
        ControlRequest::StartEngine => {
            let distro = ctx.config.read().await.distro_name.clone();
            match ctx.transport.start_engine().await {
                Ok(()) => {
                    check_guest_agent_version(ctx, &distro).await;
                    ControlResponse::Ack
                }
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::StopEngine => {
            let distro = ctx.config.read().await.distro_name.clone();
            match ctx.transport.hibernate(&distro).await {
                Ok(()) => {
                    ctx.hibernate_count.fetch_add(1, Ordering::Relaxed);
                    ControlResponse::Ack
                }
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::ShutdownDaemon => {
            let current_state = *ctx.state.read().await;
            if current_state != EngineState::Stopped {
                let distro = ctx.config.read().await.distro_name.clone();
                if let Err(err) = ctx.transport.hibernate(&distro).await {
                    warn!(error = %err, "Failed to gracefully hibernate during daemon shutdown");
                }
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
        ControlRequest::GetDiagnosticReport { lines } => {
            let (markdown, has_crash_report) =
                crate::diagnostics::build_diagnostic_report(ctx, lines.unwrap_or(120)).await;
            ControlResponse::DiagnosticReport {
                markdown,
                has_crash_report,
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
        ControlRequest::ListTurboMounts => {
            let rules = ctx.config.read().await.turbo_mounts.clone();
            ControlResponse::TurboMounts(ctx.turbo_sync.list_statuses(&rules).await)
        }
        ControlRequest::AddTurboMount {
            windows_path,
            excludes,
        } => {
            let trimmed = windows_path.trim().to_string();
            if trimmed.is_empty() {
                return ControlResponse::Error {
                    message: "windows_path cannot be empty".to_string(),
                };
            }
            let id = crate::turbo_sync::make_turbo_rule_id(&trimmed);
            let rule = TurboMountRule {
                id: id.clone(),
                windows_path: trimmed.clone(),
                wsl_path: format!("/var/lib/nanodock/turbo/{id}"),
                excludes: excludes.unwrap_or_else(default_turbo_excludes),
                enabled: true,
            };
            let (distro, rules) = {
                let mut cfg = ctx.config.write().await;
                if cfg
                    .turbo_mounts
                    .iter()
                    .any(|r| r.windows_path.eq_ignore_ascii_case(&trimmed))
                {
                    return ControlResponse::Error {
                        message: format!("Turbo Mount already exists for {trimmed}"),
                    };
                }
                cfg.turbo_mounts.push(rule.clone());
                if let Err(err) = config_store::save_config(&ctx.config_path, &cfg) {
                    warn!(error = %err, "Failed to persist turbo mount config");
                }
                (cfg.distro_name.clone(), cfg.turbo_mounts.clone())
            };
            if *ctx.state.read().await == EngineState::Running {
                ctx.turbo_sync.sync_rule(&distro, &rule).await;
            }
            ControlResponse::TurboMounts(ctx.turbo_sync.list_statuses(&rules).await)
        }
        ControlRequest::RemoveTurboMount { id } => {
            let rules = {
                let mut cfg = ctx.config.write().await;
                cfg.turbo_mounts.retain(|r| r.id != id);
                if let Err(err) = config_store::save_config(&ctx.config_path, &cfg) {
                    warn!(error = %err, "Failed to persist turbo mount removal");
                }
                cfg.turbo_mounts.clone()
            };
            ctx.turbo_sync.remove_rule_state(&id).await;
            ControlResponse::TurboMounts(ctx.turbo_sync.list_statuses(&rules).await)
        }
        ControlRequest::ToggleTurboMount { id, enabled } => {
            let (distro, matched_rule, rules) = {
                let mut cfg = ctx.config.write().await;
                let mut found = None;
                for rule in &mut cfg.turbo_mounts {
                    if rule.id == id {
                        rule.enabled = enabled;
                        found = Some(rule.clone());
                    }
                }
                if let Err(err) = config_store::save_config(&ctx.config_path, &cfg) {
                    warn!(error = %err, "Failed to persist turbo mount toggle");
                }
                (cfg.distro_name.clone(), found, cfg.turbo_mounts.clone())
            };
            if enabled {
                if let Some(rule) = matched_rule {
                    if *ctx.state.read().await == EngineState::Running {
                        ctx.turbo_sync.sync_rule(&distro, &rule).await;
                    }
                }
            }
            ControlResponse::TurboMounts(ctx.turbo_sync.list_statuses(&rules).await)
        }
        ControlRequest::TriggerTurboSync { id } => {
            let (distro, rules) = {
                let cfg = ctx.config.read().await;
                (cfg.distro_name.clone(), cfg.turbo_mounts.clone())
            };
            if *ctx.state.read().await == EngineState::Running {
                for rule in &rules {
                    if rule.enabled && id.as_ref().is_none_or(|target| target == &rule.id) {
                        ctx.turbo_sync.sync_rule(&distro, rule).await;
                    }
                }
            }
            ControlResponse::TurboMounts(ctx.turbo_sync.list_statuses(&rules).await)
        }
        ControlRequest::ListLocalProxyRoutes => {
            let routes = ctx.local_proxy_routes.read().await.clone();
            ControlResponse::LocalProxyRoutes(routes)
        }
        other => dispatch_docker_request(other, ctx).await,
    }
}

async fn check_guest_agent_version<E: WslExecutor + 'static>(ctx: &AppContext<E>, distro: &str) {
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
