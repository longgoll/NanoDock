mod commands;
mod daemon_client;
mod event_bridge;
mod tray;
mod window_manager;

use std::path::Path;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent};

use crate::tray::{setup_tray, AllowExit};

struct DaemonChild(Mutex<Option<Child>>);

/// Gracefully shuts down `core-daemon.exe` (whether spawned by this UI or running externally)
/// and waits briefly for the child process to exit so installer updates do not hit locked files.
async fn shutdown_daemon_process(app_handle: &tauri::AppHandle) {
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        daemon_client::send_control_request(shared_types::ControlRequest::ShutdownDaemon),
    )
    .await;

    if let Some(state) = app_handle.try_state::<Arc<DaemonChild>>() {
        if let Ok(mut guard) = state.0.lock() {
            if let Some(mut child) = guard.take() {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) if std::time::Instant::now() < deadline => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[tauri::command]
async fn prepare_for_update(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(flag) = app_handle.try_state::<AllowExit>() {
        flag.0.store(true, Ordering::SeqCst);
    }
    shutdown_daemon_process(&app_handle).await;
    Ok(())
}

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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AllowExit(Arc::clone(&allow_exit)))
        .manage(daemon_child)
        .manage(commands::terminal::ExecSessionRegistry::default())
        .manage(commands::terminal::StreamCancelRegistry::default())
        .setup(move |app| {
            commands::system::auto_configure_cli_on_first_run();
            if let Some(child) = spawn_daemon_if_needed() {
                if let Ok(mut guard) = daemon_for_setup.0.lock() {
                    *guard = Some(child);
                }
            }
            let (tray_tx, tray_rx) = tokio::sync::watch::channel(0u64);
            setup_tray(app.handle(), tray_rx)?;
            event_bridge::spawn_event_bridge(app.handle().clone(), tray_tx);
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
            commands::engine::list_networks,
            commands::engine::create_network,
            commands::engine::remove_network,
            commands::engine::prune_networks,
            commands::engine::inspect_container,
            commands::engine::get_container_stats,
            commands::engine::run_container,
            commands::engine::open_container_shell,
            commands::engine::open_localhost_port,
            commands::engine_extra::exec_container,
            commands::engine_extra::list_container_files,
            commands::engine_extra::read_container_file,
            commands::engine_extra::inspect_image_layers,
            commands::engine_extra::export_volume,
            commands::engine_extra::import_volume,
            commands::engine_extra::get_engine_logs,
            commands::engine_extra::get_engine_metrics,
            commands::engine_extra::list_wsl_distros,
            commands::engine_extra::set_wsl_distro_integration,
            commands::engine_extra::get_build_cache,
            commands::engine_extra::prune_build_cache,
            commands::system::get_environment_status,
            commands::system::install_wsl2,
            commands::system::set_user_path_integration,
            commands::system::set_docker_cli_shim,
            commands::system::fix_docker_creds_store,
            commands::diagnostics::run_doctor_checks,
            commands::diagnostics::set_wincred_creds_store,
            commands::diagnostics::pick_archive_file,
            commands::terminal::start_exec_session,
            commands::terminal::send_exec_input,
            commands::terminal::resize_exec_session,
            commands::terminal::close_exec_session,
            commands::terminal::start_container_logs_stream,
            commands::terminal::start_container_stats_stream,
            commands::terminal::stop_container_stream,
            prepare_for_update,
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
            tauri::async_runtime::block_on(shutdown_daemon_process(app_handle));
        }
        _ => {}
    });
}
