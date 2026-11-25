use serde_json::Value;

/// Converts a Windows host path (e.g. `C:\Users\dev\app` or `d:/data`) into its WSL2 `/mnt/<drive>/...` equivalent.
/// Returns `None` if the input is not a Windows drive-letter path.
pub fn windows_path_to_wsl(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
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
    None
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

/// Rewrites `C:\host\dir:/container/dir[:ro]` -> `/mnt/c/host/dir:/container/dir[:ro]`.
fn rewrite_bind_spec(spec: &str) -> Option<String> {
    // Windows drive prefix occupies index 0..2 (`C:`), so the container separator `:` is at index >= 2.
    let bytes = spec.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let rest_after_drive = &spec[2..];
        if let Some(colon_offset) = rest_after_drive.find(':') {
            let host_part = &spec[..2 + colon_offset];
            let target_and_opts = &spec[2 + colon_offset..];
            let wsl_host = windows_path_to_wsl(host_part)?;
            return Some(format!("{wsl_host}{target_and_opts}"));
        }
    }
    None
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
        assert_eq!(windows_path_to_wsl("/var/run/docker.sock"), None);
        assert_eq!(windows_path_to_wsl("my_named_volume"), None);
    }

    #[test]
    fn rewrites_binds_and_mounts_in_container_create() {
        let payload = serde_json::json!({
            "Image": "alpine",
            "HostConfig": {
                "Binds": [
                    "C:\\src\\app:/app:ro",
                    "named_vol:/data"
                ],
                "Mounts": [
                    { "Type": "bind", "Source": "D:\\cache", "Target": "/cache" },
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
            Some("named_vol:/data")
        );
        assert_eq!(
            parsed["HostConfig"]["Mounts"][0]["Source"].as_str(),
            Some("/mnt/d/cache")
        );
        assert_eq!(
            parsed["HostConfig"]["Mounts"][1]["Source"].as_str(),
            Some("pgdata")
        );
    }
}
