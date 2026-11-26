use shared_types::{
    ContainerActionKind, ControlRequest, ControlResponse, EngineConfig, RunContainerSpec,
};

use crate::daemon_client::send_control_request;

#[tauri::command]
pub async fn get_engine_status() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::GetStatus)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_engine() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::StartEngine)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_engine() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::StopEngine)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trim_disk() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::TrimDisk)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::GetConfig)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_config(config: EngineConfig) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::UpdateConfig(config))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_containers() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ListContainers)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn container_action(
    id: String,
    action: ContainerActionKind,
) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ContainerAction { id, action })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_container_logs(
    id: String,
    tail: Option<usize>,
) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::GetContainerLogs { id, tail })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ListImages)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pull_image(reference: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::PullImage { reference })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_image(id: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::RemoveImage { id })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prune_images() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::PruneImages)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_volumes() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ListVolumes)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_volume(name: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::RemoveVolume { name })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prune_volumes() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::PruneVolumes)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_networks() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ListNetworks)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_network(
    name: String,
    driver: Option<String>,
    internal: bool,
) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::CreateNetwork {
        name,
        driver,
        internal,
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_network(id: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::RemoveNetwork { id })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prune_networks() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::PruneNetworks)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inspect_container(id: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::InspectContainer { id })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_container_stats(id: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::GetContainerStats { id })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_container(spec: RunContainerSpec) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::RunContainer(spec))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_volume(
    name: String,
    driver: Option<String>,
) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::CreateVolume { name, driver })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_container_shell(id: String) -> Result<(), String> {
    let clean: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    if clean.is_empty() {
        return Err("Invalid container ID".to_string());
    }

    let script = format!(
        "docker exec -it {clean} sh -c 'command -v bash >/dev/null && exec bash || exec sh'"
    );
    // Prefer Windows Terminal if installed, otherwise fall back to PowerShell.
    let wt_res = std::process::Command::new("wt.exe")
        .args(["powershell", "-NoExit", "-Command", &script])
        .spawn();
    if wt_res.is_ok() {
        return Ok(());
    }

    std::process::Command::new("powershell.exe")
        .args(["-NoExit", "-Command", &script])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_localhost_port(port: u16) -> Result<(), String> {
    let url = format!("http://localhost:{port}");
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("Only supported on Windows".to_string())
    }
}
