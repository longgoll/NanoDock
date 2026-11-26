# Daemon & Control IPC

## Windows Named Pipe Server (`\\.\pipe\docker_engine`)
- Listens for standard Docker Engine API requests.
- **Concurrency:** Immediately pre-allocates a fresh `NamedPipeServer` instance after accepting a connection to avoid `ERROR_PIPE_BUSY` under parallel `docker compose` / Testcontainers loads.
- **Conflict Detection:** On startup, checks if `com.docker.service` or an existing `\\.\pipe\docker_engine` owner is active and emits a clear diagnostic error.

## Control RPC Pipe (`\\.\pipe\nanodock_ctl`)
- Dedicated newline-delimited JSON-RPC pipe between `core-daemon` and `src-tauri` / `nanodock` CLI (`PIPE_REJECT_REMOTE_CLIENTS` enabled).
- Schema lives in `crates/shared-types/src/ipc/` (`models.rs` and `protocol.rs`).
- Supports engine lifecycle, `.wslconfig` & startup management, container exec/files/logs/stats/inspect, image layer history, volume `.tar.gz` export/import, WSL2 distro `DOCKER_HOST` integration, runtime savings metrics, and `core-daemon`/`dockerd` log tailing.
- Allows the Tauri WebView2 window to be completely destroyed without interrupting the daemon.
