# Changelog

All notable changes to NanoDock will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
