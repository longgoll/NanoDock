use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use shared_types::ControlRequest;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::daemon_client::send_control_request;
use crate::window_manager::open_or_focus_dashboard;

/// Shared flag set to `true` only when the user explicitly clicks "Quit" in the Tray menu.
pub struct AllowExit(pub Arc<AtomicBool>);

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
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
    builder
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

    Ok(())
}
