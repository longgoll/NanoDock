use thiserror::Error;

/// Typed errors for WSL2 discovery, bootstrap, configuration, and lifecycle operations.
#[derive(Debug, Error)]
pub enum WslError {
    #[error("WSL2 command failed (exit {code:?}): {stderr}")]
    CommandFailed { code: Option<i32>, stderr: String },

    #[error("WSL2 is not enabled; installation was triggered and a Windows reboot is required")]
    RebootRequired,

    #[error("failed to execute wsl.exe: {0}")]
    Io(#[from] std::io::Error),

    #[error("rootfs tarball not found at {0}")]
    RootfsMissing(String),

    #[error("failed to parse configuration: {0}")]
    Config(String),
}
