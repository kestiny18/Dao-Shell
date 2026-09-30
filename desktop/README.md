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
The default model selector appears first. Each listed model can be edited, tested or removed independently; choose “暂不使用模型” to disable model use. Other model names and multiple models remain available. Existing shared connections are preserved internally. Editing one model splits its connection when necessary; entering a new key isolates its credential reference from other models. Leaving the key blank retains the current reference; changing the API address does not transfer saved credentials to the new address. Existing CLI settings are
imported automatically. Saved changes apply between requests and reset all model execution contexts and
file candidates; visible conversation history is retained.

Drag the divider to adjust sidebar width, or focus it and use the arrow keys;
double-click restores the default. The computer overview offers cards or a list
for registered apps, with local icons where available and initials otherwise.
Sidebar width and app view are remembered on this device.

Network interfaces always use cards.
The resource details section shows CPU and memory top-ten process lists from the
same short observation, with whole-machine CPU percentages and binary memory units.
Changing the sort does not sample again. The time/window, unknown values and number
of processes lost between observations remain visible. Refresh is manual after the
initial overview load; a failed refresh retains the dated previous snapshot. No
process history is persisted. Model explanation is opt-in and sends the displayed
system/disk facts and both top-ten lists (process names, PIDs and usage), not the
application catalog. A high value alone does not identify a slowdown's cause.

Registered applications use the page scroll
and display all collected records (registry enumeration still has a safety bound).
Installation sizes are registry `EstimatedSize` estimates; missing values are unknown,
not zero, and no installation directory is scanned. Running badges require an exact
match between a readable process executable path and a uniquely registered executable
`DisplayIcon` path. Shared icon paths, directory-only registrations, protected processes
and unmatched applications remain unknown, never “not running”. Confirmed applications
appear first; the overview includes the application sample time and coverage limits.
Double-click an application to run it, or right-click / press Shift+F10 for Run,
Run as administrator, Open file location and Uninstall. Uninstall only opens the
fixed Windows installed-apps settings page; no registry uninstall command is run.
Launching is conservative: a unique, exactly named Start-menu shortcut must point
to a local fixed-drive EXE without arguments or a custom working directory. Ambiguous,
missing, network, reparse-point, installer/maintenance and command-interpreter targets
are unavailable with an explanation. Store and portable applications are not fully
covered. DisplayIcon is never treated as a launch instruction. Enumeration is bounded
to 4,096 entries and eight nested directories. Execution rechecks and pins the shortcut
and target identity; refresh invalidates old action references. Administrator execution
uses Windows UAC; cancellation is reported as cancellation. A handoff receipt does
not prove that the application started. These actions share the global request lock
and are available only through explicit UI actions, never model tools.

The existing green D/chevron mark is explicitly configured in `bundle.icon` as
both PNG and Windows ICO, including the installer icon.

Keys can remain in this desktop process or be saved in Windows Credential Manager.
A temporary key from another CLI process is not available here. Configured
environment variables take precedence over stored credentials; newly entered keys
take precedence in the current process. Saved credential values are never returned
to the frontend. Removing a model does not delete its OS credential entry.

Settings → Access offers restricted access to chosen directories or full access
within the current OS user's authority. Read and CLI-write directories are lists: add using the native Windows folder picker and remove individual entries. Canceling the picker changes nothing; selected paths stay in the draft until Save. This does not scan the selected folders. Full access does not elevate privileges,
skip confirmation or enable extra operations. Search starts in selected locations
(Downloads, Documents and Desktop when none are selected), rather than scanning
all disks. Specify a directory in natural language to search another location.

Rust, a working native compiler and WebView2 are required. The local experiment
was built with the repository's GNU toolchain and local w64devkit; the wrapper
also works with a configured system toolchain. It does not install dependencies
or change your global PATH. There is no Node frontend build step.

Try “找一下项目里的合同 PDF”, select a candidate, then confirm opening it.
The input uses the configured model automatically. Without a model, or when its
connection is unavailable before any tool call, it falls back once to local filename
search using the original input. The notice explains that this literal search does
not understand natural language. Cancellation, permission refusal, protocol/tool
errors and failures after a tool call never trigger this fallback. Stop cancels the current request; New session creates an independent conversation. An opening receipt reports the OS handoff, not proof that the
target application displayed the document.

## Workspace and local history

The fixed first tab is Overview (概览). New session creates a conversation in the
sidebar; closing its tab preserves the session, and selecting it reopens the tab.
Settings uses one closable tab. Hover or focus a session row to rename it; there is
no session deletion control. Enter saves a name and Escape cancels editing. Names
must be nonempty and fit within 512 UTF-8 bytes. Hover or focus a tab to reveal its
close button. Closing the running session first asks to stop it,
then waits for cancellation. Only one request runs at a time; other sessions can
still be viewed and their drafts edited. Enter sends; Shift+Enter adds a line;
IME composition does not submit.

Sidebar collapse is temporarily hidden; older collapsed layouts open expanded.
The divider still resizes the sidebar. History, drafts, open tabs, selected tab and sidebar width are
saved to `workspace.json` beside the desktop configuration. By default this is
`%LOCALAPPDATA%\Dao-Shell`; `DAO_SHELL_DESKTOP_CONFIG` also isolates workspace
storage to that configuration's directory. Visible messages and drafts can contain
private text and paths. They are stored locally as plain JSON, with no cloud sync.
Uninstall continues to preserve this local data.

Restart restores visible history only. Model/tool context, candidates, pending
confirmations and active requests are never restored or replayed. The conversation
shows a notice explaining that old references need a new description or search.
No model call is made by restoration. Keys, raw model protocol, reasoning and
capability tokens are excluded from the workspace schema. Text the user explicitly
types remains ordinary visible history, so avoid typing secrets in conversations.

Saving uses a synchronized temporary file and atomic replacement. Save errors stay
visible with a retry button; a failed rename retains its editor and previous name. Exit
waits for pending saves and asks before leaving after a failure. If restoration
fails because data is damaged or the version is unsupported, the original file is
preserved and automatic saving is blocked. Close the app, back up and move
`workspace.json` out of the configuration directory, then restart to begin a new
workspace. Do not remove the model configuration. Limits are 100 sessions,
2,000 messages per session, 16,000 input characters and 8 MiB per workspace.

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
CPU and memory top-ten process facts to the configured model; it does not send the application inventory.

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


Computer profile loads only when expanded or explicitly refreshed, is cached in
this window, and never enters model facts or workspace history. It reads CPU and
memory via sysinfo, Windows firmware manufacturer/product registry values, and
EnumDisplayDevices adapter descriptions. Missing/OEM placeholder values remain
unknown; display adapters may be virtual. Storage lists individual volume capacities
without paths or a summed physical-device claim. Copy uses the same visible
whitelisted summary (including its collection time); machine/user names, serials,
network addresses and private mount paths are excluded. Clipboard failures are
reported and the selectable summary remains available for manual copying.
