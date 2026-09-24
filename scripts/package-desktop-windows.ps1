#Requires -Version 7.0
# Builds one release installer. No model configuration or credentials are packaged.
param([ValidateRange(1, 32)][int]$Jobs = 2)
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$desktop = Join-Path $workspace 'desktop'
$previousPath = $env:Path
try {
    $env:Path = "$(Join-Path $env:USERPROFILE '.cargo\bin');$(Join-Path $workspace '.tools\w64devkit\bin');$previousPath"
    $compiler = & rustc --version --verbose
    if ($LASTEXITCODE -ne 0 -or -not ($compiler -match '^host: x86_64-pc-windows-')) {
        throw 'This packaging script requires an x64 Windows Rust host'
    }
    & npm ci --prefix $desktop
    if ($LASTEXITCODE -ne 0) { throw 'Desktop packaging dependencies could not be installed' }
    $metadataJson = & cargo metadata --manifest-path (Join-Path $desktop 'src-tauri\Cargo.toml') --locked --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Desktop Cargo metadata failed' }
    $metadata = $metadataJson | ConvertFrom-Json -AsHashtable
    $project = $metadata.packages | Where-Object { $_.name -eq 'dao-shell-desktop' -and -not $_.source }
    $resources = Join-Path $metadata.target_directory 'package-resources'
    $notices = Join-Path $resources 'third-party'
    New-Item -ItemType Directory -Force -Path $notices | Out-Null
    Copy-Item -LiteralPath (Join-Path $workspace 'LICENSE') -Destination $resources
    Copy-Item -LiteralPath (Join-Path $desktop 'TRY-IT.zh-CN.txt') -Destination $resources
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
    $sysroot = & rustc --print sysroot
    foreach ($name in @('COPYRIGHT.html', 'COPYRIGHT-library.html')) {
        $notice = Join-Path $sysroot "share\doc\rust\$name"
        if (Test-Path -LiteralPath $notice) { Copy-Item -LiteralPath $notice -Destination $notices }
    }
    $mingw = Join-Path $workspace '.tools\w64devkit\COPYING.MinGW-w64-runtime.txt'
    if (Test-Path -LiteralPath $mingw) { Copy-Item -LiteralPath $mingw -Destination $notices }
    $overlay = Join-Path $metadata.target_directory 'package-config.json'
    @{ bundle = @{ resources = @{ "$($resources.Replace('\', '/'))/" = './' } } } | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 -LiteralPath $overlay
    Push-Location $desktop
    try {
        & node node_modules/@tauri-apps/cli/tauri.js build --ci --no-bundle --config $overlay -- --locked --jobs $Jobs
        if ($LASTEXITCODE -ne 0) { throw 'Desktop release build failed' }
        # GNU links the WebView2 loader dynamically; bundling the runtime alone is insufficient.
        $loader = Join-Path $metadata.target_directory 'release\WebView2Loader.dll'
        if ($compiler -match '^host: x86_64-pc-windows-gnu$') {
            if (-not (Test-Path -LiteralPath $loader)) { throw 'Required WebView2Loader.dll is missing' }
            Copy-Item -LiteralPath $loader -Destination $resources
        }
        & node node_modules/@tauri-apps/cli/tauri.js bundle --ci --bundles nsis --config $overlay
        if ($LASTEXITCODE -ne 0) { throw 'Desktop installer bundling failed' }
    } finally { Pop-Location }
    $binary = Join-Path $metadata.target_directory 'release\dao-shell-desktop.exe'
    $bytes = [IO.File]::ReadAllBytes($binary)
    $pe = [BitConverter]::ToInt32($bytes, 0x3c)
    if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x8664) { throw 'Expected an x64 desktop executable' }
    $subsystem = [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
    if ($subsystem -ne 2) { throw "Desktop executable must use Windows GUI subsystem (found $subsystem)" }
    $installer = Join-Path $metadata.target_directory "release\bundle\nsis\Dao-Shell_$($project.version)_x64-setup.exe"
    if (-not (Test-Path -LiteralPath $installer)) { throw "Installer missing: $installer" }
    $dist = Join-Path $workspace 'dist'
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    $output = Join-Path $dist "Dao-Shell-$($project.version)-windows-x64-setup.exe"
    Copy-Item -LiteralPath $installer -Destination $output
    $hash = Get-FileHash -LiteralPath $output -Algorithm SHA256
    "$($hash.Hash.ToLower())  $([IO.Path]::GetFileName($output))" | Set-Content -Encoding ASCII -LiteralPath "$output.sha256"
    Write-Host "Installer: $output"
    Write-Host 'Verified: release executable uses Windows GUI subsystem (no console window).'
    $hash | Format-List
} finally { $env:Path = $previousPath }
