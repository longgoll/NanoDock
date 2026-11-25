<#
.SYNOPSIS
    Builds the NanoDock Alpine WSL2 micro-distro tarball (micro-distro.tar.gz).
#>
$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$OutDir = Join-Path $PSScriptRoot "out"
$TarPath = Join-Path $OutDir "micro-distro.tar"
$GzPath = Join-Path $OutDir "micro-distro.tar.gz"
$ImageTag = "nanodock-rootfs-builder:latest"
$ContainerName = "nanodock-rootfs-export"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Write-Host "==> Building rootfs image..." -ForegroundColor Cyan
docker build -f (Join-Path $PSScriptRoot "Dockerfile.rootfs") -t $ImageTag $RepoRoot
if ($LASTEXITCODE -ne 0) {
    throw "Container build failed (exit code $LASTEXITCODE). Ensure Docker or Podman machine is running."
}

Write-Host "==> Exporting container filesystem..." -ForegroundColor Cyan
docker rm -f $ContainerName 2>$null | Out-Null
docker create --name $ContainerName $ImageTag /bin/sh | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create export container (exit code $LASTEXITCODE)."
}

try {
    docker export -o $TarPath $ContainerName
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to export container filesystem (exit code $LASTEXITCODE)."
    }
} finally {
    docker rm -f $ContainerName 2>$null | Out-Null
}

Write-Host "==> Compressing to micro-distro.tar.gz..." -ForegroundColor Cyan
if (Get-Command gzip -ErrorAction SilentlyContinue) {
    gzip -f -9 $TarPath
} else {
    # Fallback .NET GZipStream if gzip.exe is not in PATH
    $inStream = [System.IO.File]::OpenRead($TarPath)
    $outStream = [System.IO.File]::Create($GzPath)
    $gzipStream = New-Object System.IO.Compression.GZipStream($outStream, [System.IO.Compression.CompressionLevel]::Optimal)
    try {
        $inStream.CopyTo($gzipStream)
    } finally {
        $gzipStream.Dispose()
        $outStream.Dispose()
        $inStream.Dispose()
    }
    Remove-Item $TarPath -Force
}

$sizeMb = [math]::Round((Get-Item $GzPath).Length / 1MB, 2)
Write-Host "==> Done: $GzPath ($sizeMb MB)" -ForegroundColor Green
