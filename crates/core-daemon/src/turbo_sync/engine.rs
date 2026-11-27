use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use shared_types::{DaemonEvent, EngineState, TurboMountRule, TurboMountStatus, TurboMountSyncState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tracing::{debug, info, warn};
use wsl_bridge::WslExecutor;

use crate::app_context::AppContext;
use crate::transport::TransportManager;
use crate::turbo_sync::scanner::{scan_diff, FileStamp};
use crate::turbo_sync::tar_stream::build_ustar_archive;
use crate::turbo_sync::watcher::DirWatcher;

const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const WATCH_DEBOUNCE: Duration = Duration::from_millis(100);
const MAX_TAR_BATCH_FILES: usize = 250;

/// Abstraction over streaming a tar archive and file deletions into the WSL2 distro.
#[async_trait]
pub trait WslSyncTarget: Send + Sync {
    async fn extract_tar(
        &self,
        distro: &str,
        wsl_dir: &str,
        tar_bytes: &[u8],
    ) -> anyhow::Result<()>;
    async fn delete_files(
        &self,
        distro: &str,
        wsl_dir: &str,
        rel_paths: &[String],
    ) -> anyhow::Result<()>;
}

/// Real `wsl.exe` implementation used at runtime.
pub struct SystemWslSyncTarget;

#[async_trait]
impl WslSyncTarget for SystemWslSyncTarget {
    async fn extract_tar(
        &self,
        distro: &str,
        wsl_dir: &str,
        tar_bytes: &[u8],
    ) -> anyhow::Result<()> {
        let script = format!("mkdir -p {} && tar -xf - -C {}", sh_quote(wsl_dir), sh_quote(wsl_dir));
        let mut cmd = Command::new("wsl.exe");
        cmd.args(["-d", distro, "-e", "sh", "-c", &script])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(tar_bytes).await?;
            stdin.shutdown().await?;
        }
        let output = child.wait_with_output().await?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("wsl tar extract failed: {}", err.trim());
        }
        Ok(())
    }

    async fn delete_files(
        &self,
        distro: &str,
        wsl_dir: &str,
        rel_paths: &[String],
    ) -> anyhow::Result<()> {
        for chunk in rel_paths.chunks(100) {
            let args: Vec<String> = chunk
                .iter()
                .map(|rel| sh_quote(&format!("{}/{}", wsl_dir.trim_end_matches('/'), rel)))
                .collect();
            let script = format!("rm -f {}", args.join(" "));
            let mut cmd = Command::new("wsl.exe");
            cmd.args(["-d", distro, "-e", "sh", "-c", &script])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            let output = cmd.output().await?;
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("wsl rm failed: {}", err.trim());
            }
        }
        Ok(())
    }
}

/// Multiplexed `yamux` sync target (`\x00NDK` control stream) with automatic fallback
/// to `SystemWslSyncTarget` (`wsl.exe`) when running against an older `guest-agent`.
pub struct MuxWslSyncTarget<E: WslExecutor> {
    transport: Arc<TransportManager<E>>,
    fallback: SystemWslSyncTarget,
    /// 0 = unknown, 1 = supported, 2 = unsupported
    mux_capable: AtomicU8,
}

impl<E: WslExecutor> MuxWslSyncTarget<E> {
    pub fn new(transport: Arc<TransportManager<E>>) -> Self {
        Self {
            transport,
            fallback: SystemWslSyncTarget,
            mux_capable: AtomicU8::new(0),
        }
    }

    async fn check_mux_capable(&self) -> bool {
        match self.mux_capable.load(Ordering::Relaxed) {
            1 => return true,
            2 => return false,
            _ => {}
        }
        let ok = tokio::time::timeout(Duration::from_secs(2), async {
            let mut stream = self.transport.open_docker_stream().await.ok()?;
            stream.write_all(b"\x00NDKPING\n").await.ok()?;
            stream.flush().await.ok()?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.ok()?;
            (line.trim() == "PONG").then_some(())
        })
        .await
        .ok()
        .flatten()
        .is_some();

        self.mux_capable
            .store(if ok { 1 } else { 2 }, Ordering::Relaxed);
        if ok {
            debug!("Turbo Mount using zero-spawn yamux control stream");
        }
        ok
    }

