# Dao-Shell desktop preview

A small desktop entry for understanding your computer and finding files with natural language.
This is a Windows development preview. A single-file Windows x64 installer can
be built for private trials; it is not a signed stable release.

## Try it on Windows

For a shared trial build, double-click `Dao-Shell-0.1.0-windows-x64-setup.exe`,
then open Dao-Shell from the desktop or Start menu. The installer includes the
WebView2 offline installer and reuses an existing runtime. No Rust, Node.js or
terminal is needed on the recipient's computer. Windows 10/11 x64 is the target.

To run from source, use PowerShell at the repository root:

```powershell
.\scripts\desktop.ps1 run
```

Open Settings at the bottom left → Models. Choose DeepSeek, OpenAI or Custom,
select a model and enter its API key, then test and save. Official service addresses
are filled automatically; Custom supports other compatible services and local models.
Other model names and multiple models remain available. Existing CLI settings are
imported automatically. Saved changes apply between requests and start a new
session; navigating between pages preserves the current conversation.

Drag the divider to adjust sidebar width, or focus it and use the arrow keys;
double-click restores the default. The computer overview offers cards or a list
for registered apps, with local icons where available and initials otherwise.
Sidebar width and app view are remembered on this device.

The existing green D/chevron mark is explicitly configured in `bundle.icon` as
both PNG and Windows ICO, including the installer icon.

Keys can remain in this desktop process or be saved in Windows Credential Manager.
A temporary key from another CLI process is not available here. Configured
environment variables take precedence over stored credentials; newly entered keys
take precedence in the current process. Saved credential values are never returned
to the frontend. Removing a connection does not delete its OS credential entry.

Settings → Access offers restricted access to chosen directories or full access
within the current OS user's authority. Full access does not elevate privileges,
skip confirmation or enable extra operations. Search starts in selected locations
(Downloads, Documents and Desktop when none are selected), rather than scanning
all disks. Specify a directory in natural language to search another location.

Rust, a working native compiler and WebView2 are required. The local experiment
was built with the repository's GNU toolchain and local w64devkit; the wrapper
also works with a configured system toolchain. It does not install dependencies
or change your global PATH. There is no Node frontend build step.

Try “找一下项目里的合同 PDF”, select a candidate, then confirm opening it.
Natural language is the default. “文件名搜索” provides an explicit fallback
without a model. Stop cancels the current request; New session clears history
and candidates. An opening receipt reports the OS handoff, not proof that the
target application displayed the document.

## Current scope

- Search, inspect metadata, and confirm opening existing files.
- Restricted or full path access; no desktop file move/write tools. CLI moves still require confirmation and a durable journal.
- Session-local candidates, model history, cancellation and one-time confirmation.
- Existing production search, not the experimental inventory backend.
- Desktop model/access/theme settings and a Windows preview installer. No tray service, global shortcut, automatic updater or background agent.
- Local CPU, memory, disk and device overview, interface traffic counters and Windows uninstall-registry records. Application records do not cover every Store or portable app; an interface IP does not prove internet reachability.

Like the CLI, natural-language requests send the prompt and relevant tool results
(including file names, paths and metadata) to the configured model provider.
File contents are not read by these tools. Direct filename search and the home page stay local.
The explicit explanation button sends the displayed CPU/memory/disk snapshot and
top-five process facts to the configured model; it does not send the application inventory.

## Development checks

Close a running desktop build before rebuilding or checking it on Windows.

```powershell
.\scripts\desktop.ps1 fmt -CargoArgs @('--check')
.\scripts\desktop.ps1 test
.\scripts\desktop.ps1 clippy -CargoArgs @('--all-targets', '--', '-D', 'warnings')
.\scripts\desktop.ps1 build
node --check desktop/ui/app.js
npm ci --prefix desktop
npm test --prefix desktop
```

For a repeatable UI check without cloud credentials, keep this fixture running
in one terminal:

```powershell
node scripts/desktop-fixture.mjs .tools/desktop-fixture
```

In another terminal, launch with its isolated configuration:

```powershell
$env:DAO_SHELL_DESKTOP_CONFIG = (Resolve-Path .tools/desktop-fixture/config.json).Path
try { .\scripts\desktop.ps1 run }
finally { Remove-Item Env:DAO_SHELL_DESKTOP_CONFIG }
```

The fixture writes synthetic text files to that directory and returns fixed model
responses. Try “找一下合同并打开第一份”, “找一下不存在的文件” and “慢一点找合同”.
The last request delays its response to allow testing Stop. Ctrl+C stops the fixture.
This validates the UI and protocol plumbing, not real-model reasoning quality.

The 2026-09-21 implementation passed 70 Rust, bridge and DOM checks (one CPU-load
test remains intentionally ignored). Native Windows verification completed on
2026-09-22 with the isolated fixture: overview and explicit explanation, settings
connection test/save/restart, navigation, file results, rejected opening, empty
results and cancellation. Test/save feedback stays beside the corresponding
action; expanding the app list keeps scrolling inside the content area.
Cloud-model quality and validation on a separate clean Windows machine remain
follow-ups; local installation testing does not replace that check.

## Build a shareable Windows installer

```powershell
.\scripts\package-desktop-windows.ps1
```

The script installs the pinned Tauri CLI locally, builds release code with the
locked Cargo dependencies, bundles licenses and the Chinese trial guide, and
writes the installer plus SHA-256 checksum to `dist/`. It uses two build jobs by
default to bound peak memory (`-Jobs 4` overrides this). Packaging initially needs
network access for build tools and Microsoft's WebView2 offline installer.

The installed executable uses the Windows GUI subsystem, which the script checks
before delivery. It launches without a console window. Program files default to
`%LOCALAPPDATA%\Programs\Dao-Shell`; configuration stays in
`%LOCALAPPDATA%\Dao-Shell`. Custom installation paths are supported. Uninstall
removes packaged files and shortcuts, preserving model configuration and saved
Windows credentials. The package includes neither a model key nor local settings.

This preview is unsigned: Windows can show an unknown-publisher or reputation
prompt. Share the source and checksum with testers; do not ask them to disable
Windows protection. Only the setup executable is required for installation.

Installer implementation reference: [Tauri Windows distribution](https://v2.tauri.app/distribute/windows-installer/).

See the [experiment record](../docs/experiments/desktop-entry.md) for boundaries and evidence.
