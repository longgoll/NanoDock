# WSL Bridge & Yamux Transport

## `wsl-bridge` Responsibilities
- **UTF-16LE Decoding:** All `wsl.exe` stdout/stderr output is decoded via `String::from_utf16_lossy` with BOM and `\0` stripped.
- **First-Run Bootstrap:** Checks `wsl.exe --status`, imports `micro-distro.tar.gz --version 2`, and runs `wsl.exe --manage <distro> --set-sparse true --allow-unsafe` (with fallback without `--allow-unsafe`).
- **Safe `.wslconfig` Management:** Backs up to `.wslconfig.bak`, preserves user keys, calculates `memory` as 50% of physical RAM (clamped 2 GB–8 GB), keeps `swap=4GB`, and gates Win11-only keys (`networkingMode=mirrored`, `dnsTunneling=true`, `autoMemoryReclaim=gradual`) behind OS/WSL version checks.
- **Credential Store Check:** Detects `"credsStore": "desktop"` in `%USERPROFILE%\.docker\config.json` and warns/assists if Docker Desktop was previously installed.

## `StreamTransport` & `YamuxStdioTransport`
- Avoids spawning `wsl.exe` per HTTP request.
- Opens a single `wsl.exe -d <distro> -e /usr/local/bin/nanodock-agent` process and runs `yamux` multiplexing over raw `stdin`/`stdout`.
- Abstracted behind `trait StreamTransport` so `AF_HYPERV` can be added later without modifying the proxy layer.
