use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const DASHBOARD_LABEL: &str = "main";

/// Opens the Dashboard window, or focuses it if already open.
///
/// When the user closes the window, `window.destroy()` is called to free 100% of
/// `msedgewebview2.exe` RAM while keeping the System Tray alive.
pub fn open_or_focus_dashboard(app: &AppHandle) -> tauri::Result<()> {
    if let Some(existing) = app.get_webview_window(DASHBOARD_LABEL) {
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, DASHBOARD_LABEL, WebviewUrl::App("index.html".into()))
        .title("NanoDock")
        .inner_size(980.0, 660.0)
        .min_inner_size(760.0, 520.0)
        .build()?;

    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Some(win) = app_handle.get_webview_window(DASHBOARD_LABEL) {
                let _ = win.destroy();
            }
        }
    });

    Ok(())
}
