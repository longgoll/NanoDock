use std::sync::Arc;

use anyhow::Context as _;
use shared_types::{ControlRequest, ControlResponse};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tracing::{error, info, warn};
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;

use super::dispatch::dispatch_request;
use super::streams::{
    stream_container_logs, stream_container_stats, stream_daemon_events, stream_exec_session,
    stream_pull_image,
};

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
        .reject_remote_clients(true)
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
                        server = match ServerOptions::new()
                            .reject_remote_clients(true)
                            .create(pipe_name)
                        {
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

async fn handle_control_client<E: WslExecutor + 'static>(
    pipe: NamedPipeServer,
    ctx: Arc<AppContext<E>>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let resp = match serde_json::from_str::<ControlRequest>(&line) {
            Ok(ControlRequest::SubscribeEvents) => {
                stream_daemon_events(&mut writer, &ctx).await?;
                return Ok(());
            }
            Ok(ControlRequest::StartExecSession {
                id,
                shell,
                cols,
                rows,
            }) => {
                stream_exec_session(
                    &mut lines,
                    &mut writer,
                    &ctx,
                    &id,
                    shell.as_deref(),
                    cols,
                    rows,
                )
                .await?;
                return Ok(());
            }
            Ok(ControlRequest::StreamContainerLogs { id, tail }) => {
                stream_container_logs(&mut writer, &ctx, &id, tail).await?;
                return Ok(());
            }
            Ok(ControlRequest::StreamContainerStats { id }) => {
                stream_container_stats(&mut writer, &ctx, &id).await?;
                return Ok(());
            }
            Ok(ControlRequest::StreamPullImage { reference }) => {
                stream_pull_image(&mut writer, &ctx, &reference).await?;
                return Ok(());
            }
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use shared_types::{EngineConfig, EngineState};
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
            Ok(self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| ok("")))
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
                assert_eq!(running_containers, 0);
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
            Arc::new(MockWsl::new(vec![
                ok(""),
                ok(env!("CARGO_PKG_VERSION")),
                fail("init failed"),
            ])),
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
