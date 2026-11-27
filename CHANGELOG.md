# Changelog

All notable changes to NanoDock will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.0] - 2026-11-27

### Added
- **Event-Driven UI & Tray Push (`SubscribeEvents`):** Added `UiEventHook` and `ControlRequest::SubscribeEvents` in `core-daemon`, plus `event_bridge` in Tauri (`nanodock://daemon-event`), enabling instant UI & System Tray updates on engine state and Docker resource changes.
- **Interactive PTY Terminal (`xterm.js`):** Upgraded `ContainerTerminalTab` to a full interactive `@xterm/xterm` + `@xterm/addon-fit` terminal backed by hijacked `Tty: true` Docker exec sessions (`StartExecSession`, `Input`, `Resize`, `Close`).
- **Real-Time Container Logs & Stats Streaming:** Added `StreamContainerLogs` and `StreamContainerStats` streaming over Tauri Channels in `ContainerDetailView`, replacing 2s polling.
- **Guest-Agent Version Check:** Added `--version` flag to `guest-agent` and startup compatibility verification on `StartEngine` with `DaemonEvent::Warning` notifications.
- **Frontend Unit Tests:** Added `vitest` and unit tests for formatting utilities (`pnpm test`).

### Changed
- **Streaming Container Archive Reader:** Replaced full-buffer archive reads in `list_container_files` and `read_container_file` with O(1)-memory streaming tar parsing in `container_files.rs`.

## [0.7.1] - 2026-11-26

### Fixed
- **Auto-Wake State Sync:** Sync `EngineState::Running` and emit `DaemonEvent::StateChanged` when `TransportManager` wakes the WSL2 distro on incoming Docker Named Pipe traffic, ensuring Auto-Hibernate and Docker event watchers activate properly.
- **Engine Logs Path & Stop Error State:** Fixed `GetEngineLogs` reading `/var/log/nanodock/dockerd.log` (matching `init.sh`), and transitioned `StopEngine` to `EngineState::Error` if `graceful_hibernate` fails.
- **WSL2 Process & Distro Safety:** Added `CREATE_NO_WINDOW` when spawning `wsl.exe` in `SystemWslExecutor` and `YamuxStdioTransport`, and skipped inspecting stopped WSL2 distros in `list_user_wsl_distros` to avoid booting idle distributions.
- **IPC Resilience & HTTP Timeout:** Added `ERROR_PIPE_BUSY` retry in Tauri `daemon_client`, a 300s timeout on `docker_http_json_request`, and allowed `.` in container names for `open_container_shell`.
- **UI Polling Optimization:** Removed `updaterStore.checkForUpdates` and `environmentStore.refresh` from the 5-second `syncAll` loop in `App.svelte` so only engine/resource state polls every 5 seconds.
- **Chunked Proxy Translation & Modularization:** Added `Transfer-Encoding: chunked` support for Windows bind-mount path translation in `POST /containers/create`, coalesced concurrent `/containers/json?all=1` lookups across `list_images` and `list_volumes` with a 2s TTL cache, and split `docker_api` into focused submodules.

## [0.7.0] - 2026-11-26

### Added
- **CLI Diagnostics, Logs & Completions:** Added `nanodock doctor`, `nanodock logs [lines]`, and `nanodock completions <powershell|bash|zsh>` in `nanodock-cli`.
- **Container Terminal & File Browser:** Added interactive **Terminal** (`ExecContainer`) and **Files** (`ListContainerFiles`, `ReadContainerFile`) tabs in `ContainerDetailView`.
- **Compose Stack Logs & Down + Quick Templates:** Added inline **Stack Logs** and **Down** actions on Compose project headers, plus quick container templates (Postgres, Redis, MySQL, Nginx, Mailpit) in `RunContainerModal`.
- **Image Layer Inspector & Volume Archive Backup:** Added **Image Layers** modal (`InspectImageLayers`) in `ImagesView` and `.tar.gz` **Export / Import** (`ExportVolume`, `ImportVolume`) in `VolumesView`.
- **WSL2 Distro Integration, Resource Profiles & Diagnostics UI:** Added 1-click WSL2 distro `DOCKER_HOST` integration, *Eco / Balanced / Performance* resource presets, `wincred` helper option, `core-daemon.log` + `dockerd.log` viewer, and resource-saving metrics (`hibernate_count`, `total_reclaimed_bytes`) on the Dashboard.

