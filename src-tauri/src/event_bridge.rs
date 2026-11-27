use std::time::Duration;

use shared_types::{ControlRequest, DaemonEvent};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;

use crate::daemon_client::open_control_pipe_with_retry;

pub const DAEMON_EVENT_NAME: &str = "nanodock://daemon-event";

/// Subscribes to `core-daemon`'s `SubscribeEvents` stream, forwards `DaemonEvent`s
/// to the Svelte WebView (`nanodock://daemon-event`), and notifies the System Tray.
pub fn spawn_event_bridge(app: AppHandle, tray_notify: watch::Sender<u64>) {
    tauri::async_runtime::spawn(async move {
        let mut seq: u64 = 0;
        loop {
            match run_event_stream(&app, &tray_notify, &mut seq).await {
                Ok(()) => tokio::time::sleep(Duration::from_secs(1)).await,
                Err(_) => tokio::time::sleep(Duration::from_secs(2)).await,
            }
        }
    });
}

async fn run_event_stream(
    app: &AppHandle,
    tray_notify: &watch::Sender<u64>,
    seq: &mut u64,
) -> anyhow::Result<()> {
    let client = open_control_pipe_with_retry().await?;
    let (reader, mut writer) = tokio::io::split(client);

    let mut req = serde_json::to_string(&ControlRequest::SubscribeEvents)?;
    req.push('\n');
    writer.write_all(req.as_bytes()).await?;

    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        if let Ok(event) = serde_json::from_str::<DaemonEvent>(&line) {
            let _ = app.emit(DAEMON_EVENT_NAME, &event);
            *seq = seq.wrapping_add(1);
            let _ = tray_notify.send(*seq);
        }
    }

    Ok(())
}
