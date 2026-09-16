param(
    [Parameter(Position = 0)][string]$Command = 'run',
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs
)
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$previousPath = $env:Path
$previousDirectory = Get-Location
try {
    $rustBin = Join-Path $env:USERPROFILE '.cargo\bin'
    $compilerBin = Join-Path $workspace '.tools\w64devkit\bin'
    $env:Path = "$rustBin;$compilerBin;$previousPath"
    Set-Location -LiteralPath $workspace
    & cargo $Command @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo $Command failed ($LASTEXITCODE)" }
} finally {
    $env:Path = $previousPath
    Set-Location -LiteralPath $previousDirectory
}
