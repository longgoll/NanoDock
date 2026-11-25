use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;
use wsl_bridge::{wake_distro, WslExecutor};

use crate::transport::traits::{BoxedStream, StreamTransport};
use crate::transport::yamux_stdio::YamuxStdioTransport;

/// Manages the active multiplexed `StreamTransport` and handles Auto-Wake on first request.
pub struct TransportManager<E: WslExecutor> {
    wsl: Arc<E>,
    distro_name: String,
    active: Mutex<Option<Arc<dyn StreamTransport>>>,
}

impl<E: WslExecutor> TransportManager<E> {
    pub fn new(wsl: Arc<E>, distro_name: String) -> Self {
        Self {
            wsl,
            distro_name,
            active: Mutex::new(None),
        }
    }

    /// Returns an open multiplexed stream to `/var/run/docker.sock`, waking the distro if needed.
    pub async fn open_docker_stream(&self) -> anyhow::Result<BoxedStream> {
        let transport = self.ensure_connected().await?;
        match transport.open_stream().await {
            Ok(stream) => Ok(stream),
            Err(_) => {
                // Reconnect once if the distro was externally terminated.
                self.close_transport().await;
                let fresh = self.ensure_connected().await?;
                fresh.open_stream().await
            }
        }
    }

    pub async fn ensure_connected(&self) -> anyhow::Result<Arc<dyn StreamTransport>> {
        let mut guard = self.active.lock().await;
        if let Some(existing) = guard.as_ref() {
            if existing.is_alive() {
                return Ok(Arc::clone(existing));
            }
        }

        info!(distro = %self.distro_name, "Auto-waking WSL2 micro-distro");
        wake_distro(self.wsl.as_ref(), &self.distro_name).await?;

        let transport: Arc<dyn StreamTransport> =
            Arc::new(YamuxStdioTransport::spawn(&self.distro_name)?);
        *guard = Some(Arc::clone(&transport));
        Ok(transport)
    }

    pub async fn close_transport(&self) {
        let mut guard = self.active.lock().await;
        if let Some(t) = guard.take() {
            let _ = t.close().await;
        }
    }
}
