use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use shared_types::{DaemonEvent, EngineConfig, EngineMetrics, EngineState};
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
    pub state: Arc<RwLock<EngineState>>,
    pub wsl: Arc<E>,
    pub startup: Arc<dyn StartupManager>,
    pub transport: Arc<TransportManager<E>>,
    pub active_streams: Arc<AtomicUsize>,
    pub hibernate_count: Arc<AtomicU64>,
    pub total_reclaimed_bytes: Arc<AtomicU64>,
    pub keep_awake: Arc<AtomicBool>,
    pub idle_remaining_secs: Arc<AtomicU64>,
    pub started_at: Instant,
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
        let state = Arc::new(RwLock::new(EngineState::Stopped));
        let (events, _) = broadcast::channel(64);
        let transport = Arc::new(TransportManager::with_state_tracker(
            Arc::clone(&wsl),
            config.distro_name.clone(),
            Arc::clone(&state),
            events.clone(),
        ));
        Arc::new(Self {
            config: RwLock::new(config),
            config_path,
            wslconfig_path,
            state,
            wsl,
            startup,
            transport,
            active_streams: Arc::new(AtomicUsize::new(0)),
            hibernate_count: Arc::new(AtomicU64::new(0)),
            total_reclaimed_bytes: Arc::new(AtomicU64::new(0)),
            keep_awake: Arc::new(AtomicBool::new(false)),
            idle_remaining_secs: Arc::new(AtomicU64::new(u64::MAX)),
            started_at: Instant::now(),
            shutdown: CancellationToken::new(),
            events,
        })
    }

    pub fn metrics(&self) -> EngineMetrics {
        let remaining_raw = self.idle_remaining_secs.load(Ordering::Relaxed);
        let keep_awake = self.keep_awake.load(Ordering::Relaxed);
        EngineMetrics {
            hibernate_count: self.hibernate_count.load(Ordering::Relaxed),
            total_reclaimed_bytes: self.total_reclaimed_bytes.load(Ordering::Relaxed),
            uptime_secs: self.started_at.elapsed().as_secs(),
            keep_awake,
            idle_remaining_secs: if keep_awake || remaining_raw == u64::MAX {
                None
            } else {
                Some(remaining_raw)
            },
        }
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
