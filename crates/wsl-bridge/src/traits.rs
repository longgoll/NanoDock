use async_trait::async_trait;

use crate::error::WslError;

/// Decoded output of a `wsl.exe` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Abstraction over `wsl.exe` execution for unit testing without a live WSL2 VM.
#[async_trait]
pub trait WslExecutor: Send + Sync {
    async fn run(&self, args: &[&str]) -> Result<CommandOutput, WslError>;
}
