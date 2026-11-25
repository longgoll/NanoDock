<#
.SYNOPSIS
    1-command developer launcher for NanoDock (WSL2 bootstrap + core-daemon + Vite + Tauri UI).
#>
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$DistroName = "nanodock"
$DistroDir = Join-Path $env:LOCALAPPDATA "NanoDock\distro"
$RootfsGz = Join-Path $PSScriptRoot "rootfs-builder\out\micro-distro.tar.gz"

Write-Host "==> Checking WSL2 distro '$DistroName'..." -ForegroundColor Cyan
$distros = wsl --list --quiet 2>$null | ForEach-Object { $_.Trim() -replace "`0", "" } | Where-Object { $_ }
if ($distros -notcontains $DistroName) {
    if (-not (Test-Path $RootfsGz)) {
        Write-Host "==> Building micro-distro.tar.gz..." -ForegroundColor Yellow
        & (Join-Path $PSScriptRoot "rootfs-builder\build.ps1")
    }
    New-Item -ItemType Directory -Force -Path $DistroDir | Out-Null
    Write-Host "==> Importing '$DistroName' into $DistroDir..." -ForegroundColor Cyan
    wsl --import $DistroName $DistroDir $RootfsGz --version 2
    if ($LASTEXITCODE -ne 0) {
        throw "wsl --import failed (exit code $LASTEXITCODE)."
    }
} else {
    Write-Host "    Distro '$DistroName' already installed." -ForegroundColor DarkGray
}

$daemonProc = $null
$viteProc = $null

try {
    Write-Host "==> Starting core-daemon..." -ForegroundColor Cyan
    $daemonProc = Start-Process -FilePath "cargo" -ArgumentList "run", "-p", "core-daemon" -PassThru -NoNewWindow

    Write-Host "==> Starting Vite dev server (http://localhost:1420)..." -ForegroundColor Cyan
    $viteProc = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", "pnpm dev" -PassThru -NoNewWindow

    # Wait briefly for Vite dev server to be ready before launching Tauri UI.
    for ($i = 0; $i -lt 30; $i++) {
        try {
            $resp = Invoke-WebRequest -Uri "http://localhost:1420" -UseBasicParsing -TimeoutSec 1 -ErrorAction Stop
            if ($resp.StatusCode -eq 200) { break }
        } catch {
            Start-Sleep -Milliseconds 500
        }
    }

    Write-Host "==> Launching NanoDock UI (check System Tray if window starts minimized)..." -ForegroundColor Green
    cargo run --manifest-path src-tauri/Cargo.toml
} finally {
    Write-Host "`n==> Shutting down background processes..." -ForegroundColor Yellow
    if ($viteProc -and -not $viteProc.HasExited) {
        & taskkill /PID $viteProc.Id /T /F 2>$null | Out-Null
    }
    if ($daemonProc -and -not $daemonProc.HasExited) {
        & taskkill /PID $daemonProc.Id /T /F 2>$null | Out-Null
    }
}
