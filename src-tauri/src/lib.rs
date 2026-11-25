mod commands;
mod daemon_client;
mod tray;
mod window_manager;

use std::path::Path;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent};

use crate::tray::{setup_tray, AllowExit};

struct DaemonChild(Mutex<Option<Child>>);

fn spawn_daemon_if_needed() -> Option<Child> {
    // Skip spawning if core-daemon is already listening on the control pipe.
    if std::fs::metadata(r"\\.\pipe\nanodock_ctl").is_ok() {
        return None;
    }

    let exe = std::env::current_exe().ok()?;
    let daemon_path = exe.parent()?.join("core-daemon.exe");
    if !Path::new(&daemon_path).exists() {
        return None;
    }

    let mut cmd = Command::new(daemon_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().ok()
}

pub fn run() {
    let allow_exit = Arc::new(AtomicBool::new(false));
    let daemon_child = Arc::new(DaemonChild(Mutex::new(None)));
    let daemon_for_setup = Arc::clone(&daemon_child);

    let app = tauri::Builder::default()
        .manage(AllowExit(Arc::clone(&allow_exit)))
        .manage(daemon_child)
        .setup(move |app| {
            if let Some(child) = spawn_daemon_if_needed() {
                if let Ok(mut guard) = daemon_for_setup.0.lock() {
                    *guard = Some(child);
                }
            }
            setup_tray(app.handle())?;
            let _ = window_manager::open_or_focus_dashboard(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::engine::get_engine_status,
            commands::engine::start_engine,
            commands::engine::stop_engine,
            commands::engine::trim_disk,
            commands::engine::get_config,
            commands::engine::update_config,
            commands::engine::list_containers,
            commands::engine::container_action,
            commands::engine::get_container_logs,
            commands::engine::list_images,
            commands::engine::pull_image,
            commands::engine::remove_image,
            commands::engine::prune_images,
            commands::engine::list_volumes,
            commands::engine::create_volume,
            commands::engine::remove_volume,
            commands::engine::prune_volumes,
            commands::engine::inspect_container,
            commands::engine::get_container_stats,
            commands::engine::run_container,
            commands::engine::open_container_shell,
            commands::engine::open_localhost_port,
        ])
        .build(tauri::generate_context!())
        .expect("error while building NanoDock Tauri application");

    app.run(move |app_handle, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            let should_exit = app_handle
                .try_state::<AllowExit>()
                .is_some_and(|f| f.0.load(Ordering::SeqCst));
            if !should_exit {
                // Keep the System Tray alive at 0 MB WebView2 RAM when dashboard closes.
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            if let Some(state) = app_handle.try_state::<Arc<DaemonChild>>() {
                if let Ok(mut guard) = state.0.lock() {
                    if let Some(mut child) = guard.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            }
        }
        _ => {}
    });
}
