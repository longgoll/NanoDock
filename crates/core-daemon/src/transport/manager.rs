use std::sync::Arc;
use std::time::Duration;

use shared_types::{DaemonEvent, EngineState};
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{info, warn};
use wsl_bridge::{graceful_hibernate, wake_distro, WslExecutor};

use crate::transport::hyperv::{try_connect_hyperv_transport, DEFAULT_VSOCK_PORT};
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
    /// Automatically recovers with brief backoff if the underlying `guest-agent` or WSL2 session restarted.
    pub async fn open_docker_stream(&self) -> anyhow::Result<BoxedStream> {
        let mut last_err = None;
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }
            let transport = match self.ensure_connected().await {
                Ok(t) => t,
                Err(err) => {
                    warn!(attempt, error = %err, "Transport connect failed; retrying");
                    last_err = Some(err);
                    self.close_transport().await;
                    continue;
                }
            };
            match transport.open_stream().await {
                Ok(stream) => return Ok(stream),
                Err(err) => {
                    warn!(attempt, error = %err, "Stream open failed; reconnecting transport");
                    last_err = Some(err);
                    self.close_transport().await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("failed to open docker stream")))
    }

    /// Explicitly wakes the WSL2 distro under the transport lock so `StartEngine`
    /// cannot race with `hibernate()` on `EngineState` transitions.
    pub async fn start_engine(&self) -> anyhow::Result<()> {
        let _guard = self.active.lock().await;
        self.set_state(EngineState::Starting).await;
        info!(distro = %self.distro_name, "Starting WSL2 micro-distro");
        if let Err(err) = wake_distro(self.wsl.as_ref(), &self.distro_name).await {
            self.set_state(EngineState::Error).await;
            return Err(err.into());
        }
        self.set_state(EngineState::Running).await;
        Ok(())
    }

    /// Atomically closes the active transport and gracefully hibernates the WSL2 distro
    /// while holding the transport lock so concurrent auto-wake requests wait cleanly
    /// and cannot interleave with `EngineState` updates.
    pub async fn hibernate(&self, distro_name: &str) -> Result<(), wsl_bridge::WslError> {
        let mut guard = self.active.lock().await;
        self.set_state(EngineState::Stopping).await;
        if let Some(t) = guard.take() {
            let _ = t.close().await;
        }
        let res = graceful_hibernate(self.wsl.as_ref(), distro_name).await;
        match &res {
            Ok(()) => self.set_state(EngineState::Stopped).await,
            Err(_) => self.set_state(EngineState::Error).await,
        }
        res
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
            if let Some(hv) = try_connect_hyperv_transport(DEFAULT_VSOCK_PORT).await {
                hv
            } else {
                Arc::new(YamuxStdioTransport::spawn(&self.distro_name)?)
            };
        *guard = Some(Arc::clone(&transport));
        self.set_state(EngineState::Running).await;

        Ok(transport)
    }

    async fn set_state(&self, next: EngineState) {
        if let Some(state_lock) = &self.state {
            let mut st = state_lock.write().await;
            if *st != next {
                *st = next;
                if let Some(tx) = &self.events {
                    let _ = tx.send(DaemonEvent::StateChanged(next));
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

        mgr.set_state(EngineState::Running).await;
        assert_eq!(*state.read().await, EngineState::Running);
        assert_eq!(
            rx.try_recv().unwrap(),
            DaemonEvent::StateChanged(EngineState::Running)
        );

        mgr.set_state(EngineState::Running).await;
        assert!(rx.try_recv().is_err());

        mgr.hibernate("nanodock").await.unwrap();
        assert_eq!(*state.read().await, EngineState::Stopped);
        assert_eq!(
            rx.try_recv().unwrap(),
            DaemonEvent::StateChanged(EngineState::Stopping)
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            DaemonEvent::StateChanged(EngineState::Stopped)
        );
    }
}
