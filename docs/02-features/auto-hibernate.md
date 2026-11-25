# Auto-Wake & Graceful Auto-Hibernate

## Auto-Wake (< 1s)
- Triggered on the first incoming connection to `\\.\pipe\docker_engine` when the distro is `Stopped`.
- Launches `wsl.exe -d <distro> -e /init.sh`, polls `/var/run/docker.sock` readiness via `nanodock-agent`, and begins multiplexing streams.

## Graceful Auto-Hibernate (0 MB Idle RAM)
- Polls running containers every 30s (`GET /containers/json?filters={"status":["running"]}`).
- Tracks active API/exec/logs streams.
- When `running_containers == 0` and active streams == 0 continuously for `idle_timeout_secs` (default `180s`):
  1. Closes the `yamux` transport cleanly.
  2. Sends `SIGTERM` to `dockerd` and `containerd` inside the distro (waits up to 5s) to prevent `overlay2` metadata corruption.
  3. Runs `wsl.exe --terminate <distro>`.
