use anyhow::Context as _;
use serde_json::Value;
use shared_types::{ContainerActionKind, ContainerSummary};
use wsl_bridge::WslExecutor;

use crate::transport::TransportManager;

use super::http::{
    decode_docker_logs, docker_http_request, fetch_all_containers_raw, sanitize_path_segment,
};

/// Queries the number of currently running containers over the existing multiplexed transport.
pub async fn count_running_containers<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<usize> {
    let (_, body) = docker_http_request(
        transport,
        "GET",
        "/containers/json?filters=%7B%22status%22%3A%5B%22running%22%5D%7D",
    )
    .await?;
    let containers: Vec<Value> =
        serde_json::from_slice(&body).context("failed to parse /containers/json")?;
    Ok(containers.len())
}

/// Lists all containers (running and stopped).
pub async fn list_containers<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<Vec<ContainerSummary>> {
    let items = fetch_all_containers_raw(transport, true).await?;

    Ok(items
        .into_iter()
        .map(|item| {
            let id = item
                .get("Id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(12)
                .collect();
            let names = item
                .get("Names")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(|s| s.trim_start_matches('/').to_string())
                        .collect()
                })
                .unwrap_or_default();
            let image = item
                .get("Image")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let state = item
                .get("State")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let status = item
                .get("Status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let created = item.get("Created").and_then(Value::as_i64).unwrap_or(0);
            let ports = format_ports(item.get("Ports").and_then(Value::as_array));
            let compose_project = item
                .get("Labels")
                .and_then(Value::as_object)
                .and_then(|labels| labels.get("com.docker.compose.project"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            ContainerSummary {
                id,
                names,
                image,
                state,
                status,
                created,
                ports,
                compose_project,
            }
        })
        .collect())
}

/// Executes a lifecycle action (`start`, `stop`, `restart`, `remove`) on a container.
pub async fn perform_container_action<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    action: ContainerActionKind,
) -> anyhow::Result<()> {
    let clean_id = sanitize_path_segment(id)?;
    let (method, path) = match action {
        ContainerActionKind::Start => ("POST", format!("/containers/{clean_id}/start")),
        ContainerActionKind::Stop => ("POST", format!("/containers/{clean_id}/stop")),
        ContainerActionKind::Restart => ("POST", format!("/containers/{clean_id}/restart")),
        ContainerActionKind::Remove => ("DELETE", format!("/containers/{clean_id}?force=1")),
    };
    docker_http_request(transport, method, &path).await?;
    Ok(())
}

/// Fetches recent container stdout/stderr logs.
pub async fn get_container_logs<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    tail: usize,
) -> anyhow::Result<String> {
    let clean_id = sanitize_path_segment(id)?;
    let path = format!("/containers/{clean_id}/logs?stdout=1&stderr=1&tail={tail}");
    let (_, body) = docker_http_request(transport, "GET", &path).await?;
    Ok(decode_docker_logs(&body))
}

/// Extracts a human-readable container name from a `/containers/json` item.
pub(super) fn container_display_name(item: &Value) -> String {
    item.get("Names")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(Value::as_str)
        .map(|s| s.trim_start_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            item.get("Id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(12)
                .collect()
        })
}

fn format_ports(ports: Option<&Vec<Value>>) -> Vec<String> {
    let Some(arr) = ports else {
        return Vec::new();
    };
    let mut formatted = Vec::new();
    for p in arr {
        let priv_port = p.get("PrivatePort").and_then(Value::as_u64);
        let pub_port = p.get("PublicPort").and_then(Value::as_u64);
        let kind = p.get("Type").and_then(Value::as_str).unwrap_or("tcp");
        if let Some(priv_p) = priv_port {
            let entry = match pub_port {
                Some(pub_p) => format!("{pub_p}:{priv_p}/{kind}"),
                None => format!("{priv_p}/{kind}"),
            };
            if !formatted.contains(&entry) {
                formatted.push(entry);
            }
        }
    }
    formatted
}
