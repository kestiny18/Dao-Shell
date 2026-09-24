# Windows installer trial

Status: packaged and locally verified 2026-09-24; interactive checks await an unlocked desktop.

- [x] Configure one Windows x64 NSIS executable with the existing icon and offline WebView2 installation.
- [x] Keep default program files under LocalAppData/Programs/Dao-Shell, separate from configuration.
- [x] Add a repeatable packaging script, trial guide and notices.
- [x] Build release and verify Windows GUI subsystem (no console).
- [x] Inspect DLL imports; GNU requires WebView2Loader.dll. Add it after building and before bundling.
- [x] Finish the corrected installer and verify its installed files.
- [x] Verify installation, launch with a minimal PATH and uninstall; preserve existing configuration.
- [x] Record artifact checksum and evidence; setup executable is ready for delivery.

The first installer omitted WebView2Loader.dll and was replaced. The corrected
script uses Tauri build --no-bundle, copies the loader for GNU, then runs Tauri
bundle. The final setup is `dist/Dao-Shell-0.1.0-windows-x64-setup.exe`,
221,988,691 bytes, SHA-256:
`1f30b6ef9be36be5e68019b86365000bf6380ecde24adae1bb984f453a4be51f`.

Isolated test configuration and a synthetic file are in
`.tools/installer-validation`. `baseline.json` records whether the user's
configuration existed and its hash, without copying its contents. All five DOM
checks passed. Installation and uninstallation both returned exit code 0. The
default directory is LocalAppData/Programs/Dao-Shell; installation contained
1,260 files and a Start menu shortcut. The main executable and loader were checked.
Launch with only Windows system
directories in PATH loaded the adjacent WebView2Loader.dll and rendered the
home page, as confirmed by its accessibility document and local overview data.
The only direct child was WebView2. Both the build and installed app have x64
machine type and Windows GUI subsystem 2. The installed executable differs from
the build only in Tauri's three-byte UNK-to-NSS bundle marker.

Uninstallation removed the program, registration and shortcut, preserving the
existing configuration hash and synthetic file. A second installation succeeded
and was left available for user testing; no task-owned app process remains open.
The earlier five-hour heartbeat remains paused; this request did not restart it.

## Interactive follow-up

- [ ] Walk through the normal installer wizard and run a filename search from the installed UI after Windows is unlocked.

The native helper could read accessibility data but captured the Windows lock
screen; activation failed. No further UI input was attempted after recognizing
the locked desktop. The user was asked to unlock or accept delivery with this
remaining check. Silent installation exercises the installer engine but does not
constitute a completed click-through wizard check.

No code-signing certificate is configured. Separate clean-machine verification
is not available on this host; local installation testing must not be presented
as that evidence.
