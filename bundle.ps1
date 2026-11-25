<#
.SYNOPSIS
    Builds a self-contained portable NanoDock release bundle in .\dist-bundle\
    (NanoDock.exe + core-daemon.exe + micro-distro.tar.gz).
#>
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$BundleDir = Join-Path $PSScriptRoot "dist-bundle"
$RootfsGz = Join-Path $PSScriptRoot "rootfs-builder\out\micro-distro.tar.gz"

if (-not (Test-Path $RootfsGz)) {
    Write-Host "==> Building micro-distro.tar.gz..." -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot "rootfs-builder\build.ps1")
}

Write-Host "==> Building frontend (pnpm build)..." -ForegroundColor Cyan
pnpm build
if ($LASTEXITCODE -ne 0) { throw "pnpm build failed." }

Write-Host "==> Building core-daemon & nanodock-cli (release)..." -ForegroundColor Cyan
cargo build --release -p core-daemon -p nanodock-cli
if ($LASTEXITCODE -ne 0) { throw "Rust daemon/CLI release build failed." }

Write-Host "==> Building NanoDock Tauri UI (release)..." -ForegroundColor Cyan
cargo build --release --manifest-path src-tauri/Cargo.toml
if ($LASTEXITCODE -ne 0) { throw "src-tauri release build failed." }

Write-Host "==> Assembling portable bundle in $BundleDir..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $BundleDir | Out-Null
Copy-Item "src-tauri\target\release\nanodock-ui.exe" (Join-Path $BundleDir "NanoDock.exe") -Force
Copy-Item "target\release\core-daemon.exe" (Join-Path $BundleDir "core-daemon.exe") -Force
Copy-Item "target\release\nanodock.exe" (Join-Path $BundleDir "nanodock.exe") -Force
Copy-Item $RootfsGz (Join-Path $BundleDir "micro-distro.tar.gz") -Force

$wv2 = "src-tauri\target\release\WebView2Loader.dll"
if (Test-Path $wv2) {
    Copy-Item $wv2 (Join-Path $BundleDir "WebView2Loader.dll") -Force
}

$ZipPath = Join-Path $PSScriptRoot "NanoDock-portable-windows-x64.zip"
Compress-Archive -Path (Join-Path $BundleDir "*") -DestinationPath $ZipPath -Force

Write-Host "==> Done! Portable folder: $BundleDir | Archive: $ZipPath" -ForegroundColor Green
