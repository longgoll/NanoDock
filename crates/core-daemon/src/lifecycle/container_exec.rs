use anyhow::Context as _;
use serde_json::Value;
use shared_types::ContainerFileEntry;
use wsl_bridge::WslExecutor;

use crate::lifecycle::docker_api::{
    decode_docker_logs, docker_http_json_request, docker_http_request, percent_encode_query,
    sanitize_path_segment,
};
use crate::transport::TransportManager;

const MAX_FILE_PREVIEW_BYTES: usize = 64 * 1024;

/// Executes a non-interactive command inside a running container (`exec create` + `exec start`).
pub async fn exec_container<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    command: &[String],
) -> anyhow::Result<(i64, String)> {
    let clean_id = sanitize_path_segment(id)?;
    if command.is_empty() {
        anyhow::bail!("command cannot be empty");
    }

    let create_body = serde_json::json!({
        "AttachStdout": true,
        "AttachStderr": true,
        "Tty": false,
        "Cmd": command,
    });
    let (_, create_resp) = docker_http_json_request(
        transport,
        "POST",
        &format!("/containers/{clean_id}/exec"),
        Some(&create_body),
    )
    .await?;

    let exec_val: Value =
        serde_json::from_slice(&create_resp).context("failed to parse exec create response")?;
    let exec_id = exec_val
        .get("Id")
        .and_then(Value::as_str)
        .context("missing exec Id")?;

    let start_body = serde_json::json!({ "Detach": false, "Tty": false });
    let (_, raw_output) = docker_http_json_request(
        transport,
        "POST",
        &format!("/exec/{exec_id}/start"),
        Some(&start_body),
    )
    .await?;

    let output = decode_docker_logs(&raw_output);
    let (_, inspect_resp) =
        docker_http_request(transport, "GET", &format!("/exec/{exec_id}/json")).await?;
    let inspect_val: Value = serde_json::from_slice(&inspect_resp).unwrap_or_default();
    let exit_code = inspect_val
        .get("ExitCode")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    Ok((exit_code, output))
}

/// Lists direct children of a directory inside a container using `GET /containers/{id}/archive`.
pub async fn list_container_files<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    dir_path: &str,
) -> anyhow::Result<Vec<ContainerFileEntry>> {
    let clean_id = sanitize_path_segment(id)?;
    let normalized = if dir_path.trim().is_empty() {
        "/"
    } else {
        dir_path.trim()
    };
    let encoded = percent_encode_query(normalized);
    let (_, tar_bytes) = docker_http_request(
        transport,
        "GET",
        &format!("/containers/{clean_id}/archive?path={encoded}"),
    )
    .await?;

    Ok(parse_tar_directory_listing(&tar_bytes, normalized))
}

/// Reads a single file from a container via `GET /containers/{id}/archive`, capped at 64 KiB.
pub async fn read_container_file<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    file_path: &str,
) -> anyhow::Result<(String, bool)> {
    let clean_id = sanitize_path_segment(id)?;
    let encoded = percent_encode_query(file_path.trim());
    let (_, tar_bytes) = docker_http_request(
        transport,
        "GET",
        &format!("/containers/{clean_id}/archive?path={encoded}"),
    )
    .await?;

    extract_first_tar_file(&tar_bytes, MAX_FILE_PREVIEW_BYTES)
        .context("target path is not a readable regular file")
}

/// Parses a standard ustar tar archive stream and returns immediate children of `base_dir`.
pub fn parse_tar_directory_listing(tar: &[u8], base_dir: &str) -> Vec<ContainerFileEntry> {
    let base_trimmed = base_dir.trim_end_matches('/');
    let expected_root = base_trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty());
    let mut entries = Vec::new();
    let mut offset = 0usize;
    let mut root_prefix: Option<String> = None;

    while offset + 512 <= tar.len() {
        let header = &tar[offset..offset + 512];
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let name_part = read_tar_string(&header[0..100]);
        let prefix_part = read_tar_string(&header[345..500]);
        let raw_name = if prefix_part.is_empty() {
            name_part
        } else {
            format!("{prefix_part}/{name_part}")
        };
        let size = parse_tar_octal(&header[124..136]).unwrap_or(0) as usize;
        let mode_num = parse_tar_octal(&header[100..108]).unwrap_or(0);
        let modified = parse_tar_octal(&header[136..148]).unwrap_or(0) as i64;
        let typeflag = header[156];
        let is_dir = typeflag == b'5';

        let data_blocks = size.div_ceil(512);
        offset += 512 + data_blocks * 512;

        if matches!(typeflag, b'x' | b'g' | b'L' | b'K') {
            continue;
        }

        let clean_name = raw_name.trim_start_matches("./").trim_end_matches('/');
        if clean_name.is_empty() || clean_name == "." {
            continue;
        }
        if root_prefix.is_none()
            && is_dir
            && !clean_name.contains('/')
            && expected_root.is_some_and(|root| root == clean_name)
        {
            root_prefix = Some(clean_name.to_string());
            continue;
        }

        let relative = match &root_prefix {
            Some(prefix) => clean_name
                .strip_prefix(prefix)
                .map(|s| s.trim_start_matches('/'))
                .unwrap_or(clean_name),
            None => clean_name,
        };

        if relative.is_empty() || relative.contains('/') {
            continue;
        }

        let full_path = if base_trimmed.is_empty() {
            format!("/{relative}")
        } else {
            format!("{base_trimmed}/{relative}")
        };

        entries.push(ContainerFileEntry {
            name: relative.to_string(),
            path: full_path,
            is_dir,
            size: size as u64,
            mode: format!("{:04o}", mode_num & 0o7777),
            modified,
        });
    }

    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    entries
}

