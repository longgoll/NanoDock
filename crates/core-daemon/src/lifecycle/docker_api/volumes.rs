use anyhow::Context as _;
use serde_json::Value;
use shared_types::VolumeSummary;
use wsl_bridge::WslExecutor;

use crate::transport::TransportManager;

use super::containers::container_display_name;
use super::http::{
    docker_http_json_request, docker_http_request, fetch_all_containers_raw, sanitize_path_segment,
};

/// Lists all Docker volumes, including which containers currently mount each volume.
pub async fn list_volumes<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<Vec<VolumeSummary>> {
    let (volumes_res, containers_res) = tokio::join!(
        docker_http_request(transport, "GET", "/volumes"),
        fetch_all_containers_raw(transport, false)
    );

    let (_, body) = volumes_res?;
    let val: Value = serde_json::from_slice(&body).context("failed to parse /volumes")?;
    let items = val
        .get("Volumes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut usage: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    if let Ok(c_items) = containers_res {
        for c in &c_items {
            let cname = container_display_name(c);
            if let Some(mounts) = c.get("Mounts").and_then(Value::as_array) {
                for m in mounts {
                    if m.get("Type").and_then(Value::as_str) == Some("volume") {
                        if let Some(vol_name) = m.get("Name").and_then(Value::as_str) {
                            let entry = usage.entry(vol_name.to_string()).or_default();
                            if !entry.contains(&cname) {
                                entry.push(cname.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(items
        .into_iter()
        .map(|item| {
            let name = item
                .get("Name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let used_by = usage.remove(&name).unwrap_or_default();
            let in_use = !used_by.is_empty();

            VolumeSummary {
                name,
                driver: item
                    .get("Driver")
                    .and_then(Value::as_str)
                    .unwrap_or("local")
                    .to_string(),
                mountpoint: item
                    .get("Mountpoint")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                created_at: item
                    .get("CreatedAt")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                in_use,
                used_by,
            }
        })
        .collect())
}

/// Creates a named Docker volume.
pub async fn create_volume<E: WslExecutor>(
    transport: &TransportManager<E>,
    name: &str,
    driver: Option<&str>,
) -> anyhow::Result<()> {
    let clean_name = sanitize_path_segment(name)?;
    let body = serde_json::json!({
        "Name": clean_name,
        "Driver": driver.unwrap_or("local"),
    });
    docker_http_json_request(transport, "POST", "/volumes/create", Some(&body)).await?;
    Ok(())
}

/// Removes a volume by name.
pub async fn remove_volume<E: WslExecutor>(
    transport: &TransportManager<E>,
    name: &str,
) -> anyhow::Result<()> {
    let clean_name = sanitize_path_segment(name)?;
    let path = format!("/volumes/{clean_name}?force=1");
    docker_http_request(transport, "DELETE", &path).await?;
    Ok(())
}

/// Prunes unused volumes and returns reclaimed bytes.
pub async fn prune_volumes<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<u64> {
    let (_, body) = docker_http_request(transport, "POST", "/volumes/prune").await?;
    let val: Value = serde_json::from_slice(&body).unwrap_or_default();
    Ok(val
        .get("SpaceReclaimed")
        .and_then(Value::as_u64)
        .unwrap_or(0))
}
