use anyhow::Context as _;
use serde_json::Value;
use shared_types::ImageLayer;
use wsl_bridge::WslExecutor;

use crate::lifecycle::docker_api::{docker_http_request, sanitize_path_segment};
use crate::transport::TransportManager;

/// Fetches layer history for a Docker image (`GET /images/{id}/history`).
pub async fn inspect_image_layers<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
) -> anyhow::Result<Vec<ImageLayer>> {
    let clean_id = sanitize_path_segment(id)?;
    let (_, body) =
        docker_http_request(transport, "GET", &format!("/images/{clean_id}/history")).await?;
    parse_image_layers_json(&body)
}

pub fn parse_image_layers_json(body: &[u8]) -> anyhow::Result<Vec<ImageLayer>> {
    let items: Vec<Value> =
        serde_json::from_slice(body).context("failed to parse /images/{id}/history")?;
    Ok(items
        .into_iter()
        .map(|item| {
            let id = item
                .get("Id")
                .and_then(Value::as_str)
                .unwrap_or("<missing>")
                .trim_start_matches("sha256:")
                .chars()
                .take(12)
                .collect();
            let created = item.get("Created").and_then(Value::as_i64).unwrap_or(0);
            let created_by = item
                .get("CreatedBy")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let size = item.get("Size").and_then(Value::as_i64).unwrap_or(0);
            let comment = item
                .get("Comment")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            ImageLayer {
                id,
                created,
                created_by,
                size,
                comment,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_image_history_layers() {
        let json = br#"[
            {"Id":"sha256:abcdef1234567890","Created":1700000000,"CreatedBy":"CMD [\"sh\"]","Size":0,"Comment":""},
            {"Id":"<missing>","Created":1699999000,"CreatedBy":"ADD rootfs.tar.gz /","Size":7340032,"Comment":"base"}
        ]"#;
        let layers = parse_image_layers_json(json).unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].id, "abcdef123456");
        assert_eq!(layers[1].size, 7340032);
        assert_eq!(layers[1].comment, "base");
    }
}
