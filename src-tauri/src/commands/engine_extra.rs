use shared_types::{ControlRequest, ControlResponse};

use crate::daemon_client::send_control_request;

#[tauri::command]
pub async fn exec_container(id: String, command: Vec<String>) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ExecContainer { id, command })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_container_files(id: String, path: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ListContainerFiles { id, path })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_container_file(id: String, path: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ReadContainerFile { id, path })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inspect_image_layers(id: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::InspectImageLayers { id })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_volume(name: String, dest_path: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ExportVolume { name, dest_path })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_volume(name: String, src_path: String) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ImportVolume { name, src_path })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_engine_logs(lines: usize) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::GetEngineLogs { lines })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_engine_metrics() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::GetEngineMetrics)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_wsl_distros() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::ListWslDistros)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_wsl_distro_integration(
    distro: String,
    enabled: bool,
) -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::SetWslDistroIntegration { distro, enabled })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_build_cache() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::GetBuildCache)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prune_build_cache() -> Result<ControlResponse, String> {
    send_control_request(ControlRequest::PruneBuildCache)
        .await
        .map_err(|e| e.to_string())
}
