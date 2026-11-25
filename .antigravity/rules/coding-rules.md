# NanoDock Coding & Architecture Rules

These rules apply to all contributors and AI coding agents working on NanoDock.

## 1. File Size & Single Responsibility (SRP)
- Keep every source file under **250–300 lines**. If a file grows beyond this, split it into focused submodules inside a folder named after the module.
- `main.rs`, `lib.rs`, and `mod.rs` are strictly for **module declarations, re-exports, and dependency wiring**. Do not place business logic in them.
- One file = one clear responsibility.

## 2. Trait-First at OS & I/O Boundaries
- All interactions with `wsl.exe`, Windows Named Pipes, filesystem config files (`.wslconfig`), and transport streams must sit behind a Rust `trait` (e.g., `WslExecutor`, `StreamTransport`, `DockerEventHook`).
- Parsing and transformation logic (`decode_wsl_output`, `.wslconfig` merger, `PathTranslator`) must have unit tests that run without requiring a live WSL2 VM.

## 3. Error Handling & Safety
- Library crates (`shared-types`, `wsl-bridge`) must define typed errors using `thiserror`.
- Binary crates (`core-daemon`, `guest-agent`) may use `anyhow` at the top-level entrypoint.
- **Never** call `.unwrap()` or `.expect()` in daemon runtime request/event loops.

## 4. Async Lifecycle & Cleanup
- Every background task (`tokio::spawn`) in `core-daemon` must accept a `CancellationToken` (or shutdown receiver) so it terminates cleanly on hibernate or app exit.
- Avoid spawning `wsl.exe` per HTTP request; all Docker API traffic must go through the multiplexed `StreamTransport`.

## 5. Line Endings & Documentation
- All files under `rootfs-builder/rootfs/` must use **LF (`\n`)** line endings (enforced by `.gitattributes` and verified in `Dockerfile.rootfs`).
- Architectural changes must be documented in the corresponding file under `docs/`, never dumped into a single monolithic file.
