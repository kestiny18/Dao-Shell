# WinGet ABI provenance

`bindings.rs` is a private Rust projection generated with windows-bindgen 0.66.0 from the installed Microsoft.DesktopAppInstaller 1.29.379.0 `Microsoft.Management.Deployment.winmd` (64,280 bytes), SHA256 `B101A7A50B4698AF369A57EC4CFBCD2DF934333E2B4B8FE4BA27968F3FAF7D69`. The winmd binary is not committed. WinGet CLI reports v1.29.380. This projection is not a public arbitrary WinGet interface.

Regeneration with a local windows-bindgen runner calling `windows_bindgen::bindgen(std::env::args().skip(1))`:

```powershell
$metadata = Join-Path (Get-AppxPackage Microsoft.DesktopAppInstaller).InstallLocation 'Microsoft.Management.Deployment.winmd'
phase4-binding-generator.exe --in default $metadata --out src/software/bindings.rs --filter Microsoft.Management.Deployment --reference windows,skip-root,Windows
rustfmt --edition 2024 src/software/bindings.rs
```

Underlying API and Microsoft sample are published in the [WinGet v1.29.380 source](https://github.com/microsoft/winget-cli/tree/v1.29.380/src/Microsoft.Management.Deployment) under the repository MIT license. Generated projection uses the windows-rs generator's standard output; it is checked in to avoid runtime metadata/code-generation dependencies.

T0 on this unpackaged workstation: exact production PackageManager CLSID `C53A4F16-787E-42A4-B304-29EFFB4BF597`, generated local IID, STA/CLSCTX_ALL returns `0x80073D54` (no package identity). MTA/local-server attempt returned the same. This is an observed host limitation, not a claim that the API never works unpackaged. No registration or package identity workaround was performed. Structured read-only `winget export` also failed; absence is therefore **unknown**, never verified absent. No maintenance operation was executed. Evidence: `.tools/phase4/t0-winget*.json`.
