# Daemon & Control IPC

## Windows Named Pipe Server (`\\.\pipe\docker_engine`)
- Listens for standard Docker Engine API requests.
- **Concurrency:** Immediately pre-allocates a fresh `NamedPipeServer` instance after accepting a connection to avoid `ERROR_PIPE_BUSY` under parallel `docker compose` / Testcontainers loads.
- **Conflict Detection:** On startup, checks if `com.docker.service` or an existing `\\.\pipe\docker_engine` owner is active and emits a clear diagnostic error.

## Control RPC Pipe (`\\.\pipe\nanodock_ctl`)
- Dedicated JSON-RPC / length-prefixed JSON pipe between `core-daemon` and `src-tauri` (or future CLI).
- Schema lives in `crates/shared-types/src/ipc.rs`.
- Allows the Tauri WebView2 window to be completely destroyed without interrupting the daemon.
