use std::path::Path;

use serde_json::Value;

/// Warning emitted when `%USERPROFILE%\.docker\config.json` still references Docker Desktop's
/// credential helper, which fails once Docker Desktop is uninstalled/stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredsStoreWarning {
    pub creds_store: String,
    pub message: String,
}

/// Inspects raw JSON content of `~/.docker/config.json` for `credsStore: "desktop"`.
pub fn check_creds_store_content(json_str: &str) -> Option<CredsStoreWarning> {
    let val: Value = serde_json::from_str(json_str).ok()?;
    let store = val.get("credsStore")?.as_str()?;
    if store.eq_ignore_ascii_case("desktop") {
        Some(CredsStoreWarning {
            creds_store: store.to_string(),
            message: "Docker config uses credsStore=\"desktop\"; remove or change it if Docker Desktop is not installed.".to_string(),
        })
    } else {
        None
    }
}

/// Checks `%USERPROFILE%\.docker\config.json` if it exists on disk.
pub fn check_user_docker_config(user_profile: &Path) -> Option<CredsStoreWarning> {
    let config_path = user_profile.join(".docker").join("config.json");
    let content = std::fs::read_to_string(config_path).ok()?;
    check_creds_store_content(&content)
}

#[cfg(test)]
mod tests {
    use super::check_creds_store_content;

    #[test]
    fn detects_desktop_creds_store() {
        let raw = r#"{"auths":{},"credsStore":"desktop"}"#;
        let warning = check_creds_store_content(raw).expect("should warn");
        assert_eq!(warning.creds_store, "desktop");
    }

    #[test]
    fn ignores_other_or_missing_creds_store() {
        assert!(check_creds_store_content(r#"{"auths":{}}"#).is_none());
        assert!(check_creds_store_content(r#"{"credsStore":"wincred"}"#).is_none());
    }
}
