#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::process::Stdio;

#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::process::Command;

/// Handles a NanoDock multiplexed control/sync stream (`\x00NDK` magic already consumed).
#[cfg(unix)]
pub async fn handle_control_stream<S>(stream: S) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(());
    }
    let cmd = line.trim_end_matches(&['\r', '\n'][..]);

    if cmd == "PING" {
        reader.get_mut().write_all(b"PONG\n").await?;
        reader.get_mut().flush().await?;
        return Ok(());
    }

    if let Some(wsl_dir) = cmd.strip_prefix("TAR ") {
        let len = reader.read_u64().await?;
        const MAX_TAR_BYTES: u64 = 256 * 1024 * 1024;
        if len > MAX_TAR_BYTES {
            write_status(
                reader.get_mut(),
                Err(anyhow::anyhow!("tar payload exceeds 256 MiB limit")),
            )
            .await?;
            return Ok(());
        }
        let res = stream_tar_to_extract(wsl_dir, &mut reader, len).await;
        write_status(reader.get_mut(), res).await?;
        return Ok(());
    }

    if let Some(wsl_dir) = cmd.strip_prefix("DEL ") {
        let count = reader.read_u32().await?;
        if count > 100_000 {
            write_status(
                reader.get_mut(),
                Err(anyhow::anyhow!("delete batch count exceeds limit")),
            )
            .await?;
            return Ok(());
        }
        let mut paths = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let plen = reader.read_u32().await? as usize;
            let mut pbuf = vec![0u8; plen];
            reader.read_exact(&mut pbuf).await?;
            paths.push(String::from_utf8_lossy(&pbuf).into_owned());
        }
        let res = delete_rel_paths(wsl_dir, &paths).await;
        write_status(reader.get_mut(), res).await?;
        return Ok(());
    }

    write_status(
        reader.get_mut(),
        Err(anyhow::anyhow!("unknown control command: {cmd}")),
    )
    .await?;
    Ok(())
}

#[cfg(unix)]
async fn stream_tar_to_extract<R: AsyncRead + Unpin>(
    wsl_dir: &str,
    reader: &mut R,
    len: u64,
) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(wsl_dir)
        .await
        .with_context(|| format!("failed to create {wsl_dir}"))?;
    let mut child = Command::new("tar")
        .args(["-xf", "-", "-C", wsl_dir])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn tar")?;
    if let Some(mut stdin) = child.stdin.take() {
        let mut limited = reader.take(len);
        let copied = tokio::io::copy(&mut limited, &mut stdin).await?;
        stdin.shutdown().await?;
        if copied != len {
            let _ = child.kill().await;
            anyhow::bail!("unexpected EOF while streaming tar ({copied}/{len} bytes)");
        }
    }
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        anyhow::bail!(
            "tar extract failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(unix)]
async fn delete_rel_paths(wsl_dir: &str, rel_paths: &[String]) -> anyhow::Result<()> {
    let base = Path::new(wsl_dir);
    for rel in rel_paths {
        let rel_path = Path::new(rel);
        if rel_path.is_absolute()
            || rel_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_)))
        {
            anyhow::bail!("unsafe relative path in DEL: {rel}");
        }
        let target = base.join(rel_path);
        match tokio::fs::remove_file(&target).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err.into()),
        }
        // Clean up empty parent directories created for synced files, stopping before `base`.
        let mut parent = target.parent();
        while let Some(dir) = parent {
            if dir == base {
                break;
            }
            if tokio::fs::remove_dir(dir).await.is_err() {
                break;
            }
            parent = dir.parent();
        }
    }
    Ok(())
}

#[cfg(unix)]
async fn write_status<W: AsyncWrite + Unpin>(
    writer: &mut W,
    res: anyhow::Result<()>,
) -> anyhow::Result<()> {
    match res {
        Ok(()) => writer.write_all(b"OK\n").await?,
        Err(err) => {
            let msg = format!("ERR {}\n", err.to_string().replace('\n', " "));
            writer.write_all(msg.as_bytes()).await?;
        }
    }
    writer.flush().await?;
    Ok(())
}
