use anyhow::Context as _;
use serde_json::Value;
use shared_types::ContainerFileEntry;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use wsl_bridge::WslExecutor;

use crate::lifecycle::docker_api::{percent_encode_query, sanitize_path_segment};
use crate::transport::TransportManager;

const MAX_FILE_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_DIR_ENTRIES: usize = 2000;
const TAR_HEADER_BYTES: usize = 512;

/// Lists direct children of a directory inside a container via streaming `GET /containers/{id}/archive`.
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
    let mut reader = open_archive_stream(transport, clean_id, normalized).await?;
    parse_tar_directory_listing_stream(&mut reader, normalized).await
}

/// Reads a single file from a container via streaming `GET /containers/{id}/archive`, capped at 64 KiB.
pub async fn read_container_file<E: WslExecutor>(
    transport: &TransportManager<E>,
    id: &str,
    file_path: &str,
) -> anyhow::Result<(String, bool)> {
    let clean_id = sanitize_path_segment(id)?;
    let mut reader = open_archive_stream(transport, clean_id, file_path.trim()).await?;
    extract_first_tar_file_stream(&mut reader, MAX_FILE_PREVIEW_BYTES)
        .await?
        .context("target path is not a readable regular file")
}

async fn open_archive_stream<E: WslExecutor>(
    transport: &TransportManager<E>,
    clean_id: &str,
    path: &str,
) -> anyhow::Result<HttpBodyReader<impl AsyncRead + Unpin>> {
    let encoded = percent_encode_query(path);
    let mut stream = transport.open_docker_stream().await?;
    let req = format!(
        "GET /containers/{clean_id}/archive?path={encoded} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .context("malformed HTTP status from dockerd")?;

    let mut chunked = false;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("transfer-encoding")
                && val.to_ascii_lowercase().contains("chunked")
            {
                chunked = true;
            }
        }
    }

    if status >= 400 {
        let mut err_buf = vec![0u8; 4096];
        let n = reader.read(&mut err_buf).await.unwrap_or(0);
        let msg = serde_json::from_slice::<Value>(&err_buf[..n])
            .ok()
            .and_then(|v| v.get("message").and_then(Value::as_str).map(ToString::to_string))
            .unwrap_or_else(|| String::from_utf8_lossy(&err_buf[..n]).trim().to_string());
        anyhow::bail!("Docker API error ({status}): {msg}");
    }

    Ok(HttpBodyReader {
        inner: reader,
        chunked,
        chunk_remaining: 0,
        eof: false,
    })
}

struct HttpBodyReader<R> {
    inner: BufReader<R>,
    chunked: bool,
    chunk_remaining: usize,
    eof: bool,
}

impl<R: AsyncRead + Unpin> HttpBodyReader<R> {
    async fn read_exact_or_eof(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let mut total = 0;
        while total < buf.len() {
            let n = self.read_some(&mut buf[total..]).await?;
            if n == 0 {
                break;
            }
            total += n;
        }
        Ok(total)
    }

    async fn skip_exact(&mut self, mut bytes: usize) -> anyhow::Result<()> {
        let mut scratch = [0u8; 8192];
        while bytes > 0 {
            let to_read = bytes.min(scratch.len());
            let n = self.read_some(&mut scratch[..to_read]).await?;
            if n == 0 {
                break;
            }
            bytes -= n;
        }
        Ok(())
    }

    async fn read_some(&mut self, dst: &mut [u8]) -> anyhow::Result<usize> {
        if self.eof || dst.is_empty() {
            return Ok(0);
        }
        if !self.chunked {
            let n = self.inner.read(dst).await?;
            if n == 0 {
                self.eof = true;
            }
            return Ok(n);
        }

        if self.chunk_remaining == 0 {
            let mut size_line = String::new();
            loop {
                size_line.clear();
                let n = self.inner.read_line(&mut size_line).await?;
                if n == 0 {
                    self.eof = true;
                    return Ok(0);
                }
                let trimmed = size_line.trim();
                if !trimmed.is_empty() {
                    let hex_part = trimmed.split(';').next().unwrap_or("");
                    let size = usize::from_str_radix(hex_part, 16)
                        .context("invalid HTTP chunk size in archive stream")?;
                    if size == 0 {
                        self.eof = true;
                        return Ok(0);
                    }
                    self.chunk_remaining = size;
                    break;
                }
            }
        }

        let to_read = dst.len().min(self.chunk_remaining);
        let n = self.inner.read(&mut dst[..to_read]).await?;
        if n == 0 {
            self.eof = true;
            return Ok(0);
        }
        self.chunk_remaining -= n;
        if self.chunk_remaining == 0 {
            let mut crlf = [0u8; 2];
            let _ = self.inner.read_exact(&mut crlf).await;
        }
        Ok(n)
    }
}

