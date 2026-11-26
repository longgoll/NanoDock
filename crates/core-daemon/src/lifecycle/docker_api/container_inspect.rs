use anyhow::Context as _;
use serde_json::Value;
use shared_types::{ContainerInspect, ContainerMount, ContainerStats, RunContainerSpec};
use wsl_bridge::WslExecutor;

use crate::transport::TransportManager;

use super::http::{
    docker_http_json_request, docker_http_request, percent_encode_query, sanitize_path_segment,
};

/// Fetches detailed inspection info for a container (`GET /containers/{id}/json`).
pub async fn inspect_container<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
) -> anyhow::Result<ContainerInspect> {
    let clean_id = sanitize_path_segment(id)?;
    let path = format!("/containers/{clean_id}/json");
    let (_, body) = docker_http_request(transport, "GET", &path).await?;
    let val: Value = serde_json::from_slice(&body).context("failed to parse container inspect")?;

    let short_id: String = val
        .get("Id")
        .and_then(Value::as_str)
        .unwrap_or(clean_id)
        .chars()
        .take(12)
        .collect();
    let name = val
        .get("Name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_string();
    let image = val
        .pointer("/Config/Image")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let state = val
        .pointer("/State/Status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let running = val
        .pointer("/State/Running")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = if running {
        format!(
            "running (pid {})",
            val.pointer("/State/Pid").and_then(Value::as_u64).unwrap_or(0)
        )
    } else {
        format!(
            "exited ({})",
            val.pointer("/State/ExitCode")
                .and_then(Value::as_i64)
                .unwrap_or(0)
        )
    };
    let created = val
        .get("Created")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let restart_policy = val
        .pointer("/HostConfig/RestartPolicy/Name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("no")
        .to_string();

    let str_array = |ptr: &str| -> Vec<String> {
        val.pointer(ptr)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };

    let mounts = val
        .get("Mounts")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|m| ContainerMount {
                    mount_type: m
                        .get("Type")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    source: m
                        .get("Source")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    destination: m
                        .get("Destination")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    mode: m
                        .get("Mode")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let mut networks = Vec::new();
    let mut ip_address = String::new();
    if let Some(net_map) = val.pointer("/NetworkSettings/Networks").and_then(Value::as_object) {
        for (net_name, net_info) in net_map {
            networks.push(net_name.clone());
            if ip_address.is_empty() {
                if let Some(ip) = net_info.get("IPAddress").and_then(Value::as_str) {
                    if !ip.is_empty() {
                        ip_address = ip.to_string();
                    }
                }
            }
        }
    }

    let mut ports = Vec::new();
    if let Some(port_map) = val.pointer("/NetworkSettings/Ports").and_then(Value::as_object) {
        for (container_port, bindings) in port_map {
            if let Some(arr) = bindings.as_array() {
                for b in arr {
                    if let Some(hp) = b.get("HostPort").and_then(Value::as_str) {
                        ports.push(format!("{hp}:{container_port}"));
                    }
                }
            } else {
                ports.push(container_port.clone());
            }
        }
    }

    Ok(ContainerInspect {
        id: short_id,
        name,
        image,
        state,
        status,
        created,
        restart_policy,
        cmd: str_array("/Config/Cmd"),
        entrypoint: str_array("/Config/Entrypoint"),
        env: str_array("/Config/Env"),
        mounts,
        networks,
        ip_address,
        ports,
    })
}

/// Fetches a single CPU & memory snapshot for a running container.
pub async fn get_container_stats<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
) -> anyhow::Result<ContainerStats> {
    let clean_id = sanitize_path_segment(id)?;
    let path = format!("/containers/{clean_id}/stats?stream=false");
    let (_, body) = docker_http_request(transport, "GET", &path).await?;
    let val: Value = serde_json::from_slice(&body).context("failed to parse container stats")?;

    let cpu_total = val
        .pointer("/cpu_stats/cpu_usage/total_usage")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let precpu_total = val
        .pointer("/precpu_stats/cpu_usage/total_usage")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let sys_total = val
        .pointer("/cpu_stats/system_cpu_usage")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let presys_total = val
        .pointer("/precpu_stats/system_cpu_usage")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let online_cpus = val
        .pointer("/cpu_stats/online_cpus")
        .and_then(Value::as_u64)
        .or_else(|| {
            val.pointer("/cpu_stats/cpu_usage/percpu_usage")
                .and_then(Value::as_array)
                .map(|a| a.len() as u64)
        })
        .unwrap_or(1)
        .max(1);

    let cpu_delta = cpu_total.saturating_sub(precpu_total) as f64;
    let sys_delta = sys_total.saturating_sub(presys_total) as f64;
    let cpu_percent = if sys_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / sys_delta) * (online_cpus as f64) * 100.0
    } else {
        0.0
    };

    let mem_raw = val
        .pointer("/memory_stats/usage")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mem_cache = val
        .pointer("/memory_stats/stats/inactive_file")
        .and_then(Value::as_u64)
        .or_else(|| val.pointer("/memory_stats/stats/cache").and_then(Value::as_u64))
        .unwrap_or(0);
    let mem_usage_bytes = mem_raw.saturating_sub(mem_cache);
    let mem_limit_bytes = val
        .pointer("/memory_stats/limit")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    Ok(ContainerStats {
        id: clean_id.to_string(),
        cpu_percent,
        mem_usage_bytes,
        mem_limit_bytes,
    })
}

/// Creates and starts a container from an image reference and basic port/env bindings.
pub async fn run_container<E: WslExecutor>(
    transport: &TransportManager<E>,
    spec: &RunContainerSpec,
) -> anyhow::Result<String> {
    let image = spec.image.trim();
    if image.is_empty() {
        anyhow::bail!("image reference cannot be empty");
    }

    let mut exposed_ports = serde_json::Map::new();
    let mut port_bindings = serde_json::Map::new();

    for mapping in &spec.ports {
        let trimmed = mapping.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (host_port, container_spec) = match trimmed.split_once(':') {
            Some((h, c)) => (h.trim(), c.trim()),
            None => anyhow::bail!("invalid port mapping '{trimmed}', expected host:container"),
        };
        let container_key = if container_spec.contains('/') {
            container_spec.to_string()
        } else {
            format!("{container_spec}/tcp")
        };
        exposed_ports.insert(container_key.clone(), serde_json::json!({}));
        port_bindings.insert(
            container_key,
            serde_json::json!([{ "HostPort": host_port }]),
        );
    }

    let body = serde_json::json!({
        "Image": image,
        "Env": spec.env,
        "ExposedPorts": exposed_ports,
        "HostConfig": {
            "PortBindings": port_bindings
        }
    });

    let path = match spec.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => format!("/containers/create?name={}", percent_encode_query(name)),
        None => "/containers/create".to_string(),
    };

    let (_, resp_body) =
        docker_http_json_request(transport, "POST", &path, Some(&body)).await?;
    let created: Value =
        serde_json::from_slice(&resp_body).context("failed to parse /containers/create")?;
    let full_id = created
        .get("Id")
        .and_then(Value::as_str)
        .context("missing Id in /containers/create response")?;

    let start_path = format!("/containers/{full_id}/start");
    docker_http_request(transport, "POST", &start_path).await?;

    Ok(full_id.chars().take(12).collect())
}
