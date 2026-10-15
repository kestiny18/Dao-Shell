#Requires -Version 7.0
# Explicit build preparation only: never starts the worker, UAC, or a service.
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $workspace
try {
    & (Join-Path $PSScriptRoot 'dev.ps1') build --locked --release --bin dao-index-worker
    $source=Join-Path $workspace 'target/release/dao-index-worker.exe'
    $destination=Join-Path $workspace 'desktop/src-tauri/index-payload'
    [IO.Directory]::CreateDirectory($destination) | Out-Null
    $bytes=[IO.File]::ReadAllBytes($source)
    $digest=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes))
    [IO.File]::WriteAllBytes((Join-Path $destination 'dao-index-worker.exe'),$bytes)
    @{schema=1;sha256=$digest;bytes=$bytes.Length} | ConvertTo-Json | Set-Content -Encoding utf8NoBOM -LiteralPath (Join-Path $destination 'manifest.json')
    Write-Output "Prepared index payload SHA256 $digest"
} finally {Pop-Location}
