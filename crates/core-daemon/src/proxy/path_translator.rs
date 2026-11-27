use serde_json::Value;

/// Converts a Windows host path (e.g. `C:\Users\dev\app`, `d:/data`, `/c/Users/dev`, or
/// `\\wsl.localhost\Ubuntu\home\dev`) into its WSL2 Linux equivalent.
/// Returns `None` if the input is not a Windows or WSL UNC path.
pub fn windows_path_to_wsl(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    // Standard Windows drive path: `C:\...` or `C:/...`
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = input[3..].replace('\\', "/");
        let trimmed = rest.trim_start_matches('/');
        if trimmed.is_empty() {
            return Some(format!("/mnt/{drive}"));
        }
        return Some(format!("/mnt/{drive}/{trimmed}"));
    }

    // Git Bash / MSYS2 drive path: `/c/Users/...` or `/c`
    if bytes.len() >= 2
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && (bytes.len() == 2 || bytes[2] == b'/')
    {
        let drive = (bytes[1] as char).to_ascii_lowercase();
        let rest = if bytes.len() > 2 {
            input[3..].trim_start_matches('/')
        } else {
            ""
        };
        if rest.is_empty() {
            return Some(format!("/mnt/{drive}"));
        }
        return Some(format!("/mnt/{drive}/{rest}"));
    }

    // WSL UNC path: `\\wsl.localhost\<distro>\...` or `\\wsl$\<distro>\...`
    let normalized = input.replace('\\', "/");
    for prefix in ["//wsl.localhost/", "//wsl$/"] {
        if let Some(after_prefix) = strip_prefix_ignore_ascii_case(&normalized, prefix) {
            if let Some((_, linux_rest)) = after_prefix.split_once('/') {
                let trimmed = linux_rest.trim_start_matches('/');
                return Some(if trimmed.is_empty() {
                    "/".to_string()
                } else {
                    format!("/{trimmed}")
                });
            }
            if !after_prefix.is_empty() {
                return Some("/".to_string());
            }
        }
    }

    None
}

fn strip_prefix_ignore_ascii_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// Rewrites Windows bind-mount paths inside a Docker `POST /containers/create` JSON payload.
/// Returns `Some(new_bytes)` if any path was modified, or `None` if unchanged.
pub fn translate_container_create_json(body: &[u8]) -> Option<Vec<u8>> {
    let mut root: Value = serde_json::from_slice(body).ok()?;
    let host_config = root.get_mut("HostConfig")?.as_object_mut()?;
    let mut changed = false;

    if let Some(binds) = host_config.get_mut("Binds").and_then(Value::as_array_mut) {
        for bind in binds.iter_mut() {
            if let Some(spec) = bind.as_str() {
                if let Some(rewritten) = rewrite_bind_spec(spec) {
                    *bind = Value::String(rewritten);
                    changed = true;
                }
            }
        }
    }

    if let Some(mounts) = host_config.get_mut("Mounts").and_then(Value::as_array_mut) {
        for mount in mounts.iter_mut() {
            let is_bind = mount
                .get("Type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "bind");
            if is_bind {
                if let Some(src) = mount.get_mut("Source") {
                    if let Some(s) = src.as_str().and_then(windows_path_to_wsl) {
                        *src = Value::String(s);
                        changed = true;
                    }
                }
            }
        }
    }

    if changed {
        serde_json::to_vec(&root).ok()
    } else {
        None
    }
}

/// Rewrites `C:\host\dir:/container/dir[:ro]`, `/c/host/dir:/container/dir`, or
/// `\\wsl.localhost\distro\dir:/container/dir` -> `<wsl_host>:/container/dir[:ro]`.
fn rewrite_bind_spec(spec: &str) -> Option<String> {
    let bytes = spec.as_bytes();
    let colon_idx = if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        // Windows drive prefix occupies index 0..2 (`C:`), so the container separator `:` is at index >= 2.
        spec[2..].find(':').map(|offset| 2 + offset)
    } else if spec.starts_with('/') || spec.starts_with("\\\\") {
        spec.find(':')
    } else {
        None
    }?;

    let host_part = &spec[..colon_idx];
    let target_and_opts = &spec[colon_idx..];
    let wsl_host = windows_path_to_wsl(host_part)?;
    Some(format!("{wsl_host}{target_and_opts}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_windows_paths_to_wsl() {
        assert_eq!(
            windows_path_to_wsl(r"C:\Users\dev\app").as_deref(),
            Some("/mnt/c/Users/dev/app")
        );
        assert_eq!(
            windows_path_to_wsl("D:/data/").as_deref(),
            Some("/mnt/d/data/")
        );
        assert_eq!(
            windows_path_to_wsl("/c/Users/dev/app").as_deref(),
            Some("/mnt/c/Users/dev/app")
        );
        assert_eq!(windows_path_to_wsl("/d").as_deref(), Some("/mnt/d"));
        assert_eq!(
            windows_path_to_wsl(r"\\wsl.localhost\Ubuntu\home\dev\app").as_deref(),
            Some("/home/dev/app")
        );
        assert_eq!(
            windows_path_to_wsl(r"\\wsl$\nanodock\var\lib\data").as_deref(),
            Some("/var/lib/data")
        );
        assert_eq!(windows_path_to_wsl("/var/run/docker.sock"), None);
        assert_eq!(windows_path_to_wsl("/mnt/c/already"), None);
        assert_eq!(windows_path_to_wsl("my_named_volume"), None);
    }

    #[test]
    fn rewrites_binds_and_mounts_in_container_create() {
        let payload = serde_json::json!({
            "Image": "alpine",
            "HostConfig": {
                "Binds": [
                    "C:\\src\\app:/app:ro",
                    "/d/workspace/api:/api:rw",
                    "\\\\wsl.localhost\\Ubuntu\\home\\dev:/home/dev",
                    "/var/run/docker.sock:/var/run/docker.sock",
                    "named_vol:/data"
                ],
                "Mounts": [
                    { "Type": "bind", "Source": "D:\\cache", "Target": "/cache" },
                    { "Type": "bind", "Source": "/c/temp", "Target": "/tmp/host" },
                    { "Type": "volume", "Source": "pgdata", "Target": "/var/lib/postgresql" }
                ]
            }
        });

        let raw = serde_json::to_vec(&payload).unwrap();
        let translated = translate_container_create_json(&raw).expect("should translate");
        let parsed: Value = serde_json::from_slice(&translated).unwrap();

        assert_eq!(
            parsed["HostConfig"]["Binds"][0].as_str(),
            Some("/mnt/c/src/app:/app:ro")
        );
        assert_eq!(
            parsed["HostConfig"]["Binds"][1].as_str(),
            Some("/mnt/d/workspace/api:/api:rw")
        );
        assert_eq!(
            parsed["HostConfig"]["Binds"][2].as_str(),
            Some("/home/dev:/home/dev")
        );
        assert_eq!(
            parsed["HostConfig"]["Binds"][3].as_str(),
            Some("/var/run/docker.sock:/var/run/docker.sock")
        );
        assert_eq!(
            parsed["HostConfig"]["Binds"][4].as_str(),
            Some("named_vol:/data")
        );
        assert_eq!(
            parsed["HostConfig"]["Mounts"][0]["Source"].as_str(),
            Some("/mnt/d/cache")
        );
        assert_eq!(
            parsed["HostConfig"]["Mounts"][1]["Source"].as_str(),
            Some("/mnt/c/temp")
        );
        assert_eq!(
            parsed["HostConfig"]["Mounts"][2]["Source"].as_str(),
            Some("pgdata")
        );
    }
}
