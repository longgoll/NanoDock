use shared_types::{ContainerSummary, LocalProxyRoute};

/// Builds `*.localhost` routes from running Docker containers with published TCP ports.
pub fn resolve_routes_from_containers(containers: &[ContainerSummary]) -> Vec<LocalProxyRoute> {
    let mut routes = Vec::new();

    for c in containers {
        if !c.state.eq_ignore_ascii_case("running") {
            continue;
        }
        let Some(target_port) = pick_preferred_host_port(&c.ports) else {
            continue;
        };
        let raw_name = c.names.first().map(String::as_str).unwrap_or(&c.id);
        let clean_name = sanitize_dns_label(raw_name.trim_start_matches('/'));
        if clean_name.is_empty() {
            continue;
        }

        let host = format!("{clean_name}.localhost");
        if !routes.iter().any(|r: &LocalProxyRoute| r.host == host) {
            routes.push(LocalProxyRoute {
                host,
                container_name: raw_name.trim_start_matches('/').to_string(),
                target_port,
                compose_project: c.compose_project.clone(),
            });
        }
    }

    routes
}

/// Extracts the hostname (lowercase, without `:port`) from an HTTP/1.1 `Host` header.
pub fn extract_host_header(buf: &[u8]) -> Option<String> {
    let header_str = std::str::from_utf8(buf).ok()?;
    for line in header_str.lines() {
        if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("host") {
                let host_only = val.trim().split(':').next().unwrap_or("").trim();
                if !host_only.is_empty() {
                    return Some(host_only.to_ascii_lowercase());
                }
            }
        }
    }
    None
}

fn pick_preferred_host_port(ports: &[String]) -> Option<u16> {
    let mut parsed_ports = Vec::new();
    for spec in ports {
        // Format: "0.0.0.0:8080->80/tcp" or "[::]:3000->3000/tcp" or "127.0.0.1:5432->5432/tcp"
        let Some((left, right)) = spec.split_once("->") else {
            continue;
        };
        if right.ends_with("/udp") {
            continue;
        }
        if let Some((_, port_str)) = left.rsplit_once(':') {
            if let Ok(port) = port_str.trim().parse::<u16>() {
                parsed_ports.push(port);
            }
        }
    }

    const PREFERRED: [u16; 8] = [80, 3000, 5173, 8080, 8000, 4000, 4200, 9000];
    for pref in PREFERRED {
        if parsed_ports.contains(&pref) {
            return Some(pref);
        }
    }
    parsed_ports.into_iter().next()
}

fn sanitize_dns_label(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_running_containers_to_localhost_routes() {
        let containers = vec![
            ContainerSummary {
                id: "1".into(),
                names: vec!["/my-web".into()],
                image: "nginx".into(),
                state: "running".into(),
                status: "Up".into(),
                created: 0,
                ports: vec!["0.0.0.0:8080->80/tcp".into()],
                compose_project: Some("demo".into()),
            },
            ContainerSummary {
                id: "2".into(),
                names: vec!["/stopped-db".into()],
                image: "postgres".into(),
                state: "exited".into(),
                status: "Exited".into(),
                created: 0,
                ports: vec!["0.0.0.0:5432->5432/tcp".into()],
                compose_project: None,
            },
        ];

        let routes = resolve_routes_from_containers(&containers);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].host, "my-web.localhost");
        assert_eq!(routes[0].target_port, 8080);
    }

    #[test]
    fn extracts_host_header_without_port() {
        let req = b"GET / HTTP/1.1\r\nHost: My-Web.localhost:8090\r\nConnection: close\r\n\r\n";
        assert_eq!(
            extract_host_header(req).as_deref(),
            Some("my-web.localhost")
        );
    }
}
