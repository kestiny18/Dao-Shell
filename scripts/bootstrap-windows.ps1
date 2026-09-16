# Optional isolated GNU development toolchain. No system PATH changes.
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$toolsDirectory = Join-Path $workspace '.tools'
New-Item -ItemType Directory -Force -Path $toolsDirectory | Out-Null
$rustup = Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe'
if (-not (Test-Path -LiteralPath $rustup)) {
    $installer = Join-Path $toolsDirectory 'rustup-init.exe'
    Invoke-WebRequest 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe' -OutFile $installer
    & $installer -y --profile minimal --default-host x86_64-pc-windows-gnu --default-toolchain none --no-modify-path
    if ($LASTEXITCODE -ne 0) { throw 'rustup installation failed' }
}
if (-not (Test-Path -LiteralPath (Join-Path $toolsDirectory 'w64devkit\bin\gcc.exe'))) {
    $compiler = Join-Path $toolsDirectory 'w64devkit.exe'
    Invoke-WebRequest 'https://github.com/skeeto/w64devkit/releases/download/v2.10.0/w64devkit-x64-2.10.0.7z.exe' -OutFile $compiler
    $expected = '18d0a4c71a166f8401ab6305781bec5882b40b5e06ba9807c61cb5f3b3c6325e'
    if ((Get-FileHash -LiteralPath $compiler -Algorithm SHA256).Hash.ToLower() -ne $expected) { throw 'Compiler hash mismatch' }
    & $compiler -y "-o$toolsDirectory"
    if ($LASTEXITCODE -ne 0) { throw 'Compiler extraction failed' }
}
Write-Host 'Toolchain ready. Run .\scripts\dev.ps1 test. An existing MSVC Rust host is preserved.'
