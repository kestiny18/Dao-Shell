param(
    [ValidateSet('build','run','check','test','clippy','fmt')][string]$Command = 'run',
    [string[]]$CargoArgs = @()
)
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$previousPath = $env:Path
try {
    $env:Path = "$(Join-Path $env:USERPROFILE '.cargo\bin');$(Join-Path $workspace '.tools\w64devkit\bin');$previousPath"
    [string[]]$lockedArgs = if ($Command -eq 'fmt') { @() } else { @('--locked') }
    & cargo $Command --manifest-path (Join-Path $workspace 'desktop\src-tauri\Cargo.toml') @lockedArgs @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Desktop $Command failed ($LASTEXITCODE)" }
} finally { $env:Path = $previousPath }