    /// Resets cached `guest-agent` capability so the next sync re-probes `\x00NDKPING`.
    #[allow(dead_code)]
    pub fn reset_mux_probe(&self) {
        self.mux_capable.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl<E: WslExecutor> WslSyncTarget for MuxWslSyncTarget<E> {
    async fn extract_tar(
        &self,
        distro: &str,
        wsl_dir: &str,
        tar_bytes: &[u8],
    ) -> anyhow::Result<()> {
        if self.check_mux_capable().await {
            if let Ok(mut stream) = self.transport.open_docker_stream().await {
                let header = format!("\x00NDKTAR {wsl_dir}\n");
                if stream.write_all(header.as_bytes()).await.is_ok()
                    && stream
                        .write_all(&(tar_bytes.len() as u64).to_be_bytes())
                        .await
                        .is_ok()
                    && stream.write_all(tar_bytes).await.is_ok()
                    && stream.flush().await.is_ok()
                {
                    let mut reader = BufReader::new(stream);
                    let mut resp = String::new();
                    if reader.read_line(&mut resp).await.is_ok() {
                        if resp.trim() == "OK" {
                            return Ok(());
                        }
                        if let Some(err) = resp.trim().strip_prefix("ERR ") {
                            anyhow::bail!("guest-agent tar extract failed: {err}");
                        }
                    }
                }
            }
        }
        self.fallback.extract_tar(distro, wsl_dir, tar_bytes).await
    }

    async fn delete_files(
        &self,
        distro: &str,
        wsl_dir: &str,
        rel_paths: &[String],
    ) -> anyhow::Result<()> {
        if self.check_mux_capable().await {
            if let Ok(mut stream) = self.transport.open_docker_stream().await {
                let header = format!("\x00NDKDEL {wsl_dir}\n");
                let mut payload = Vec::new();
                payload.extend_from_slice(header.as_bytes());
                payload.extend_from_slice(&(rel_paths.len() as u32).to_be_bytes());
                for p in rel_paths {
                    let b = p.as_bytes();
                    payload.extend_from_slice(&(b.len() as u32).to_be_bytes());
                    payload.extend_from_slice(b);
                }
                if stream.write_all(&payload).await.is_ok() && stream.flush().await.is_ok() {
                    let mut reader = BufReader::new(stream);
                    let mut resp = String::new();
                    if reader.read_line(&mut resp).await.is_ok() {
                        if resp.trim() == "OK" {
                            return Ok(());
                        }
                        if let Some(err) = resp.trim().strip_prefix("ERR ") {
                            anyhow::bail!("guest-agent delete failed: {err}");
                        }
                    }
                }
            }
        }
        self.fallback.delete_files(distro, wsl_dir, rel_paths).await
    }
}

/// Manages incremental Windows -> WSL2 ext4 synchronization for `TurboMountRule`s.
pub struct TurboSyncEngine {
    target: Arc<dyn WslSyncTarget>,
    sync_mutex: Mutex<()>,
    snapshots: RwLock<HashMap<String, BTreeMap<String, FileStamp>>>,
    statuses: RwLock<HashMap<String, TurboMountStatus>>,
    events: broadcast::Sender<DaemonEvent>,
}

impl TurboSyncEngine {
    #[allow(dead_code)]
    pub fn new(events: broadcast::Sender<DaemonEvent>) -> Self {
        Self::with_target(Arc::new(SystemWslSyncTarget), events)
    }

    pub fn with_target(
        target: Arc<dyn WslSyncTarget>,
        events: broadcast::Sender<DaemonEvent>,
    ) -> Self {
        Self {
            target,
            sync_mutex: Mutex::new(()),
            snapshots: RwLock::new(HashMap::new()),
            statuses: RwLock::new(HashMap::new()),
            events,
        }
    }

    pub async fn list_statuses(&self, rules: &[TurboMountRule]) -> Vec<TurboMountStatus> {
        let map = self.statuses.read().await;
        rules
            .iter()
            .map(|r| {
                map.get(&r.id)
                    .cloned()
                    .unwrap_or_else(|| default_status(r))
            })
            .collect()
    }

    pub async fn remove_rule_state(&self, id: &str) {
        self.snapshots.write().await.remove(id);
        self.statuses.write().await.remove(id);
    }

    /// Runs an incremental sync for `rule`, emitting `DaemonEvent::TurboMountUpdated` on change.
    pub async fn sync_rule(&self, distro: &str, rule: &TurboMountRule) -> TurboMountStatus {
        let _sync_guard = self.sync_mutex.lock().await;
        let prev = self
            .snapshots
            .read()
            .await
            .get(&rule.id)
            .cloned()
            .unwrap_or_default();

        let root = PathBuf::from(&rule.windows_path);
        let excludes = rule.excludes.clone();
        let scan_res = tokio::task::spawn_blocking(move || scan_diff(&root, &prev, &excludes)).await;

        let diff = match scan_res {
            Ok(Ok(d)) => d,
            Ok(Err(err)) => {
                return self
                    .record_error(rule, format!("scan failed: {err}"))
                    .await;
            }
            Err(err) => {
                return self
                    .record_error(rule, format!("scan task failed: {err}"))
                    .await;
            }
        };

        if diff.upserted.is_empty() && diff.deleted.is_empty() {
            let mut statuses = self.statuses.write().await;
            let entry = statuses
                .entry(rule.id.clone())
                .or_insert_with(|| default_status(rule));
            entry.enabled = rule.enabled;
            if entry.state == TurboMountSyncState::Idle {
                entry.state = TurboMountSyncState::Synced;
                entry.files_synced = diff.next_snapshot.len() as u64;
                entry.last_sync_epoch_ms = Some(now_epoch_ms());
            }
            return entry.clone();
        }

        self.set_syncing(rule).await;

        if !diff.deleted.is_empty() {
            if let Err(err) = self
                .target
                .delete_files(distro, &rule.wsl_path, &diff.deleted)
                .await
            {
                return self.record_error(rule, err.to_string()).await;
            }
        }

        for chunk in diff.upserted.chunks(MAX_TAR_BATCH_FILES) {
            let root_clone = PathBuf::from(&rule.windows_path);
            let chunk_vec = chunk.to_vec();
            let tar_res =
                tokio::task::spawn_blocking(move || build_ustar_archive(&root_clone, &chunk_vec))
                    .await;
            let tar_bytes = match tar_res {
                Ok(Ok(bytes)) => bytes,
                Ok(Err(err)) => return self.record_error(rule, err.to_string()).await,
                Err(err) => return self.record_error(rule, err.to_string()).await,
            };
            if let Err(err) = self
                .target
                .extract_tar(distro, &rule.wsl_path, &tar_bytes)
                .await
            {
                return self.record_error(rule, err.to_string()).await;
            }
        }

        let total_files = diff.next_snapshot.len() as u64;
        self.snapshots
            .write()
            .await
            .insert(rule.id.clone(), diff.next_snapshot);

        let status = TurboMountStatus {
            id: rule.id.clone(),
            windows_path: rule.windows_path.clone(),
            wsl_path: rule.wsl_path.clone(),
            excludes: rule.excludes.clone(),
            enabled: rule.enabled,
            state: TurboMountSyncState::Synced,
            files_synced: total_files,
            last_sync_epoch_ms: Some(now_epoch_ms()),
            last_error: None,
        };
        self.statuses
            .write()
            .await
            .insert(rule.id.clone(), status.clone());
        let _ = self
            .events
            .send(DaemonEvent::TurboMountUpdated(status.clone()));
        debug!(id = %rule.id, files = total_files, "Turbo Mount synced");
        status
    }

    async fn set_syncing(&self, rule: &TurboMountRule) {
        let mut statuses = self.statuses.write().await;
        let entry = statuses
            .entry(rule.id.clone())
            .or_insert_with(|| default_status(rule));
        entry.state = TurboMountSyncState::Syncing;
        entry.last_error = None;
        let _ = self
            .events
            .send(DaemonEvent::TurboMountUpdated(entry.clone()));
    }

    async fn record_error(&self, rule: &TurboMountRule, message: String) -> TurboMountStatus {
        warn!(id = %rule.id, error = %message, "Turbo Mount sync failed");
        let status = TurboMountStatus {
            id: rule.id.clone(),
            windows_path: rule.windows_path.clone(),
            wsl_path: rule.wsl_path.clone(),
            excludes: rule.excludes.clone(),
            enabled: rule.enabled,
            state: TurboMountSyncState::Error,
            files_synced: self
                .statuses
                .read()
                .await
                .get(&rule.id)
                .map(|s| s.files_synced)
                .unwrap_or(0),
            last_sync_epoch_ms: Some(now_epoch_ms()),
            last_error: Some(message),
        };
        self.statuses
            .write()
            .await
            .insert(rule.id.clone(), status.clone());
        let _ = self
            .events
            .send(DaemonEvent::TurboMountUpdated(status.clone()));
        status
    }
}

/// Watches enabled Turbo Mount rules using OS directory notifications (with 100ms debounce)
/// and a 30-second fallback reconcile while the WSL2 engine is `Running`.
pub async fn run_turbo_sync_watcher<E: wsl_bridge::WslExecutor + 'static>(ctx: Arc<AppContext<E>>) {
    let mut reconcile = tokio::time::interval(RECONCILE_INTERVAL);
    let changed = Arc::new(Notify::new());
    let mut watchers: HashMap<String, (String, DirWatcher)> = HashMap::new();

    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => {
                watchers.clear();
                info!("Shutting down Turbo Mount watcher");
                break;
            }
            _ = changed.notified() => {
                // Coalesce burst filesystem events over 100ms before scanning.
                loop {
                    tokio::select! {
                        _ = ctx.shutdown.cancelled() => {
                            watchers.clear();
                            return;
                        }
                        _ = changed.notified() => continue,
                        _ = tokio::time::sleep(WATCH_DEBOUNCE) => break,
                    }
                }
                sync_enabled_rules(&ctx, &mut watchers, &changed).await;
            }
            _ = reconcile.tick() => {
                sync_enabled_rules(&ctx, &mut watchers, &changed).await;
            }
        }
    }
}

