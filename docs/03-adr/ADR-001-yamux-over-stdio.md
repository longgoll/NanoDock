# ADR-001: Yamux over `wsl.exe` Stdio vs `AF_HYPERV`

## Status
Accepted

## Context
Spawning `wsl.exe` per HTTP request fails under `docker compose` and Testcontainers concurrency. Direct `AF_HYPERV` (`vsock`) requires resolving the WSL2 utility VM GUID from HCS/Registry and can hit permission or Windows-version edge cases.

## Decision
Use a single persistent `wsl.exe -d <distro> -e /usr/local/bin/nanodock-agent` process with `yamux` multiplexing over raw `stdin`/`stdout`, abstracted behind `trait StreamTransport`.

## Consequences
- Zero Administrator privileges required.
- Clean abstraction allows adding `AfHypervTransport` later without touching proxy or lifecycle code.
