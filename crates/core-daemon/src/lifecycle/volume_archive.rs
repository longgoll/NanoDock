use wsl_bridge::WslExecutor;

use crate::lifecycle::docker_api::sanitize_path_segment;
use crate::proxy::path_translator::windows_path_to_wsl;

/// Exports `/var/lib/docker/volumes/<name>/_data` directly to a Windows `.tar.gz` path
/// via the NanoDock WSL2 distro without pulling any extra helper image.
pub async fn export_volume<E: WslExecutor>(
    wsl: &E,
    distro_name: &str,
    volume_name: &str,
    dest_windows_path: &str,
) -> anyhow::Result<()> {
    let clean_name = sanitize_path_segment(volume_name)?;
    let wsl_dest = windows_path_to_wsl(dest_windows_path.trim())
        .ok_or_else(|| anyhow::anyhow!("destination must be an absolute Windows path"))?;

    let volume_dir = format!("/var/lib/docker/volumes/{clean_name}/_data");
    let script = format!(
        "mkdir -p \"$(dirname '{}')\" && tar -czf '{}' -C '{}' .",
        escape_single_quotes(&wsl_dest),
        escape_single_quotes(&wsl_dest),
        escape_single_quotes(&volume_dir),
    );
    let out = wsl
        .run(&["-d", distro_name, "-u", "root", "-e", "sh", "-c", &script])
        .await?;
    if !out.success {
        anyhow::bail!("failed to export volume '{clean_name}': {}", out.stderr.trim());
    }
    Ok(())
}

/// Imports a `.tar.gz` archive into `/var/lib/docker/volumes/<name>/_data`.
pub async fn import_volume<E: WslExecutor>(
    wsl: &E,
    distro_name: &str,
    volume_name: &str,
    src_windows_path: &str,
) -> anyhow::Result<()> {
    let clean_name = sanitize_path_segment(volume_name)?;
    let wsl_src = windows_path_to_wsl(src_windows_path.trim())
        .ok_or_else(|| anyhow::anyhow!("source must be an absolute Windows path"))?;

    let volume_dir = format!("/var/lib/docker/volumes/{clean_name}/_data");
    let script = format!(
        "mkdir -p '{}' && tar -xzf '{}' -C '{}'",
        escape_single_quotes(&volume_dir),
        escape_single_quotes(&wsl_src),
        escape_single_quotes(&volume_dir),
    );
    let out = wsl
        .run(&["-d", distro_name, "-u", "root", "-e", "sh", "-c", &script])
        .await?;
    if !out.success {
        anyhow::bail!("failed to import volume '{clean_name}': {}", out.stderr.trim());
    }
    Ok(())
}

fn escape_single_quotes(input: &str) -> String {
    input.replace('\'', "'\"'\"'")
}