fn extract_first_tar_file(tar: &[u8], max_bytes: usize) -> Option<(String, bool)> {
    let mut offset = 0usize;
    while offset + 512 <= tar.len() {
        let header = &tar[offset..offset + 512];
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let size = parse_tar_octal(&header[124..136]).unwrap_or(0) as usize;
        let typeflag = header[156];
        let data_start = offset + 512;
        let data_end = (data_start + size).min(tar.len());

        if typeflag == b'0' || typeflag == 0 {
            let truncated = size > max_bytes;
            let slice_end = data_start + size.min(max_bytes);
            let slice = &tar[data_start..slice_end.min(data_end)];
            return Some((String::from_utf8_lossy(slice).into_owned(), truncated));
        }

        let data_blocks = size.div_ceil(512);
        offset += 512 + data_blocks * 512;
    }
    None
}

fn read_tar_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn parse_tar_octal(bytes: &[u8]) -> Option<u64> {
    let s = read_tar_string(bytes);
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(trimmed, 8).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tar_header(name: &str, size: usize, typeflag: u8) -> [u8; 512] {
        let mut hdr = [0u8; 512];
        hdr[..name.len()].copy_from_slice(name.as_bytes());
        let mode = b"0000755\0";
        hdr[100..108].copy_from_slice(mode);
        let size_oct = format!("{size:011o}\0");
        hdr[124..136].copy_from_slice(size_oct.as_bytes());
        hdr[156] = typeflag;
        hdr
    }

    #[test]
    fn parses_tar_directory_listing_and_reads_file() {
        let mut tar = Vec::new();
        tar.extend_from_slice(&make_tar_header("app/", 0, b'5'));
        tar.extend_from_slice(&make_tar_header("app/sub/", 0, b'5'));
        tar.extend_from_slice(&make_tar_header("app/hello.txt", 5, b'0'));
        let mut data = [0u8; 512];
        data[..5].copy_from_slice(b"world");
        tar.extend_from_slice(&data);
        tar.extend_from_slice(&make_tar_header("app/sub/nested.txt", 2, b'0'));
        tar.extend_from_slice(&[0u8; 512]);
        tar.extend_from_slice(&[0u8; 1024]);

        let listing = parse_tar_directory_listing(&tar, "/app");
        assert_eq!(listing.len(), 2);
        assert!(listing[0].is_dir);
        assert_eq!(listing[0].name, "sub");
        assert_eq!(listing[1].name, "hello.txt");
        assert_eq!(listing[1].size, 5);

        let (content, truncated) = extract_first_tar_file(&tar, 1024).unwrap();
        assert_eq!(content, "world");
        assert!(!truncated);
    }

    #[test]
    fn parses_root_directory_without_skipping_first_folder() {
        let mut tar = Vec::new();
        tar.extend_from_slice(&make_tar_header("./", 0, b'5'));
        tar.extend_from_slice(&make_tar_header("./bin/", 0, b'5'));
        tar.extend_from_slice(&make_tar_header("./etc/", 0, b'5'));
        tar.extend_from_slice(&make_tar_header("./etc/hosts", 9, b'0'));
        tar.extend_from_slice(&[0u8; 512]);
        tar.extend_from_slice(&[0u8; 1024]);

        let listing = parse_tar_directory_listing(&tar, "/");
        assert_eq!(listing.len(), 2);
        assert_eq!(listing[0].name, "bin");
        assert_eq!(listing[0].path, "/bin");
        assert_eq!(listing[1].name, "etc");
        assert_eq!(listing[1].path, "/etc");
    }
}
