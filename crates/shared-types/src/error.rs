use thiserror::Error;

/// Common error types shared across NanoDock crates and IPC boundaries.
#[derive(Debug, Error)]
pub enum SharedError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("daemon unavailable: {0}")]
    DaemonUnavailable(String),

    #[error("conflict detected: {0}")]
    Conflict(String),
}
