# Dao-Shell desktop preview

A small desktop entry for understanding your computer and finding files with natural language.
This is a Windows development preview, not a packaged release.

## Try it on Windows

From the repository root in PowerShell:

```powershell
.\scripts\desktop.ps1 run
```

Open Settings → Models to add a connection, enter its model names and API key,
test the connection, select a default model and save. Existing CLI settings are
imported automatically. Saved changes apply between requests and start a new
session; navigating between pages preserves the current conversation.

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
- Desktop model/access/theme settings. No tray service, global shortcut, installer or background agent.
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

The 2026-09-21 implementation passed Rust, bridge and DOM behavior checks. Native
visual revalidation of the new pages is pending because the Windows desktop was
locked during the attempted check; do not treat DOM tests as native UI acceptance.

See the [experiment record](../docs/experiments/desktop-entry.md) for boundaries and evidence.
