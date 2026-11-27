use serde_json::Value;
use shared_types::TurboMountRule;

/// Result of translating `/containers/create` JSON bind-mounts.
pub struct TranslationResult {
    pub body: Option<Vec<u8>>,
    pub matched_turbo_ids: Vec<String>,
}

/// Normalizes a Windows path for case-insensitive prefix matching.
fn normalize_win_path(path: &str) -> String {
    let mut s = path.trim().replace('\\', "/");
    while s.len() > 3 && s.ends_with('/') {
        s.pop();
    }
    // Normalize `/c/...` (MSYS2/Git Bash) to `c:/...` for matching against `C:\...` rules.
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && (bytes.len() == 2 || bytes[2] == b'/')
    {
        let drive = (bytes[1] as char).to_ascii_lowercase();
        let rest = if bytes.len() > 2 { &s[2..] } else { "" };
        return format!("{drive}:{rest}");
    }
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        return format!("{drive}{}", &s[1..]);
    }
    s
}

/// Matches a host path against enabled `TurboMountRule`s, returning `(wsl_path, rule_id)`.
pub fn match_turbo_mount(host_path: &str, rules: &[TurboMountRule]) -> Option<(String, String)> {
    let norm_host = normalize_win_path(host_path);
    let mut best: Option<(usize, String, String)> = None;

    for rule in rules {
        if !rule.enabled {
            continue;
        }
        let norm_rule = normalize_win_path(&rule.windows_path);
        if norm_host.eq_ignore_ascii_case(&norm_rule) {
            let target = rule.wsl_path.trim_end_matches('/').to_string();
            if best.as_ref().is_none_or(|(len, _, _)| norm_rule.len() > *len) {
                best = Some((norm_rule.len(), target, rule.id.clone()));
            }
        } else if norm_host.len() > norm_rule.len()
            && norm_host[..norm_rule.len()].eq_ignore_ascii_case(&norm_rule)
            && norm_host.as_bytes()[norm_rule.len()] == b'/'
        {
            let suffix = &norm_host[norm_rule.len()..];
            let target = format!("{}{}", rule.wsl_path.trim_end_matches('/'), suffix);
            if best.as_ref().is_none_or(|(len, _, _)| norm_rule.len() > *len) {
                best = Some((norm_rule.len(), target, rule.id.clone()));
            }
        }
    }

    best.map(|(_, target, id)| (target, id))
}

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
#[allow(dead_code)]
pub fn translate_container_create_json(body: &[u8]) -> Option<Vec<u8>> {
    translate_container_create_json_with_turbo(body, &[]).body
}

/// Rewrites Windows bind-mount paths using `turbo_mounts` first (fast ext4), falling back
/// to `/mnt/<drive>/...` when no Turbo Mount rule matches.
pub fn translate_container_create_json_with_turbo(
    body: &[u8],
    turbo_mounts: &[TurboMountRule],
) -> TranslationResult {
    let Ok(mut root) = serde_json::from_slice::<Value>(body) else {
        return TranslationResult {
            body: None,
            matched_turbo_ids: Vec::new(),
        };
    };
    let Some(host_config) = root.get_mut("HostConfig").and_then(Value::as_object_mut) else {
        return TranslationResult {
            body: None,
            matched_turbo_ids: Vec::new(),
        };
    };
    let mut changed = false;
    let mut matched_turbo_ids = Vec::new();

    if let Some(binds) = host_config.get_mut("Binds").and_then(Value::as_array_mut) {
        for bind in binds.iter_mut() {
            if let Some(spec) = bind.as_str() {
                if let Some((rewritten, turbo_id)) = rewrite_bind_spec(spec, turbo_mounts) {
                    *bind = Value::String(rewritten);
                    changed = true;
                    if let Some(id) = turbo_id {
                        if !matched_turbo_ids.contains(&id) {
                            matched_turbo_ids.push(id);
                        }
                    }
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
                    if let Some(raw_src) = src.as_str() {
                        if let Some((turbo_wsl, id)) = match_turbo_mount(raw_src, turbo_mounts) {
                            *src = Value::String(turbo_wsl);
                            changed = true;
                            if !matched_turbo_ids.contains(&id) {
                                matched_turbo_ids.push(id);
                            }
                        } else if let Some(s) = windows_path_to_wsl(raw_src) {
                            *src = Value::String(s);
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    TranslationResult {
        body: if changed {
            serde_json::to_vec(&root).ok()
        } else {
            None
        },
        matched_turbo_ids,
    }
}

/// Rewrites `C:\host\dir:/container/dir[:ro]`, `/c/host/dir:/container/dir`, or
/// `\\wsl.localhost\distro\dir:/container/dir` -> `<wsl_host>:/container/dir[:ro]`.
fn rewrite_bind_spec(spec: &str, turbo_mounts: &[TurboMountRule]) -> Option<(String, Option<String>)> {
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
    if let Some((turbo_wsl, id)) = match_turbo_mount(host_part, turbo_mounts) {
        return Some((format!("{turbo_wsl}{target_and_opts}"), Some(id)));
    }
    let wsl_host = windows_path_to_wsl(host_part)?;
    Some((format!("{wsl_host}{target_and_opts}"), None))
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

    #[test]
    fn prefers_turbo_mount_over_mnt_c_including_subdirectories() {
        let rules = vec![TurboMountRule {
            id: "myapp".to_string(),
            windows_path: r"C:\Users\dev\my-app".to_string(),
            wsl_path: "/var/lib/nanodock/turbo/myapp".to_string(),
            excludes: vec!["node_modules".to_string()],
            enabled: true,
        }];
        let payload = serde_json::json!({
            "Image": "node:20",
            "HostConfig": {
                "Binds": [
                    "C:\\Users\\dev\\my-app:/app",
                    "/c/Users/dev/my-app/packages/api:/api:ro",
                    "D:\\other:/other"
                ]
            }
        });
        let raw = serde_json::to_vec(&payload).unwrap();
        let res = translate_container_create_json_with_turbo(&raw, &rules);
        assert_eq!(res.matched_turbo_ids, vec!["myapp".to_string()]);
        let parsed: Value = serde_json::from_slice(&res.body.unwrap()).unwrap();
        assert_eq!(
            parsed["HostConfig"]["Binds"][0].as_str(),
            Some("/var/lib/nanodock/turbo/myapp:/app")
        );
        assert_eq!(
            parsed["HostConfig"]["Binds"][1].as_str(),
            Some("/var/lib/nanodock/turbo/myapp/packages/api:/api:ro")
        );
        assert_eq!(
            parsed["HostConfig"]["Binds"][2].as_str(),
            Some("/mnt/d/other:/other")
        );
    }
}
