use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Context as _;
use shared_types::{ControlRequest, ControlResponse, EngineState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tracing::{error, info, warn};
use wsl_bridge::{ensure_wslconfig, graceful_hibernate, trim_distro, wake_distro, WslExecutor};

use crate::app_context::AppContext;
use crate::config_store;
use crate::lifecycle::{docker_api, networks};

pub const CONTROL_PIPE_NAME: &str = r"\\.\pipe\nanodock_ctl";

/// Serves newline-delimited JSON `ControlRequest`s from Tauri UI or NanoDock CLI.
pub async fn serve_control_pipe<E: WslExecutor + 'static>(
    ctx: Arc<AppContext<E>>,
) -> anyhow::Result<()> {
    serve_control_pipe_at(ctx, CONTROL_PIPE_NAME).await
}

async fn serve_control_pipe_at<E: WslExecutor + 'static>(
    ctx: Arc<AppContext<E>>,
    pipe_name: &str,
) -> anyhow::Result<()> {
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(pipe_name)
        .with_context(|| format!("failed to bind {pipe_name}"))?;

    info!(pipe = pipe_name, "Control RPC pipe listening");

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => {
                info!("Shutting down Control RPC server");
                break;
            }
            res = server.connect() => {
                match res {
                    Ok(()) => {
                        let connected = server;
                        server = match ServerOptions::new().create(pipe_name) {
                            Ok(next) => next,
                            Err(err) => {
                                error!(error = %err, "Failed to allocate next Control pipe instance");
                                break;
                            }
                        };
                        let ctx_clone = Arc::clone(&ctx);
                        tokio::spawn(async move {
                            if let Err(err) = handle_control_client(connected, ctx_clone).await {
                                warn!(error = %err, "Control RPC client error");
                            }
                        });
                    }
                    Err(err) => {
                        error!(error = %err, "Control pipe connect error");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_control_client<E: WslExecutor>(
    pipe: NamedPipeServer,
    ctx: Arc<AppContext<E>>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let resp = match serde_json::from_str::<ControlRequest>(&line) {
            Ok(req) => dispatch_request(req, &ctx).await,
            Err(err) => ControlResponse::Error {
                message: format!("invalid request: {err}"),
            },
        };
        let mut encoded = serde_json::to_string(&resp)?;
        encoded.push('\n');
        writer.write_all(encoded.as_bytes()).await?;
    }

    Ok(())
}

async fn dispatch_request<E: WslExecutor>(
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
                    ctx.set_state(EngineState::Stopped).await;
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
                Ok(_) => ControlResponse::Ack,
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
        ControlRequest::ListContainers => {
            match docker_api::list_containers(&ctx.transport).await {
                Ok(items) => ControlResponse::Containers(items),
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::ContainerAction { id, action } => {
            match docker_api::perform_container_action(&ctx.transport, &id, action).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::GetContainerLogs { id, tail } => {
            match docker_api::get_container_logs(&ctx.transport, &id, tail.unwrap_or(200)).await {
                Ok(logs) => ControlResponse::ContainerLogs { logs },
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::InspectContainer { id } => {
            match docker_api::inspect_container(&ctx.transport, &id).await {
                Ok(info) => ControlResponse::ContainerInspect(info),
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::GetContainerStats { id } => {
            match docker_api::get_container_stats(&ctx.transport, &id).await {
                Ok(stats) => ControlResponse::ContainerStats(stats),
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::RunContainer(spec) => {
            match docker_api::run_container(&ctx.transport, &spec).await {
                Ok(id) => ControlResponse::ContainerCreated { id },
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::ListImages => match docker_api::list_images(&ctx.transport).await {
            Ok(items) => ControlResponse::Images(items),
            Err(err) => ControlResponse::Error {
                message: err.to_string(),
            },
        },
        ControlRequest::PullImage { reference } => {
            match docker_api::pull_image(&ctx.transport, &reference).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::RemoveImage { id } => {
            match docker_api::remove_image(&ctx.transport, &id).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::PruneImages => match docker_api::prune_images(&ctx.transport).await {
            Ok(reclaimed_bytes) => ControlResponse::PruneResult { reclaimed_bytes },
            Err(err) => ControlResponse::Error {
                message: err.to_string(),
            },
        },
        ControlRequest::ListVolumes => match docker_api::list_volumes(&ctx.transport).await {
            Ok(items) => ControlResponse::Volumes(items),
            Err(err) => ControlResponse::Error {
                message: err.to_string(),
            },
        },
        ControlRequest::CreateVolume { name, driver } => {
            match docker_api::create_volume(&ctx.transport, &name, driver.as_deref()).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::RemoveVolume { name } => {
            match docker_api::remove_volume(&ctx.transport, &name).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::PruneVolumes => match docker_api::prune_volumes(&ctx.transport).await {
            Ok(reclaimed_bytes) => ControlResponse::PruneResult { reclaimed_bytes },
            Err(err) => ControlResponse::Error {
                message: err.to_string(),
            },
        },
        ControlRequest::ListNetworks => match networks::list_networks(&ctx.transport).await {
            Ok(items) => ControlResponse::Networks(items),
            Err(err) => ControlResponse::Error {
                message: err.to_string(),
            },
        },
        ControlRequest::CreateNetwork {
            name,
            driver,
            internal,
        } => {
            match networks::create_network(&ctx.transport, &name, driver.as_deref(), internal)
                .await
            {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::RemoveNetwork { id } => {
            match networks::remove_network(&ctx.transport, &id).await {
                Ok(()) => ControlResponse::Ack,
                Err(err) => ControlResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        ControlRequest::PruneNetworks => match networks::prune_networks(&ctx.transport).await {
            Ok(reclaimed_bytes) => ControlResponse::PruneResult { reclaimed_bytes },
            Err(err) => ControlResponse::Error {
                message: err.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use shared_types::EngineConfig;
    use wsl_bridge::{CommandOutput, WslError};

    use super::*;

    struct MockWsl {
        responses: Mutex<VecDeque<CommandOutput>>,
    }

    impl MockWsl {
        fn new(responses: Vec<CommandOutput>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
            }
        }
    }

    #[async_trait]
    impl WslExecutor for MockWsl {
        async fn run(&self, _args: &[&str]) -> Result<CommandOutput, WslError> {
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
    async fn dispatch_handles_status_and_config_updates() {
        let ctx = AppContext::new(EngineConfig::default(), Arc::new(MockWsl::new(vec![])));
        ctx.active_streams.store(2, Ordering::SeqCst);

        match dispatch_request(ControlRequest::GetStatus, &ctx).await {
            ControlResponse::Status {
                state,
                running_containers,
                ..
            } => {
                assert_eq!(state, EngineState::Stopped);
                assert_eq!(running_containers, 2);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let next_cfg = EngineConfig {
            idle_timeout_secs: Some(60),
            ..Default::default()
        };
        assert!(matches!(
            dispatch_request(ControlRequest::UpdateConfig(next_cfg.clone()), &ctx).await,
            ControlResponse::Ack
        ));
        match dispatch_request(ControlRequest::GetConfig, &ctx).await {
            ControlResponse::Config(cfg) => assert_eq!(cfg, next_cfg),
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn dispatch_start_engine_transitions_state_on_success_and_error() {
        let ctx = AppContext::new(
            EngineConfig::default(),
            Arc::new(MockWsl::new(vec![ok(""), fail("init failed")])),
        );

        assert!(matches!(
            dispatch_request(ControlRequest::StartEngine, &ctx).await,
            ControlResponse::Ack
        ));
        assert_eq!(*ctx.state.read().await, EngineState::Running);

        assert!(matches!(
            dispatch_request(ControlRequest::StartEngine, &ctx).await,
            ControlResponse::Error { .. }
        ));
        assert_eq!(*ctx.state.read().await, EngineState::Error);
    }

    #[tokio::test]
    async fn dispatch_trim_disk_returns_ack_on_success() {
        let ctx = AppContext::new(
            EngineConfig::default(),
            Arc::new(MockWsl::new(vec![ok("/: 0 B trimmed")])),
        );
        assert!(matches!(
            dispatch_request(ControlRequest::TrimDisk, &ctx).await,
            ControlResponse::Ack
        ));
    }

    #[tokio::test]
    async fn dispatch_shutdown_daemon_cancels_token() {
        let ctx = AppContext::new(EngineConfig::default(), Arc::new(MockWsl::new(vec![])));
        assert!(!ctx.shutdown.is_cancelled());
        assert!(matches!(
            dispatch_request(ControlRequest::ShutdownDaemon, &ctx).await,
            ControlResponse::Ack
        ));
        assert!(ctx.shutdown.is_cancelled());
    }

    #[tokio::test]
    async fn named_pipe_roundtrip_serves_status_and_config() {
        use tokio::net::windows::named_pipe::ClientOptions;

        let pipe_name = format!(r"\\.\pipe\nanodock_ctl_test_{}", std::process::id());
        let ctx = AppContext::new(EngineConfig::default(), Arc::new(MockWsl::new(vec![])));
        let server_ctx = Arc::clone(&ctx);
        let server_pipe = pipe_name.clone();
        let server_task = tokio::spawn(async move {
            serve_control_pipe_at(server_ctx, &server_pipe)
                .await
                .unwrap();
        });

        // Give the server a moment to bind the first pipe instance.
        let client = loop {
            match ClientOptions::new().open(&pipe_name) {
                Ok(c) => break c,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
            }
        };

        let (reader, mut writer) = tokio::io::split(client);
        let mut lines = BufReader::new(reader).lines();

        let mut req_line = serde_json::to_string(&ControlRequest::GetStatus).unwrap();
        req_line.push('\n');
        writer.write_all(req_line.as_bytes()).await.unwrap();

        let resp_line = lines.next_line().await.unwrap().unwrap();
        let resp: ControlResponse = serde_json::from_str(&resp_line).unwrap();
        assert!(matches!(
            resp,
            ControlResponse::Status {
                state: EngineState::Stopped,
                running_containers: 0,
                ..
            }
        ));

        ctx.shutdown.cancel();
        let _ = server_task.await;
    }
}