async fn parse_tar_directory_listing_stream<R: AsyncRead + Unpin>(
    reader: &mut HttpBodyReader<R>,
    base_dir: &str,
) -> anyhow::Result<Vec<ContainerFileEntry>> {
    let base_trimmed = base_dir.trim_end_matches('/');
    let expected_root = base_trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty());
    let mut entries = Vec::new();
    let mut root_prefix: Option<String> = None;
    let mut header = [0u8; TAR_HEADER_BYTES];

    while entries.len() < MAX_DIR_ENTRIES {
        let n = reader.read_exact_or_eof(&mut header).await?;
        if n < TAR_HEADER_BYTES || header.iter().all(|&b| b == 0) {
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

        let data_bytes = size.div_ceil(TAR_HEADER_BYTES) * TAR_HEADER_BYTES;
        reader.skip_exact(data_bytes).await?;

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
    Ok(entries)
}

async fn extract_first_tar_file_stream<R: AsyncRead + Unpin>(
    reader: &mut HttpBodyReader<R>,
    max_bytes: usize,
) -> anyhow::Result<Option<(String, bool)>> {
    let mut header = [0u8; TAR_HEADER_BYTES];
    loop {
        let n = reader.read_exact_or_eof(&mut header).await?;
        if n < TAR_HEADER_BYTES || header.iter().all(|&b| b == 0) {
            break;
        }
        let size = parse_tar_octal(&header[124..136]).unwrap_or(0) as usize;
        let typeflag = header[156];

        if typeflag == b'0' || typeflag == 0 {
            let truncated = size > max_bytes;
            let to_read = size.min(max_bytes);
            let mut buf = vec![0u8; to_read];
            let actual = reader.read_exact_or_eof(&mut buf).await?;
            buf.truncate(actual);
            return Ok(Some((String::from_utf8_lossy(&buf).into_owned(), truncated)));
        }

        let data_bytes = size.div_ceil(TAR_HEADER_BYTES) * TAR_HEADER_BYTES;
        reader.skip_exact(data_bytes).await?;
    }
    Ok(None)
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
        hdr[100..108].copy_from_slice(b"0000755\0");
        let size_oct = format!("{size:011o}\0");
        hdr[124..136].copy_from_slice(size_oct.as_bytes());
        hdr[156] = typeflag;
        hdr
    }

    #[tokio::test]
    async fn streams_tar_directory_listing_and_reads_capped_file() {
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

        let mut list_reader = HttpBodyReader {
            inner: BufReader::new(std::io::Cursor::new(tar.clone())),
            chunked: false,
            chunk_remaining: 0,
            eof: false,
        };
        let listing = parse_tar_directory_listing_stream(&mut list_reader, "/app")
            .await
            .unwrap();
        assert_eq!(listing.len(), 2);
        assert!(listing[0].is_dir);
        assert_eq!(listing[0].name, "sub");
        assert_eq!(listing[1].name, "hello.txt");
        assert_eq!(listing[1].size, 5);

        let mut file_reader = HttpBodyReader {
            inner: BufReader::new(std::io::Cursor::new(tar)),
            chunked: false,
            chunk_remaining: 0,
            eof: false,
        };
        let (content, truncated) = extract_first_tar_file_stream(&mut file_reader, 3)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(content, "wor");
        assert!(truncated);
    }

    #[tokio::test]
    async fn decodes_chunked_tar_stream_without_buffering() {
        let mut tar = Vec::new();
        tar.extend_from_slice(&make_tar_header("./", 0, b'5'));
        tar.extend_from_slice(&make_tar_header("./etc/", 0, b'5'));
        tar.extend_from_slice(&[0u8; 1024]);

        let chunked = format!("{:x}\r\n", tar.len())
            .into_bytes()
            .into_iter()
            .chain(tar)
            .chain(b"\r\n0\r\n\r\n".iter().copied())
            .collect::<Vec<u8>>();

        let mut reader = HttpBodyReader {
            inner: BufReader::new(std::io::Cursor::new(chunked)),
            chunked: true,
            chunk_remaining: 0,
            eof: false,
        };
        let listing = parse_tar_directory_listing_stream(&mut reader, "/")
            .await
            .unwrap();
        assert_eq!(listing.len(), 1);
        assert_eq!(listing[0].name, "etc");
        assert_eq!(listing[0].path, "/etc");
    }
}
