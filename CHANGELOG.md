# Changelog

All notable changes to NanoDock will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
