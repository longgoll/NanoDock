# Development & Build Guide

## Prerequisites (Windows)
- Rust toolchain (`rustup` with `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-musl` for `guest-agent`)
- Node.js + pnpm (for Tauri v2 + Svelte 5 frontend)
- WSL2 enabled on Windows 10/11

## Workspace Commands
```powershell
cargo check --workspace
cargo test --workspace
```

## Building the Alpine Micro-Distro (`micro-distro.tar.gz`)
Requires an existing container builder (`docker` or compatible) to run the multi-stage Alpine build:
```powershell
.\rootfs-builder\build.ps1
```
Output is written to `rootfs-builder/out/micro-distro.tar.gz`.
