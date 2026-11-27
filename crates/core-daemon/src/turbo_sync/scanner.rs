use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Lightweight file metadata stamp used to detect modifications without hashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub mtime_ms: u64,
    pub size: u64,
}

/// Incremental diff between two directory snapshots.
#[derive(Debug, Default)]
pub struct ScanDiff {
    pub upserted: Vec<PathBuf>,
    pub deleted: Vec<String>,
    pub next_snapshot: BTreeMap<String, FileStamp>,
}

/// Scans `root` recursively, skipping `excludes`, and computes changed/added/deleted files
/// relative to `prev`.
pub fn scan_diff(
    root: &Path,
    prev: &BTreeMap<String, FileStamp>,
    excludes: &[String],
) -> std::io::Result<ScanDiff> {
    let mut next_snapshot = BTreeMap::new();
    let mut upserted = Vec::new();

    if root.exists() {
        walk_dir(root, root, excludes, prev, &mut next_snapshot, &mut upserted)?;
    }

    let deleted = prev
        .keys()
        .filter(|rel| !next_snapshot.contains_key(*rel))
        .cloned()
        .collect();

    Ok(ScanDiff {
        upserted,
        deleted,
        next_snapshot,
    })
}

fn walk_dir(
    root: &Path,
    dir: &Path,
    excludes: &[String],
    prev: &BTreeMap<String, FileStamp>,
    next_snapshot: &mut BTreeMap<String, FileStamp>,
    upserted: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if is_excluded(&name_str, excludes) {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
            walk_dir(root, &path, excludes, prev, next_snapshot, upserted)?;
        } else if file_type.is_file() {
            let meta = entry.metadata()?;
            let mtime_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let stamp = FileStamp {
                mtime_ms,
                size: meta.len(),
            };
            if let Ok(rel) = path.strip_prefix(root) {
                let rel_posix = rel.to_string_lossy().replace('\\', "/");
                let changed = prev.get(&rel_posix).is_none_or(|old| *old != stamp);
                if changed {
                    upserted.push(rel.to_path_buf());
                }
                next_snapshot.insert(rel_posix, stamp);
            }
        }
    }
    Ok(())
}

fn is_excluded(name: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|pattern| {
        let pat = pattern.trim();
        if pat.is_empty() {
            return false;
        }
        if let Some(suffix) = pat.strip_prefix('*') {
            name.ends_with(suffix)
        } else {
            name.eq_ignore_ascii_case(pat)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_added_modified_deleted_and_respects_excludes() {
        let dir = std::env::temp_dir().join(format!("nanodock-scan-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        std::fs::write(dir.join("src/index.ts"), "console.log(1)").unwrap();
        std::fs::write(dir.join("node_modules/pkg/a.js"), "ignored").unwrap();

        let excludes = vec!["node_modules".to_string(), "*.log".to_string()];
        let first = scan_diff(&dir, &BTreeMap::new(), &excludes).unwrap();
        assert_eq!(first.upserted.len(), 1);
        assert!(first.next_snapshot.contains_key("src/index.ts"));
        assert!(!first.next_snapshot.contains_key("node_modules/pkg/a.js"));

        // Second scan with no changes should yield empty diff.
        let second = scan_diff(&dir, &first.next_snapshot, &excludes).unwrap();
        assert!(second.upserted.is_empty());
        assert!(second.deleted.is_empty());

        // Delete file and add another.
        std::fs::remove_file(dir.join("src/index.ts")).unwrap();
        std::fs::write(dir.join("src/app.ts"), "export const x = 2;").unwrap();
        let third = scan_diff(&dir, &second.next_snapshot, &excludes).unwrap();
        assert_eq!(third.upserted, vec![PathBuf::from("src/app.ts")]);
        assert_eq!(third.deleted, vec!["src/index.ts".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