## [0.6.0] - 2026-11-26

### Added
- **Smart First-Run CLI Setup:** Automatically adds NanoDock to Windows User `PATH` and creates a `docker.exe` shim (via `nanodock-cli`) on first launch when no external `docker.exe` is installed.
- **System & CLI Integration Settings:** Added a new Settings card and startup alert banner with 1-click **Enable WSL2** (`wsl --install --no-distribution` via UAC), **User `PATH`** toggle, **`docker.exe` shim** toggle, and 1-click **`credsStore: "desktop"`** auto-fix (with `config.json.bak` backup).

## [0.5.0] - 2026-11-26

### Added
- **RootFS BuildKit Support:** Added `docker-cli-buildx` to `rootfs-builder/Dockerfile.rootfs` for modern `docker build` and `docker compose build`.
- **Portable & Installer Packaging:** Updated `bundle.ps1` to include `nanodock.exe` (`nanodock-cli`) and generate `NanoDock-portable-windows-x64.zip`; configured Tauri `nsis` installer with `installMode: "currentUser"` (no Admin required).
- **CI/CD & Automated Releases:** Added `cargo clippy --workspace --all-targets -- -D clippy::all` to `.github/workflows/ci.yml` and created `.github/workflows/release.yml` for tag-triggered (`v*`) RootFS, portable ZIP, and NSIS installer releases.

### Security
- Enforced `reject_remote_clients(true)` (`PIPE_REJECT_REMOTE_CLIENTS`) on `\\.\pipe\docker_engine` and `\\.\pipe\nanodock_ctl` Named Pipe servers.

## [0.4.0] - 2026-11-26

### Added
- **Docker Networks Management:** Added `NetworkSummary` and network RPCs (`ListNetworks`, `CreateNetwork`, `RemoveNetwork`, `PruneNetworks`) in `core-daemon`, Tauri commands, and a new `NetworksView` tab with English/Vietnamese i18n.
- **Compose Stack Quick Actions:** Added inline Start, Stop, and Restart buttons on Compose project group headers in `ContainersView`.
- **Dynamic System Tray:** Tray icon tooltip and Start/Stop menu items now sync every 5s with live `EngineState` and active container count.

## [0.3.0] - 2026-11-26

### Added
- Persist `EngineConfig` to `%LOCALAPPDATA%\NanoDock\config.json` and sync `%USERPROFILE%\.wslconfig` on daemon startup and config updates, with automatic Windows 10/11 build detection (`mirrored_networking` / `autoMemoryReclaim`).
- Implemented `auto_start_on_login` registration in Windows Registry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) via `StartupManager`.
- Added `ControlRequest::ShutdownDaemon` RPC so quitting the Tauri tray app gracefully hibernates WSL2 and stops `core-daemon` before falling back to process termination.

### Fixed
- Explicitly set `networkingMode=nat` in `.wslconfig` when `mirrored_networking` is disabled on Windows 11.

## [0.2.0] - 2026-11-26

### Added
- Display the current application version dynamically in the sidebar header (`@tauri-apps/api/app` `getVersion()`).
- Added `.antigravity/rules/versioning-rules.md` defining Semantic Versioning and manifest sync rules for AI coding agents and contributors.

## [0.1.0] - 2026-11-25

### Added
- Initial NanoDock desktop app (Tauri + Svelte + Rust WSL2 Docker engine daemon).
