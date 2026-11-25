use shared_types::{ContainerActionKind, ControlRequest, ControlResponse, EngineConfig};

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
