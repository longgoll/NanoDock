# RootFS & Lightweight Init

## Alpine Micro-Distro (`rootfs-builder/`)
- Base: `alpine:latest` (musl libc).
- Packages: `docker`, `docker-cli-compose`, `containerd`, `iptables`, `ip6tables`, `ca-certificates`, `util-linux` (`fstrim`), plus statically linked `nanodock-agent`.
- Stripped caches (`/var/cache/apk/*`, `/usr/share/man/*`, `/usr/share/doc/*`) to keep `micro-distro.tar.gz` around 30–40 MB.

## `init.sh` (No systemd / OpenRC)
- Enforced LF (`\n`) line endings.
- Idempotent startup using lock/PID check.
- Mounts pseudo-filesystems (`/proc`, `/sys`, `/dev/pts`, `/sys/fs/cgroup` for cgroup v2) if not already mounted.
- Cleans stale `/mnt/wsl/nanodock/docker.sock` before starting `containerd` and `dockerd`.

## Dual Socket (`daemon.json`)
`dockerd` listens directly on both:
- `unix:///var/run/docker.sock` (for `nanodock-agent`)
- `unix:///mnt/wsl/nanodock/docker.sock` (shared across other WSL2 distros like Ubuntu and VS Code Dev Containers)
