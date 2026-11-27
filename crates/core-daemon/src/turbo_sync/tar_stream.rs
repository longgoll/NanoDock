use std::path::{Path, PathBuf};

use tracing::warn;

const MAX_SINGLE_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Builds an uncompressed USTAR archive in memory from `rel_paths` rooted at `root`.
/// Files that vanish mid-scan, exceed 64 MiB, or exceed the 255-byte USTAR path limit
/// are skipped with a warning so the rest of the batch succeeds.
pub fn build_ustar_archive(root: &Path, rel_paths: &[PathBuf]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();

    for rel in rel_paths {
        let full_path = root.join(rel);
        let Ok(meta) = std::fs::metadata(&full_path) else {
            continue;
        };
        if meta.len() > MAX_SINGLE_FILE_BYTES {
            warn!(path = %full_path.display(), size = meta.len(), "Skipping oversized file in Turbo Mount");
            continue;
        }
        let Ok(bytes) = std::fs::read(&full_path) else {
            continue;
        };
        let rel_posix = rel.to_string_lossy().replace('\\', "/");
        if rel_posix.is_empty() {
            continue;
        }

        if let Err(err) = append_ustar_entry(&mut out, &rel_posix, &bytes) {
            warn!(path = %rel_posix, error = %err, "Skipping file in Turbo Mount archive");
        }
    }

    // Standard 1024-byte EOF trailer.
    out.resize(out.len() + 1024, 0);
    Ok(out)
}

fn append_ustar_entry(out: &mut Vec<u8>, name: &str, data: &[u8]) -> std::io::Result<()> {
    let (prefix, short_name) = split_ustar_name(name)?;
    let mut header = [0u8; 512];

    write_str(&mut header[0..100], short_name);
    write_octal(&mut header[100..108], 0o644, 7);
    write_octal(&mut header[108..116], 0, 7);
    write_octal(&mut header[116..124], 0, 7);
    write_octal(&mut header[124..136], data.len() as u64, 11);
    write_octal(&mut header[136..148], 0, 11);
    header[148..156].fill(b' ');
    header[156] = b'0';
    write_str(&mut header[257..263], "ustar");
    header[263..265].copy_from_slice(b"00");
    if !prefix.is_empty() {
        write_str(&mut header[345..500], prefix);
    }

    let checksum: u32 = header.iter().map(|&b| b as u32).sum();
    let chk_str = format!("{checksum:06o}\0 ");
    header[148..156].copy_from_slice(chk_str.as_bytes());

    out.extend_from_slice(&header);
    out.extend_from_slice(data);
    let rem = data.len() % 512;
    if rem != 0 {
        out.resize(out.len() + (512 - rem), 0);
    }
    Ok(())
}

fn split_ustar_name(name: &str) -> std::io::Result<(&str, &str)> {
    if name.len() <= 100 {
        return Ok(("", name));
    }
    if let Some((idx, _)) = name
        .match_indices('/')
        .find(|(idx, _)| *idx <= 155 && name.len() - idx - 1 <= 100)
    {
        return Ok((&name[..idx], &name[idx + 1..]));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("path exceeds USTAR 255-byte limit: {name}"),
    ))
}

fn write_str(dst: &mut [u8], s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(dst.len());
    dst[..len].copy_from_slice(&bytes[..len]);
}

fn write_octal(dst: &mut [u8], val: u64, width: usize) {
    let formatted = format!("{val:0width$o}\0", width = width);
    let bytes = formatted.as_bytes();
    let len = bytes.len().min(dst.len());
    dst[..len].copy_from_slice(&bytes[..len]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_ustar_archive_with_checksum() {
        let dir = std::env::temp_dir().join(format!("nanodock-tar-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/hello.txt"), b"hello nanodock").unwrap();

        let archive = build_ustar_archive(&dir, &[PathBuf::from("src/hello.txt")]).unwrap();
        assert_eq!(archive.len() % 512, 0);
        assert!(archive.len() >= 512 + 512 + 1024);
        assert_eq!(&archive[257..262], b"ustar");
        assert!(archive[512..].starts_with(b"hello nanodock"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
