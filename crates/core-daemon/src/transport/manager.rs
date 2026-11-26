use std::sync::Arc;

use shared_types::{DaemonEvent, EngineState};
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::info;
use wsl_bridge::{wake_distro, WslExecutor};

use crate::transport::traits::{BoxedStream, StreamTransport};
use crate::transport::yamux_stdio::YamuxStdioTransport;

/// Manages the active multiplexed `StreamTransport` and handles Auto-Wake on first request.
pub struct TransportManager<E: WslExecutor> {
    wsl: Arc<E>,
    distro_name: String,
    active: Mutex<Option<Arc<dyn StreamTransport>>>,
    state: Option<Arc<RwLock<EngineState>>>,
    events: Option<broadcast::Sender<DaemonEvent>>,
}

impl<E: WslExecutor> TransportManager<E> {
    #[allow(dead_code)]
    pub fn new(wsl: Arc<E>, distro_name: String) -> Self {
        Self {
            wsl,
            distro_name,
            active: Mutex::new(None),
            state: None,
            events: None,
        }
    }

    pub fn with_state_tracker(
        wsl: Arc<E>,
        distro_name: String,
        state: Arc<RwLock<EngineState>>,
        events: broadcast::Sender<DaemonEvent>,
    ) -> Self {
        Self {
            wsl,
            distro_name,
            active: Mutex::new(None),
            state: Some(state),
            events: Some(events),
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
        self.mark_running().await;

        Ok(transport)
    }

    async fn mark_running(&self) {
        if let Some(state_lock) = &self.state {
            let mut st = state_lock.write().await;
            if *st != EngineState::Running {
                *st = EngineState::Running;
                if let Some(tx) = &self.events {
                    let _ = tx.send(DaemonEvent::StateChanged(EngineState::Running));
                }
            }
        }
    }

    pub async fn close_transport(&self) {
        let mut guard = self.active.lock().await;
        if let Some(t) = guard.take() {
            let _ = t.close().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use wsl_bridge::{CommandOutput, WslError};

    use super::*;

    struct NoopWsl;

    #[async_trait]
    impl WslExecutor for NoopWsl {
        async fn run(&self, _args: &[&str]) -> Result<CommandOutput, WslError> {
            Ok(CommandOutput {
                success: true,
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    #[tokio::test]
    async fn mark_running_updates_state_and_emits_event_once() {
        let state = Arc::new(RwLock::new(EngineState::Stopped));
        let (tx, mut rx) = broadcast::channel(8);
        let mgr = TransportManager::with_state_tracker(
            Arc::new(NoopWsl),
            "nanodock".to_string(),
            Arc::clone(&state),
            tx,
        );

        mgr.mark_running().await;
        assert_eq!(*state.read().await, EngineState::Running);
        assert_eq!(
            rx.try_recv().unwrap(),
            DaemonEvent::StateChanged(EngineState::Running)
        );

        mgr.mark_running().await;
        assert!(rx.try_recv().is_err());
    }
}
