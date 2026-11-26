use anyhow::Context as _;
use serde_json::Value;
use shared_types::ImageSummary;
use wsl_bridge::WslExecutor;

use crate::transport::TransportManager;

use super::containers::container_display_name;
use super::http::{
    docker_http_request, fetch_all_containers_raw, percent_encode_query, sanitize_path_segment,
};

/// Lists all local Docker images, including which containers currently reference each image.
pub async fn list_images<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<Vec<ImageSummary>> {
    let (images_res, containers_res) = tokio::join!(
        docker_http_request(transport, "GET", "/images/json"),
        fetch_all_containers_raw(transport, false)
    );

    let (_, body) = images_res?;
    let items: Vec<Value> =
        serde_json::from_slice(&body).context("failed to parse /images/json")?;

    let mut usage: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    if let Ok(c_items) = containers_res {
        for c in &c_items {
            let full_img_id = c.get("ImageID").and_then(Value::as_str).unwrap_or_default();
            let short_img_id: String = full_img_id
                .strip_prefix("sha256:")
                .unwrap_or(full_img_id)
                .chars()
                .take(12)
                .collect();
            if !short_img_id.is_empty() {
                usage
                    .entry(short_img_id)
                    .or_default()
                    .push(container_display_name(c));
            }
        }
    }

    Ok(items
        .into_iter()
        .map(|item| {
            let full_id = item.get("Id").and_then(Value::as_str).unwrap_or_default();
            let id: String = full_id
                .strip_prefix("sha256:")
                .unwrap_or(full_id)
                .chars()
                .take(12)
                .collect();
            let repo_tags = item
                .get("RepoTags")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let size = item.get("Size").and_then(Value::as_i64).unwrap_or(0);
            let created = item.get("Created").and_then(Value::as_i64).unwrap_or(0);
            let used_by = usage.remove(&id).unwrap_or_default();
            let in_use = !used_by.is_empty();

            ImageSummary {
                id,
                repo_tags,
                size,
                created,
                in_use,
                used_by,
            }
        })
        .collect())
}

/// Pulls an image reference (e.g., `nginx:alpine`) via Docker Engine API.
pub async fn pull_image<E: WslExecutor>(
    transport: &TransportManager<E>,
    reference: &str,
) -> anyhow::Result<()> {
    let trimmed = reference.trim();
    if trimmed.is_empty() || trimmed.contains([' ', '\r', '\n']) {
        anyhow::bail!("invalid image reference");
    }
    let encoded = percent_encode_query(trimmed);
    let path = format!("/images/create?fromImage={encoded}");
    docker_http_request(transport, "POST", &path).await?;
    Ok(())
}

/// Removes an image by ID or tag.
pub async fn remove_image<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
) -> anyhow::Result<()> {
    let clean_id = sanitize_path_segment(id)?;
    let path = format!("/images/{clean_id}?force=1");
    docker_http_request(transport, "DELETE", &path).await?;
    Ok(())
}

/// Prunes unused images and returns reclaimed bytes.
pub async fn prune_images<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<u64> {
    let (_, body) = docker_http_request(transport, "POST", "/images/prune").await?;
    let val: Value = serde_json::from_slice(&body).unwrap_or_default();
    Ok(val
        .get("SpaceReclaimed")
        .and_then(Value::as_u64)
        .unwrap_or(0))
}
