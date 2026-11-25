use anyhow::Context as _;
use serde_json::Value;
use shared_types::NetworkSummary;
use wsl_bridge::WslExecutor;

use crate::lifecycle::docker_api::{
    docker_http_json_request, docker_http_request, sanitize_path_segment,
};
use crate::transport::TransportManager;

/// Lists all Docker networks with container usage counts.
pub async fn list_networks<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<Vec<NetworkSummary>> {
    let (_, body) = docker_http_request(transport, "GET", "/networks").await?;
    parse_networks_json(&body)
}

/// Creates a Docker network.
pub async fn create_network<E: WslExecutor>(
    transport: &TransportManager<E>,
    name: &str,
    driver: Option<&str>,
    internal: bool,
) -> anyhow::Result<()> {
    let clean_name = sanitize_path_segment(name)?;
    let body = serde_json::json!({
        "Name": clean_name,
        "Driver": driver.unwrap_or("bridge"),
        "Internal": internal,
    });
    docker_http_json_request(transport, "POST", "/networks/create", Some(&body)).await?;
    Ok(())
}

/// Removes a Docker network by ID or name.
pub async fn remove_network<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
) -> anyhow::Result<()> {
    let clean_id = sanitize_path_segment(id)?;
    let path = format!("/networks/{clean_id}");
    docker_http_request(transport, "DELETE", &path).await?;
    Ok(())
}

/// Prunes unused Docker networks.
pub async fn prune_networks<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<u64> {
    docker_http_request(transport, "POST", "/networks/prune").await?;
    Ok(0)
}

pub fn parse_networks_json(body: &[u8]) -> anyhow::Result<Vec<NetworkSummary>> {
    let items: Vec<Value> =
        serde_json::from_slice(body).context("failed to parse /networks")?;

    let mut out: Vec<NetworkSummary> = items
        .into_iter()
        .map(|item| {
            let id = item
                .get("Id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(12)
                .collect();
            let name = item
                .get("Name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let driver = item
                .get("Driver")
                .and_then(Value::as_str)
                .unwrap_or("bridge")
                .to_string();
            let scope = item
                .get("Scope")
                .and_then(Value::as_str)
                .unwrap_or("local")
                .to_string();
            let internal = item
                .get("Internal")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let used_by: Vec<String> = item
                .get("Containers")
                .and_then(Value::as_object)
                .map(|map| {
                    map.values()
                        .filter_map(|c| c.get("Name").and_then(Value::as_str))
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let in_use = !used_by.is_empty();

            NetworkSummary {
                id,
                name,
                driver,
                scope,
                internal,
                in_use,
                used_by,
            }
        })
        .collect();

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_networks_json_with_containers() {
        let json = br#"[
            {
                "Id": "abcdef1234567890",
                "Name": "app_net",
                "Driver": "bridge",
                "Scope": "local",
                "Internal": true,
                "Containers": {
                    "c1": { "Name": "web" }
                }
            },
            {
                "Id": "1111222233334444",
                "Name": "bridge",
                "Driver": "bridge",
                "Scope": "local",
                "Internal": false,
                "Containers": {}
            }
        ]"#;

        let nets = parse_networks_json(json).unwrap();
        assert_eq!(nets.len(), 2);
        assert_eq!(nets[0].name, "app_net");
        assert_eq!(nets[0].id, "abcdef123456");
        assert!(nets[0].internal);
        assert!(nets[0].in_use);
        assert_eq!(nets[0].used_by, vec!["web"]);

        assert_eq!(nets[1].name, "bridge");
        assert!(!nets[1].in_use);
    }
}
