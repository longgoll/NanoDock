use std::collections::BTreeMap;
use std::path::Path;

use shared_types::WslConfigSpec;

use crate::error::WslError;

/// Computes default WSL2 memory limit: 50% of physical RAM, clamped between 2048 MB and 8192 MB.
pub fn recommended_memory_mb(total_physical_ram_mb: u64) -> u64 {
    let half = total_physical_ram_mb / 2;
    half.clamp(2048, 8192)
}

/// Safely merges NanoDock's anti-OOM and sparse VHD settings into an existing `.wslconfig`
/// string, preserving all user-defined sections and custom keys under `[wsl2]`.
///
/// Windows 11 / WSL 2.0+ only keys (`networkingMode`, `dnsTunneling`, `autoMemoryReclaim`)
/// are only injected when `supports_win11_wsl2_features` is true.
pub fn merge_wslconfig_content(
    existing: &str,
    spec: &WslConfigSpec,
    supports_win11_wsl2_features: bool,
) -> String {
    let mut sections: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut current_section = String::new();

    for raw_line in existing.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_string();
            sections.entry(current_section.clone()).or_default();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            sections
                .entry(current_section.clone())
                .or_default()
                .push((key, val));
        }
    }

    let wsl2_entries = sections.entry("wsl2".to_string()).or_default();
    upsert_key(wsl2_entries, "memory", &format!("{}MB", spec.memory_mb));
    upsert_key(wsl2_entries, "swap", &format!("{}MB", spec.swap_mb));
    upsert_key(
        wsl2_entries,
        "sparseVhd",
        if spec.sparse_vhd { "true" } else { "false" },
    );

    if supports_win11_wsl2_features {
        upsert_key(wsl2_entries, "autoMemoryReclaim", "gradual");
        if spec.mirrored_networking {
            upsert_key(wsl2_entries, "networkingMode", "mirrored");
            upsert_key(wsl2_entries, "dnsTunneling", "true");
        }
    }

    render_ini(&sections)
}

/// Backs up `%USERPROFILE%\.wslconfig` to `.wslconfig.bak` (if present) and writes the merged config.
pub fn ensure_wslconfig(
    wslconfig_path: &Path,
    spec: &WslConfigSpec,
    supports_win11_wsl2_features: bool,
) -> Result<(), WslError> {
    let existing = if wslconfig_path.exists() {
        let backup = wslconfig_path.with_extension("wslconfig.bak");
        std::fs::copy(wslconfig_path, backup)?;
        std::fs::read_to_string(wslconfig_path)?
    } else {
        String::new()
    };

    let merged = merge_wslconfig_content(&existing, spec, supports_win11_wsl2_features);
    if let Some(parent) = wslconfig_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(wslconfig_path, merged)?;
    Ok(())
}

fn upsert_key(entries: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some((_, existing_val)) = entries
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
    {
        *existing_val = value.to_string();
    } else {
        entries.push((key.to_string(), value.to_string()));
    }
}

fn render_ini(sections: &BTreeMap<String, Vec<(String, String)>>) -> String {
    let mut out = String::new();
    for (section, entries) in sections {
        if !section.is_empty() {
            out.push_str(&format!("[{section}]\n"));
        }
        for (k, v) in entries {
            out.push_str(&format!("{k}={v}\n"));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_recommended_memory() {
        assert_eq!(recommended_memory_mb(2048), 2048);
        assert_eq!(recommended_memory_mb(8192), 4096);
        assert_eq!(recommended_memory_mb(16384), 8192);
        assert_eq!(recommended_memory_mb(65536), 8192);
    }

    #[test]
    fn preserves_custom_keys_and_updates_wsl2() {
        let existing = "[wsl2]\nprocessors=6\nmemory=2GB\n\n[experimental]\nsparseVhd=true\n";
        let spec = WslConfigSpec {
            memory_mb: 4096,
            swap_mb: 4096,
            sparse_vhd: true,
            mirrored_networking: true,
        };
        let merged = merge_wslconfig_content(existing, &spec, true);
        assert!(merged.contains("processors=6"));
        assert!(merged.contains("memory=4096MB"));
        assert!(merged.contains("swap=4096MB"));
        assert!(merged.contains("networkingMode=mirrored"));
        assert!(merged.contains("[experimental]"));
    }

    #[test]
    fn omits_win11_keys_when_unsupported() {
        let spec = WslConfigSpec::default();
        let merged = merge_wslconfig_content("", &spec, false);
        assert!(!merged.contains("networkingMode"));
        assert!(!merged.contains("autoMemoryReclaim"));
        assert!(merged.contains("swap=4096MB"));
    }
}
