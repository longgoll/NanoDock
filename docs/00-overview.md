# NanoDock — Architecture Overview

NanoDock is an ultra-lightweight Docker Desktop alternative for Windows 10/11 built on a custom Alpine WSL2 micro-distro, a Rust smart proxy daemon, and a zero-background-RAM Tauri v2 + Svelte 5 UI.

## Core Goals
- **Zero-Idle Footprint:** 0 MB RAM and 0.0% CPU for the Linux VM when no containers are running.
- **Ultra-Light Overhead:** Background daemon < 20 MB RAM; WebView2 destroyed on window close (0 MB UI RAM in tray).
- **100% Compatibility:** Native `docker` CLI, `docker compose`, Testcontainers, VS Code Dev Containers, and `/dev/dxg` GPU passthrough.
- **Anti-Bloat & Anti-OOM:** Per-distro `sparseVhd` + event-driven `fstrim -av`, plus safe `swap` & `autoMemoryReclaim`.

## Data Flow
```text
[Docker CLI / IDE / Compose / Testcontainers]
       │ (\\.\pipe\docker_engine)
       ▼
[core-daemon (Windows Background Process)]
       ├── wsl-bridge        ──► Bootstrap, .wslconfig, sparseVhd, UTF-16LE wsl.exe
       ├── pipe_server       ──► Concurrent Named Pipe instances + conflict check
       ├── proxy             ──► Path translation (C:\ -> /mnt/c/) + HTTP stream
       ├── transport (yamux) ──► Single persistent wsl.exe stdio -> guest-agent
       ├── lifecycle         ──► Auto-Wake (~0.5s) & Graceful Auto-Hibernate
       ├── watchers          ──► Docker /events -> 5s debounce -> fstrim -av
       └── control_rpc       ──► \\.\pipe\nanodock_ctl (Tauri UI / CLI)
       │
       ▼
[WSL2 Micro-Distro (Alpine ~35 MB)]
       ├── guest-agent       ──► yamux server -> /var/run/docker.sock
       ├── containerd + dockerd (no systemd/OpenRC)
       └── Dual socket       ──► /var/run/docker.sock & /mnt/wsl/nanodock/docker.sock
```

## Documentation Index
- [Daemon & Control IPC](01-architecture/daemon-and-ipc.md)
- [WSL Bridge & Yamux Transport](01-architecture/wsl-bridge-and-yamux.md)
- [Windows Path Translation](01-architecture/path-translation.md)
- [RootFS & Init](01-architecture/rootfs-and-init.md)
- [Auto-Wake & Hibernate](02-features/auto-hibernate.md)
- [Disk Reclamation](02-features/disk-reclaim.md)
- [Coding Rules](04-guides/coding-rules.md)
- [Dev & Build Guide](04-guides/dev-and-build.md)
