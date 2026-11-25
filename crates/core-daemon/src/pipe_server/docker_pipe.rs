use std::sync::Arc;

use anyhow::Context as _;
use tokio::net::windows::named_pipe::ServerOptions;
use tracing::{error, info, warn};
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::proxy::handle_client_stream;

pub const DOCKER_PIPE_NAME: &str = r"\\.\pipe\docker_engine";

/// Listens on `\\.\pipe\docker_engine`, pre-allocating a new pipe instance on every
/// accepted connection to prevent `ERROR_PIPE_BUSY` under parallel Docker CLI/Compose loads.
pub async fn serve_docker_pipe<E: WslExecutor + 'static>(ctx: Arc<AppContext<E>>) -> anyhow::Result<()> {
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(DOCKER_PIPE_NAME)
        .with_context(|| {
            format!(
                "failed to bind {DOCKER_PIPE_NAME}; if Docker Desktop (com.docker.service) is running, please stop it first"
            )
        })?;

    info!(pipe = DOCKER_PIPE_NAME, "Docker Engine Named Pipe server listening");

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => {
                info!("Shutting down Docker Named Pipe server");
                break;
            }
            res = server.connect() => {
                match res {
                    Ok(()) => {
                        let connected = server;
                        // Immediately create the next pipe instance before handling the client.
                        server = match ServerOptions::new().create(DOCKER_PIPE_NAME) {
                            Ok(next) => next,
                            Err(err) => {
                                error!(error = %err, "Failed to allocate next Named Pipe instance");
                                break;
                            }
                        };

                        let transport = Arc::clone(&ctx.transport);
                        let active = Arc::clone(&ctx.active_streams);
                        tokio::spawn(async move {
                            if let Err(err) = handle_client_stream(connected, transport, active).await {
                                warn!(error = %err, "Client proxy stream ended with error");
                            }
                        });
                    }
                    Err(err) => {
                        error!(error = %err, "Named Pipe connect error");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
