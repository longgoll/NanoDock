mod control;
mod server;
mod stdio;
mod vsock;

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(args.first().map(String::as_str), Some("--version" | "-V")) {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if matches!(args.first().map(String::as_str), Some("--vsock")) {
        let port = args
            .get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .or_else(|| {
                std::env::var("NANODOCK_VSOCK_PORT")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
            })
            .unwrap_or(47000);
        let socket_path = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "/var/run/docker.sock".to_string());
        return vsock::run_vsock_server(port, &socket_path).await;
    }

    let socket_path = args
        .first()
        .cloned()
        .unwrap_or_else(|| "/var/run/docker.sock".to_string());

    server::run_yamux_server(stdio::StdioStream::new(), &socket_path).await
}

#[cfg(not(unix))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("guest-agent is a Linux WSL2 binary and must be compiled for a Unix target");
}
