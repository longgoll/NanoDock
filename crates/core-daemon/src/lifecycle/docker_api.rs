use anyhow::Context as _;
use serde_json::Value;
use shared_types::{
    ContainerActionKind, ContainerInspect, ContainerMount, ContainerStats, ContainerSummary,
    ImageSummary, RunContainerSpec, VolumeSummary,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wsl_bridge::WslExecutor;

use crate::transport::TransportManager;

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
    let (_, body) = docker_http_request(transport, "GET", "/containers/json?all=1").await?;
    let items: Vec<Value> =
        serde_json::from_slice(&body).context("failed to parse /containers/json?all=1")?;

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
        format!("running (pid {})", val.pointer("/State/Pid").and_then(Value::as_u64).unwrap_or(0))
    } else {
        format!(
            "exited ({})",
            val.pointer("/State/ExitCode").and_then(Value::as_i64).unwrap_or(0)
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
fn container_display_name(item: &Value) -> String {
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

/// Lists all local Docker images, including which containers currently reference each image.
pub async fn list_images<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<Vec<ImageSummary>> {
    let (images_res, containers_res) = tokio::join!(
        docker_http_request(transport, "GET", "/images/json"),
        docker_http_request(transport, "GET", "/containers/json?all=1")
    );

    let (_, body) = images_res?;
    let items: Vec<Value> =
        serde_json::from_slice(&body).context("failed to parse /images/json")?;

    let mut usage: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    if let Ok((_, c_body)) = containers_res {
        if let Ok(c_items) = serde_json::from_slice::<Vec<Value>>(&c_body) {
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

/// Lists all Docker volumes, including which containers currently mount each volume.
pub async fn list_volumes<E: WslExecutor>(
    transport: &TransportManager<E>,
) -> anyhow::Result<Vec<VolumeSummary>> {
    let (volumes_res, containers_res) = tokio::join!(
        docker_http_request(transport, "GET", "/volumes"),
        docker_http_request(transport, "GET", "/containers/json?all=1")
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
    if let Ok((_, c_body)) = containers_res {
        if let Ok(c_items) = serde_json::from_slice::<Vec<Value>>(&c_body) {
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

pub(super) async fn docker_http_request<E: WslExecutor>(
    transport: &TransportManager<E>,
    method: &str,
    path: &str,
) -> anyhow::Result<(u16, Vec<u8>)> {
    docker_http_json_request(transport, method, path, None).await
}

pub(super) async fn docker_http_json_request<E: WslExecutor>(
    transport: &TransportManager<E>,
    method: &str,
    path: &str,
    json_body: Option<&Value>,
) -> anyhow::Result<(u16, Vec<u8>)> {
    let mut stream = transport.open_docker_stream().await?;
    let body_bytes = match json_body {
        Some(v) => serde_json::to_vec(v)?,
        None => Vec::new(),
    };
    let req = if json_body.is_some() {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body_bytes.len()
        )
    } else {
        format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
    };
    stream.write_all(req.as_bytes()).await?;
    if !body_bytes.is_empty() {
        stream.write_all(&body_bytes).await?;
    }

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await?;

    let status = parse_http_status(&raw).context("malformed HTTP status from dockerd")?;
    let body = extract_http_body(&raw).context("malformed HTTP response from dockerd")?;

    if status >= 400 {
        let msg = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|v| v.get("message").and_then(Value::as_str).map(ToString::to_string))
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        anyhow::bail!("Docker API error ({status}): {msg}");
    }

    Ok((status, body))
}

fn parse_http_status(raw: &[u8]) -> Option<u16> {
    let line_end = raw.windows(2).position(|w| w == b"\r\n")?;
    let status_line = std::str::from_utf8(&raw[..line_end]).ok()?;
    status_line.split_whitespace().nth(1)?.parse().ok()
}

pub(super) fn sanitize_path_segment(input: &str) -> anyhow::Result<&str> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.contains(['/', '\\', ' ', '\r', '\n', '?']) {
        anyhow::bail!("invalid identifier: {input}");
    }
    Ok(trimmed)
}

fn percent_encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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

/// Decodes Docker's 8-byte multiplexed log frames, falling back to plain UTF-8 if unframed.
fn decode_docker_logs(raw: &[u8]) -> String {
    let mut pos = 0;
    let mut out = String::new();
    while pos + 8 <= raw.len() {
        let stream_type = raw[pos];
        let padding_ok = raw[pos + 1] == 0 && raw[pos + 2] == 0 && raw[pos + 3] == 0;
        if !matches!(stream_type, 0 | 1 | 2) || !padding_ok {
            return String::from_utf8_lossy(raw).into_owned();
        }
        let size = u32::from_be_bytes([raw[pos + 4], raw[pos + 5], raw[pos + 6], raw[pos + 7]])
            as usize;
        pos += 8;
        if pos + size > raw.len() {
            return String::from_utf8_lossy(raw).into_owned();
        }
        out.push_str(&String::from_utf8_lossy(&raw[pos..pos + size]));
        pos += size;
    }
    if pos == raw.len() {
        out
    } else {
        String::from_utf8_lossy(raw).into_owned()
    }
}

/// Extracts the HTTP response body, decoding `Transfer-Encoding: chunked` if present.
pub fn extract_http_body(raw: &[u8]) -> Option<Vec<u8>> {
    let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let headers = String::from_utf8_lossy(&raw[..header_end]);
    let body = &raw[header_end..];

    let is_chunked = headers
        .lines()
        .any(|l| l.to_ascii_lowercase().contains("transfer-encoding: chunked"));

    if is_chunked {
        decode_chunked_body(body)
    } else {
        Some(body.to_vec())
    }
}

fn decode_chunked_body(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let crlf = data.windows(2).position(|w| w == b"\r\n")?;
        let size_str = std::str::from_utf8(&data[..crlf]).ok()?.trim();
        let size_hex = size_str.split(';').next()?.trim();
        let size = usize::from_str_radix(size_hex, 16).ok()?;
        data = &data[crlf + 2..];
        if size == 0 {
            break;
        }
        if data.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{decode_docker_logs, extract_http_body};

    #[test]
    fn decodes_plain_json_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]";
        assert_eq!(extract_http_body(raw).as_deref(), Some(b"[]".as_slice()));
    }

    #[test]
    fn decodes_chunked_json_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n[]\r\n0\r\n\r\n";
        assert_eq!(extract_http_body(raw).as_deref(), Some(b"[]".as_slice()));
    }

    #[test]
    fn decodes_multiplexed_docker_log_frames() {
        let mut framed = vec![1, 0, 0, 0, 0, 0, 0, 6];
        framed.extend_from_slice(b"hello\n");
        framed.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 4]);
        framed.extend_from_slice(b"err\n");
        assert_eq!(decode_docker_logs(&framed), "hello\nerr\n");
    }
}
