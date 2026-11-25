# ADR-002: Destroy WebView2 on Window Close

## Status
Accepted

## Context
Hiding a Tauri window keeps `msedgewebview2.exe` resident in memory (~150–300 MB RAM).

## Decision
Intercept `CloseRequested` on the dashboard `WebviewWindow` and call `window.destroy()`. Prevent application exit in `RunEvent::ExitRequested` (`api.prevent_exit()`) so the System Tray remains active at 0 MB WebView2 RAM. Re-create the window on demand from the tray menu.

## Consequences
- UI state must be fetched from `core-daemon` via `control_rpc` on window open rather than relying on long-lived frontend memory.