async fn sync_enabled_rules<E: wsl_bridge::WslExecutor>(
    ctx: &AppContext<E>,
    watchers: &mut HashMap<String, (String, DirWatcher)>,
    changed: &Arc<Notify>,
) {
    if *ctx.state.read().await != EngineState::Running {
        if !watchers.is_empty() {
            watchers.clear();
        }
        return;
    }

    let (distro, rules) = {
        let cfg = ctx.config.read().await;
        (cfg.distro_name.clone(), cfg.turbo_mounts.clone())
    };
    let enabled_rules: Vec<TurboMountRule> = rules.into_iter().filter(|r| r.enabled).collect();

    watchers.retain(|id, (path, _)| {
        enabled_rules
            .iter()
            .any(|r| r.id == *id && r.windows_path == *path)
    });

    for rule in enabled_rules {
        if !watchers.contains_key(&rule.id) {
            if let Some(w) = DirWatcher::spawn(Path::new(&rule.windows_path), Arc::clone(changed)) {
                watchers.insert(rule.id.clone(), (rule.windows_path.clone(), w));
            }
        }
        ctx.turbo_sync.sync_rule(&distro, &rule).await;
    }
}

pub fn make_turbo_rule_id(windows_path: &str) -> String {
    let base = Path::new(windows_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let slug: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let short_hash = windows_path
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    format!("{}-{:04x}", slug.trim_matches('-'), short_hash & 0xffff)
}

fn default_status(rule: &TurboMountRule) -> TurboMountStatus {
    TurboMountStatus {
        id: rule.id.clone(),
        windows_path: rule.windows_path.clone(),
        wsl_path: rule.wsl_path.clone(),
        excludes: rule.excludes.clone(),
        enabled: rule.enabled,
        state: TurboMountSyncState::Idle,
        files_synced: 0,
        last_sync_epoch_ms: None,
        last_error: None,
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;

    struct MockSyncTarget {
        extracted: Mutex<Vec<(String, usize)>>,
        deleted: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WslSyncTarget for MockSyncTarget {
        async fn extract_tar(
            &self,
            _distro: &str,
            wsl_dir: &str,
            tar_bytes: &[u8],
        ) -> anyhow::Result<()> {
            self.extracted
                .lock()
                .await
                .push((wsl_dir.to_string(), tar_bytes.len()));
            Ok(())
        }

        async fn delete_files(
            &self,
            _distro: &str,
            _wsl_dir: &str,
            rel_paths: &[String],
        ) -> anyhow::Result<()> {
            self.deleted.lock().await.extend(rel_paths.iter().cloned());
            Ok(())
        }
    }

    #[tokio::test]
    async fn sync_rule_extracts_new_files_and_deletes_removed_files() {
        let dir = std::env::temp_dir().join(format!("nanodock-engine-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.rs"), "fn main() {}").unwrap();

        let target = Arc::new(MockSyncTarget {
            extracted: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
        });
        let (tx, _) = broadcast::channel(16);
        let engine = TurboSyncEngine::with_target(target.clone(), tx);

        let rule = TurboMountRule {
            id: "proj".to_string(),
            windows_path: dir.to_string_lossy().to_string(),
            wsl_path: "/var/lib/nanodock/turbo/proj".to_string(),
            excludes: vec![],
            enabled: true,
        };

        let st1 = engine.sync_rule("nanodock", &rule).await;
        assert_eq!(st1.state, TurboMountSyncState::Synced);
        assert_eq!(st1.files_synced, 1);
        assert_eq!(target.extracted.lock().await.len(), 1);

        std::fs::remove_file(dir.join("main.rs")).unwrap();
        let st2 = engine.sync_rule("nanodock", &rule).await;
        assert_eq!(st2.files_synced, 0);
        assert_eq!(*target.deleted.lock().await, vec!["main.rs".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
