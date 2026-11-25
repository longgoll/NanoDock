# Windows Path Translation

## Why It Is Required
Windows Docker CLI, Compose, and Testcontainers send host paths such as `C:\Users\dev\app` or `D:\data` in bind-mount payloads. Linux `dockerd` inside WSL2 expects `/mnt/c/Users/dev/app` or `/mnt/d/data`.

## Proxy Middleware (`PathTranslator`)
- Intercepts JSON bodies on `POST /containers/create` (and versioned `/v*/containers/create`).
- Rewrites `HostConfig.Binds` (`C:\path:/container/path[:opts]`) and `HostConfig.Mounts` (`Source` when `Type == "bind"`).
- Normalizes backslashes to forward slashes and lowercases the drive letter:
  - `C:\foo\bar` -> `/mnt/c/foo/bar`
  - `c:/foo/bar` -> `/mnt/c/foo/bar`
- Leaves named volumes and already-Linux paths (`/mnt/...`, `/var/...`) untouched.
