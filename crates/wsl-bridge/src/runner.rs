use async_trait::async_trait;
use tokio::process::Command;

use crate::error::WslError;
use crate::traits::{CommandOutput, WslExecutor};

/// Production `WslExecutor` that invokes Windows `wsl.exe` and decodes UTF-16LE output.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemWslExecutor;

#[async_trait]
impl WslExecutor for SystemWslExecutor {
    async fn run(&self, args: &[&str]) -> Result<CommandOutput, WslError> {
        let mut cmd = Command::new("wsl.exe");
        cmd.args(args);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let output = cmd.output().await?;
        Ok(CommandOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: decode_wsl_output(&output.stdout),
            stderr: decode_wsl_output(&output.stderr),
        })
    }
}

/// Decodes `wsl.exe` stdout/stderr bytes.
///
/// Windows `wsl.exe` typically emits UTF-16LE (often with BOM and `\0` padding),
/// though some subcommands (e.g., Linux distro stdout) emit UTF-8.
pub fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let looks_utf16 = bytes.starts_with(&[0xFF, 0xFE])
        || (bytes.len() >= 2
            && bytes.len().is_multiple_of(2)
            && bytes.iter().skip(1).step_by(2).any(|&b| b == 0));

    if looks_utf16 {
        let start = if bytes.starts_with(&[0xFF, 0xFE]) { 2 } else { 0 };
        let u16_words: Vec<u16> = bytes[start..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        String::from_utf16_lossy(&u16_words)
            .replace('\0', "")
            .trim()
            .to_string()
    } else {
        String::from_utf8_lossy(bytes)
            .replace('\0', "")
            .trim()
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::decode_wsl_output;

    #[test]
    fn decodes_utf16le_with_bom_and_nulls() {
        let mut raw = vec![0xFF, 0xFE];
        for ch in "nanodock\0\r\n".encode_utf16() {
            raw.extend_from_slice(&ch.to_le_bytes());
        }
        assert_eq!(decode_wsl_output(&raw), "nanodock");
    }

    #[test]
    fn decodes_utf16le_without_bom() {
        let mut raw = Vec::new();
        for ch in "Ubuntu\r\nnanodock\r\n".encode_utf16() {
            raw.extend_from_slice(&ch.to_le_bytes());
        }
        assert_eq!(decode_wsl_output(&raw), "Ubuntu\r\nnanodock");
    }

    #[test]
    fn decodes_plain_utf8_linux_output() {
        let raw = b"/: 1.2 GiB trimmed\n";
        assert_eq!(decode_wsl_output(raw), "/: 1.2 GiB trimmed");
    }
}
