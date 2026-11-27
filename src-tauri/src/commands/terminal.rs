use std::collections::HashMap;
use std::sync::Arc;

use shared_types::{ControlRequest, ControlResponse, ExecClientFrame, ExecServerFrame, LogStreamFrame};
use tauri::ipc::Channel;
use tauri::State;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex};

use crate::daemon_client::open_control_pipe_with_retry;

#[derive(Default)]
pub struct ExecSessionRegistry {
    sessions: Arc<Mutex<HashMap<String, mpsc::Sender<ExecClientFrame>>>>,
}

#[derive(Default)]
pub struct StreamCancelRegistry {
    tokens: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
}

#[tauri::command]
pub async fn start_exec_session(
    registry: State<'_, ExecSessionRegistry>,
    container_id: String,
    shell: Option<String>,
    cols: u16,
    rows: u16,
    on_event: Channel<ExecServerFrame>,
) -> Result<String, String> {
    let client = open_control_pipe_with_retry()
        .await
        .map_err(|e| e.to_string())?;
    let (reader, mut writer) = tokio::io::split(client);

    let mut start_req = serde_json::to_string(&ControlRequest::StartExecSession {
        id: container_id,
        shell,
        cols,
        rows,
    })
    .map_err(|e| e.to_string())?;
    start_req.push('\n');
    writer
        .write_all(start_req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    let mut lines = BufReader::new(reader).lines();
    let first_line = lines
        .next_line()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "daemon closed exec session".to_string())?;
    let first_frame: ExecServerFrame =
        serde_json::from_str(&first_line).map_err(|e| e.to_string())?;

    let session_id = match &first_frame {
        ExecServerFrame::Started { exec_id } => exec_id.clone(),
        ExecServerFrame::Error { message } => return Err(message.clone()),
        _ => return Err("unexpected initial exec frame".to_string()),
    };

    let (tx, mut rx) = mpsc::channel::<ExecClientFrame>(64);
    registry
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), tx);

    let _ = on_event.send(first_frame);
    let sessions_map = Arc::clone(&registry.sessions);
    let sid = session_id.clone();

    tauri::async_runtime::spawn(async move {
        let write_task = tokio::spawn(async move {
            while let Some(frame) = rx.recv().await {
                let is_close = matches!(frame, ExecClientFrame::Close);
                if let Ok(mut encoded) = serde_json::to_string(&frame) {
                    encoded.push('\n');
                    if writer.write_all(encoded.as_bytes()).await.is_err() {
                        break;
                    }
                }
                if is_close {
                    break;
                }
            }
        });

        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(frame) = serde_json::from_str::<ExecServerFrame>(&line) {
                let is_exit = matches!(frame, ExecServerFrame::Exit { .. });
                let _ = on_event.send(frame);
                if is_exit {
                    break;
                }
            }
        }

        write_task.abort();
        sessions_map.lock().await.remove(&sid);
    });

    Ok(session_id)
}

#[tauri::command]
pub async fn send_exec_input(
    registry: State<'_, ExecSessionRegistry>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let tx = registry
        .sessions
        .lock()
        .await
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "exec session not found".to_string())?;
    tx.send(ExecClientFrame::Input { data })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resize_exec_session(
    registry: State<'_, ExecSessionRegistry>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let tx = registry.sessions.lock().await.get(&session_id).cloned();
    if let Some(tx) = tx {
        let _ = tx.send(ExecClientFrame::Resize { cols, rows }).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn close_exec_session(
    registry: State<'_, ExecSessionRegistry>,
    session_id: String,
) -> Result<(), String> {
    if let Some(tx) = registry.sessions.lock().await.remove(&session_id) {
        let _ = tx.send(ExecClientFrame::Close).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_container_logs_stream(
    cancels: State<'_, StreamCancelRegistry>,
    stream_key: String,
    container_id: String,
    tail: usize,
    on_frame: Channel<LogStreamFrame>,
) -> Result<(), String> {
    if let Some(prev) = cancels.tokens.lock().await.remove(&stream_key) {
        let _ = prev.send(());
    }
    let client = open_control_pipe_with_retry()
        .await
        .map_err(|e| e.to_string())?;
    let (reader, mut writer) = tokio::io::split(client);
    let mut req = serde_json::to_string(&ControlRequest::StreamContainerLogs {
        id: container_id,
        tail,
    })
    .map_err(|e| e.to_string())?;
    req.push('\n');
    writer
        .write_all(req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();
    cancels.tokens.lock().await.insert(stream_key.clone(), cancel_tx);
    let tokens_map = Arc::clone(&cancels.tokens);

    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            tokio::select! {
                _ = &mut cancel_rx => break,
                res = lines.next_line() => {
                    match res {
                        Ok(Some(line)) => {
                            if let Ok(frame) = serde_json::from_str::<LogStreamFrame>(&line) {
                                let ended = matches!(frame, LogStreamFrame::Ended);
                                if on_frame.send(frame).is_err() || ended {
                                    break;
                                }
                            }
                        }
                        Ok(None) | Err(_) => break,
                    }
                }
            }
        }
        tokens_map.lock().await.remove(&stream_key);
    });

    Ok(())
}

#[tauri::command]
pub async fn start_container_stats_stream(
    cancels: State<'_, StreamCancelRegistry>,
    stream_key: String,
    container_id: String,
    on_stats: Channel<ControlResponse>,
) -> Result<(), String> {
    if let Some(prev) = cancels.tokens.lock().await.remove(&stream_key) {
        let _ = prev.send(());
    }
    let client = open_control_pipe_with_retry()
        .await
        .map_err(|e| e.to_string())?;
    let (reader, mut writer) = tokio::io::split(client);
    let mut req = serde_json::to_string(&ControlRequest::StreamContainerStats { id: container_id })
        .map_err(|e| e.to_string())?;
    req.push('\n');
    writer
        .write_all(req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();
    cancels.tokens.lock().await.insert(stream_key.clone(), cancel_tx);
    let tokens_map = Arc::clone(&cancels.tokens);

    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            tokio::select! {
                _ = &mut cancel_rx => break,
                res = lines.next_line() => {
                    match res {
                        Ok(Some(line)) => {
                            if let Ok(resp) = serde_json::from_str::<ControlResponse>(&line) {
                                if on_stats.send(resp).is_err() {
                                    break;
                                }
                            }
                        }
                        Ok(None) | Err(_) => break,
                    }
                }
            }
        }
        tokens_map.lock().await.remove(&stream_key);
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_container_stream(
    cancels: State<'_, StreamCancelRegistry>,
    stream_key: String,
) -> Result<(), String> {
    if let Some(tx) = cancels.tokens.lock().await.remove(&stream_key) {
        let _ = tx.send(());
    }
    Ok(())
}
