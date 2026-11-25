# Disk Reclamation (`sparseVhd` + `fstrim`)

## Proactive Prevention
- Distro is marked sparse immediately after import: `wsl.exe --manage <distro> --set-sparse true --allow-unsafe`.
- `.wslconfig` sets `sparseVhd=true` when supported by the installed WSL version.

## Reactive Event-Driven Trim
- `watchers/disk_reclaim.rs` subscribes to Docker `GET /events` via `trait DockerEventHook`.
- On `destroy` (container), `delete` (image), or `prune` (volume/system/builder):
  - Resets a **5-second debounce timer**.
  - Executes `wsl.exe -d <distro> fstrim -av` once the burst of deletions settles, immediately returning freed blocks in `.vhdx` to Windows NTFS.
