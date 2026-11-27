use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use shared_types::{ControlRequest, ControlResponse, EngineState};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tokio::sync::watch;

use crate::daemon_client::send_control_request;
use crate::window_manager::open_or_focus_dashboard;

/// Shared flag set to `true` only when the user explicitly clicks "Quit" in the Tray menu.
pub struct AllowExit(pub Arc<AtomicBool>);

pub fn setup_tray(app: &AppHandle, notify_rx: watch::Receiver<u64>) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
    let start_item = MenuItem::with_id(app, "start", "Start Engine", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop", "Stop Engine (0 MB RAM)", true, None::<&str>)?;
    let trim_item = MenuItem::with_id(app, "trim", "Prune & Trim Disk (.vhdx)", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit NanoDock", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&open_item, &start_item, &stop_item, &trim_item, &quit_item],
    )?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("NanoDock — Lightweight WSL2 Docker Engine");
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    let tray = builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                let _ = open_or_focus_dashboard(app);
            }
            "start" => {
                tauri::async_runtime::spawn(async {
                    let _ = send_control_request(ControlRequest::StartEngine).await;
                });
            }
            "stop" => {
                tauri::async_runtime::spawn(async {
                    let _ = send_control_request(ControlRequest::StopEngine).await;
                });
            }
            "trim" => {
                tauri::async_runtime::spawn(async {
                    let _ = send_control_request(ControlRequest::TrimDisk).await;
                });
            }
            "quit" => {
                if let Some(flag) = app.try_state::<AllowExit>() {
                    flag.0.store(true, Ordering::SeqCst);
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = open_or_focus_dashboard(tray.app_handle());
            }
        })
        .build(app)?;

    spawn_tray_status_sync(tray, start_item, stop_item, notify_rx);
    Ok(())
}

fn spawn_tray_status_sync(
    tray: TrayIcon,
    start_item: MenuItem<tauri::Wry>,
    stop_item: MenuItem<tauri::Wry>,
    mut notify_rx: watch::Receiver<u64>,
) {
    tauri::async_runtime::spawn(async move {
        let mut fallback = tokio::time::interval(Duration::from_secs(60));
        loop {
            match send_control_request(ControlRequest::GetStatus).await {
                Ok(ControlResponse::Status {
                    state,
                    running_containers,
                    ..
                }) => {
                    let tooltip = match state {
                        EngineState::Running => {
                            format!("NanoDock — Running ({running_containers} active)")
                        }
                        EngineState::Stopped => "NanoDock — Sleeping (0 MB RAM)".to_string(),
                        EngineState::Starting => "NanoDock — Waking up...".to_string(),
                        EngineState::Stopping => "NanoDock — Hibernating...".to_string(),
                        EngineState::Error => "NanoDock — Engine Error".to_string(),
                    };
                    let _ = tray.set_tooltip(Some(&tooltip));
                    let _ = start_item
                        .set_enabled(matches!(state, EngineState::Stopped | EngineState::Error));
                    let _ = stop_item.set_enabled(matches!(state, EngineState::Running));
                }
                Err(_) => {
                    let _ = tray.set_tooltip(Some("NanoDock — Daemon offline"));
                }
                _ => {}
            }

            tokio::select! {
                _ = fallback.tick() => {}
                res = notify_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                }
            }
        }
    });
}
