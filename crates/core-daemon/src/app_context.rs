use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use shared_types::{DaemonEvent, EngineConfig, EngineState};
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use wsl_bridge::WslExecutor;

use crate::startup::{NoopStartup, StartupManager};
use crate::transport::TransportManager;

/// Shared runtime state and coordination primitives for `core-daemon`.
pub struct AppContext<E: WslExecutor> {
    pub config: RwLock<EngineConfig>,
    pub config_path: PathBuf,
    pub wslconfig_path: Option<PathBuf>,
    pub state: RwLock<EngineState>,
    pub wsl: Arc<E>,
    pub startup: Arc<dyn StartupManager>,
    pub transport: Arc<TransportManager<E>>,
    pub active_streams: Arc<AtomicUsize>,
    pub shutdown: CancellationToken,
    pub events: broadcast::Sender<DaemonEvent>,
}

impl<E: WslExecutor> AppContext<E> {
    #[allow(dead_code)]
    pub fn new(config: EngineConfig, wsl: Arc<E>) -> Arc<Self> {
        let test_cfg =
            std::env::temp_dir().join(format!("nanodock-test-{}.json", std::process::id()));
        Self::with_dependencies(config, test_cfg, None, wsl, Arc::new(NoopStartup))
    }

    pub fn with_dependencies(
        config: EngineConfig,
        config_path: PathBuf,
        wslconfig_path: Option<PathBuf>,
        wsl: Arc<E>,
        startup: Arc<dyn StartupManager>,
    ) -> Arc<Self> {
        let transport = Arc::new(TransportManager::new(
            Arc::clone(&wsl),
            config.distro_name.clone(),
        ));
        let (events, _) = broadcast::channel(64);
        Arc::new(Self {
            config: RwLock::new(config),
            config_path,
            wslconfig_path,
            state: RwLock::new(EngineState::Stopped),
            wsl,
            startup,
            transport,
            active_streams: Arc::new(AtomicUsize::new(0)),
            shutdown: CancellationToken::new(),
            events,
        })
    }

    pub async fn set_state(&self, next: EngineState) {
        let mut guard = self.state.write().await;
        if *guard != next {
            *guard = next;
            let _ = self.events.send(DaemonEvent::StateChanged(next));
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
    async fn set_state_emits_only_on_change() {
        let ctx = AppContext::new(EngineConfig::default(), Arc::new(NoopWsl));
        let mut rx = ctx.events.subscribe();

        // Initial state is Stopped; setting Stopped again emits nothing.
        ctx.set_state(EngineState::Stopped).await;
        assert!(rx.try_recv().is_err());

        ctx.set_state(EngineState::Running).await;
        assert_eq!(
            rx.try_recv().unwrap(),
            DaemonEvent::StateChanged(EngineState::Running)
        );
        assert!(rx.try_recv().is_err());
    }
}
