#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
& (Join-Path $PSScriptRoot 'dev.ps1') build --release --locked
$metadata = (& (Join-Path $PSScriptRoot 'dev.ps1') metadata --locked --format-version 1 | ConvertFrom-Json -AsHashtable)
$project = $metadata.packages | Where-Object { $_.name -eq 'dao-shell' -and -not $_.source } | Select-Object -First 1
if (-not $project) { throw 'Project metadata not found' }
$version = $project.version
$package = Join-Path $workspace "dist\daosh-$version-windows-x64"
New-Item -ItemType Directory -Force -Path $package | Out-Null
$legacyReadme = Join-Path $package 'README.en.md'
if (Test-Path -LiteralPath $legacyReadme) { Remove-Item -LiteralPath $legacyReadme }
Copy-Item -LiteralPath (Join-Path $workspace 'target\release\daosh.exe') -Destination $package
Copy-Item -LiteralPath (Join-Path $workspace 'README.md') -Destination $package
foreach ($name in @('README.zh-CN.md', 'ROADMAP.md', 'CHANGELOG.md', 'CONTRIBUTING.md', 'LICENSE')) {
    $document = Join-Path $workspace $name
    if (Test-Path -LiteralPath $document) { Copy-Item -LiteralPath $document -Destination $package }
}
Copy-Item -LiteralPath (Join-Path $workspace 'docs\IMPLEMENTATION.md') -Destination $package
$packageDocs = Join-Path $package 'docs'
New-Item -ItemType Directory -Force -Path $packageDocs | Out-Null
Copy-Item -Path (Join-Path $workspace 'docs\*') -Destination $packageDocs -Recurse -Force
Copy-Item -LiteralPath (Join-Path $workspace 'rust-toolchain.toml') -Destination $package
$notices = Join-Path $package 'third-party'
New-Item -ItemType Directory -Force -Path $notices | Out-Null
foreach ($dependency in $metadata.packages) {
    if (-not $dependency.source) { continue }
    $source = Split-Path -Parent $dependency.manifest_path
    $destination = Join-Path $notices "$($dependency.name)-$($dependency.version)"
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    "$($dependency.name) $($dependency.version)`n$($dependency.license)`n$($dependency.repository)" | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $destination 'source.txt')
    Get-ChildItem -LiteralPath $source -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE)' } | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $destination
    }
}
$mingwNotice = Join-Path $workspace '.tools\w64devkit\COPYING.MinGW-w64-runtime.txt'
if (Test-Path -LiteralPath $mingwNotice) { Copy-Item -LiteralPath $mingwNotice -Destination $notices }
$rustPrefix = Join-Path $env:USERPROFILE '.rustup\toolchains'
Get-ChildItem -LiteralPath $rustPrefix -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like '1.98.1-*' } | ForEach-Object {
    foreach ($name in @('COPYRIGHT.html', 'COPYRIGHT-library.html')) {
        $rustLicense = Join-Path $_.FullName "share\doc\rust\$name"
        if (Test-Path -LiteralPath $rustLicense) { Copy-Item -LiteralPath $rustLicense -Destination (Join-Path $notices "rust-$($_.Name)-$name") }
    }
}
$archive = "$package.zip"
Get-ChildItem -LiteralPath $package -Recurse -File | Where-Object { $_.LastWriteTime.Year -lt 1980 } | ForEach-Object { $_.LastWriteTime = [datetime]'1980-01-01' }
Compress-Archive -Path (Join-Path $package '*') -DestinationPath $archive -Force
$hash = Get-FileHash -LiteralPath $archive -Algorithm SHA256
"$($hash.Hash.ToLower())  $([System.IO.Path]::GetFileName($archive))" | Set-Content -Encoding ASCII -LiteralPath "$archive.sha256"
$hash | Format-List
Write-Host "Portable build: $package"
