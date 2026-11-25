# Coding Rules Reference

The canonical coding rules for AI agents and contributors are maintained in:
- [.antigravity/rules/coding-rules.md](../../.antigravity/rules/coding-rules.md)

## Summary
- Max ~250–300 lines per source file; split into submodules early.
- `main.rs`, `lib.rs`, `mod.rs` only wire modules and re-export.
- Trait-first boundaries for `wsl.exe`, Named Pipes, and transports.
- Typed errors with `thiserror` in libs; no `.unwrap()`/`.expect()` in daemon loops.
- `LF` line endings enforced for `rootfs-builder/rootfs/**`.
