mod server;
mod stdio;

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let socket_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/var/run/docker.sock".to_string());

    server::run_yamux_server(stdio::StdioStream::new(), &socket_path).await
}

#[cfg(not(unix))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("guest-agent is a Linux WSL2 binary and must be compiled for a Unix target");
}
