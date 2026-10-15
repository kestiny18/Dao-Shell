# Dao-Shell desktop preview

A small desktop entry for understanding your computer and finding files with natural language.
This is a Windows development preview. A single-file Windows x64 installer can
be built for private trials; it is not a signed stable release.

## Try it on Windows

For a shared trial build, double-click `Dao-Shell-0.1.0-windows-x64-setup.exe`,
then open Dao-Shell from the desktop or Start menu. The installer includes the
small WebView2 bootstrapper and reuses an existing runtime. If the runtime is
missing, installation needs internet access to download it from Microsoft.
Offline recipients without WebView2 must install the standalone runtime first.
No Rust, Node.js or terminal is needed on the recipient's computer.
Windows 10/11 x64 is the target.

To run from source, use PowerShell at the repository root:

```powershell
.\scripts\desktop.ps1 run
```

The interface follows the Windows user's UI language by default: Chinese variants
use Simplified Chinese; other or unknown languages use English. Override this in
Settings → General & appearance → Interface language → Follow system / 简体中文 /
English, then Save settings. The choice survives restart; existing conversation
history, user titles, application names and file paths are preserved as written.
New model requests follow the interface language unless the user requests another
language. System-owned dialogs may follow Windows' own language. Trial guides in
both English (`TRY-IT.en.txt`) and Chinese (`TRY-IT.zh-CN.txt`) are packaged.

Open Settings at the bottom left → Models. Choose DeepSeek, OpenAI or Custom,
select a model and enter its API key, then test and save. Official service addresses
are filled automatically; Custom supports other compatible services and local models.
The default model selector appears first. Each listed model can be edited, tested or removed independently; choose “Do not use a model” to disable model use. Other model names and multiple models remain available. Existing shared connections are preserved internally. Editing one model splits its connection when necessary; entering a new key isolates its credential reference from other models. Leaving the key blank retains the current reference; changing the API address does not transfer saved credentials to the new address. Existing CLI settings are
imported automatically. Saved changes apply between requests and reset all model execution contexts and
file candidates; visible conversation history is retained.

Drag the divider to adjust sidebar width, or focus it and use the arrow keys;
double-click restores the default. The computer overview offers cards or a list
for registered apps, with local icons where available and initials otherwise.
Sidebar width and app view are remembered on this device.

Network interfaces always use cards.
The resource details section shows CPU and memory top-ten process lists from the
same short observation, with whole-machine CPU percentages and binary memory units.
Both rankings stay visible; viewing them does not sample again. The time/window, unknown values and number
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
`DisplayIcon` path or a unique trusted Start-menu target. Shared executable paths, directory-only registrations, protected processes
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
within the current OS user's authority. Read and CLI-write directories are lists: add using the native Windows folder picker and remove individual entries. Canceling the picker changes nothing; selected paths stay in the draft until Save. This does not scan the selected folders. Full access automatically selects the authenticated whole-volume name service when
its installed build matches. There is no separate search enable switch. Restricted
access searches only permitted directories. First explicitly saving Full offers the
fixed index component install/update/start action when needed; it still requires a
separate local decision and Windows administrator permission. Startup, status and
search never install or repeatedly request elevation. Missing or incompatible
components fall back only to the listed compatible directories (Downloads,
Documents and Desktop when none are selected), with a visible coverage receipt.
Full access does not elevate the desktop or authorize arbitrary operations.

Rust, a working native compiler and WebView2 are required. The local experiment
was built with the repository's GNU toolchain and local w64devkit; the wrapper
also works with a configured system toolchain. It does not install dependencies
or change your global PATH. There is no Node frontend build step.

Try “找一下项目里的合同 PDF”, then click a current candidate to open it.
The input uses the configured model automatically. Without a model, or when its
connection is unavailable before any tool call, it falls back once to local filename
search using the original input. The notice explains that this literal search does
not understand natural language. Cancellation, permission refusal, protocol/tool
errors and failures after a tool call never trigger this fallback. Stop cancels the current request; New session creates an independent conversation. A current candidate click goes directly to the Windows default open handler, including
HTML, executable and other associated types, without a second app confirmation.
Executable files run with ordinary OS semantics; no verb, parameters or elevation
request is generated from model text. Model-initiated file_open still asks locally.
Scope, current candidate and native identity checks remain required. An opening
receipt reports OS handoff, not proof that the target application displayed it.

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

- Search, inspect metadata, and open current candidates with the system default application.
- Restricted or full path access; no desktop file move/write tools. CLI moves still require confirmation and a durable journal.
- Session-local candidates, model history, cancellation and one-time confirmation.
- Opt-in selected-directory snapshots for literal filename search; other requests retain live scanning.
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
network access for build tools and Microsoft's small WebView2 bootstrapper.
The default `webviewInstallMode` is `embedBootstrapper`; it embeds the bootstrapper,
not the complete runtime installer. Keep `WebView2Loader.dll` in the GNU package.

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


## File search snapshots (P9)

Settings → 文件搜索 → 添加加速目录 uses the native picker and current saved
read permissions. Registration takes effect immediately, independently of the
model/access draft. The application stores metadata-only SQLite caches and a
registration JSON under `file-inventory` beside its configuration. Cache storage
must be outside the selected directory. Overlapping registrations are rejected;
remove the earlier registration before selecting a broader/narrower root.

Building runs serially outside the session lock. The view offers update, stop
and remove, with observed counts, last completed time and failure messages.
Stopping ends that directory’s automatic updates, preserves the previous snapshot
and uses live scanning until another manual update or restart. Removing deletes this application's snapshot, never source files.
Restart restores registrations and the old snapshot, then rebuilds before using
it for search. A damaged cache remains marked for explicit rebuild. While the application runs, ordinary-user filesystem notifications are armed
before scanning and coalesced into bounded batches. Ordinary file changes are
reobserved and transactionally published; directory changes, ambiguity and queue
loss trigger full reconciliation. Periodic reconciliation runs every five minutes,
even when incremental updates keep arriving. Watch failures visibly degrade to
live search and retry with 30–300 second backoff; exceeding scan budgets pauses
automatic retries until manual update/restart. Exiting ends listeners and updates.
Snapshots remain event-assisted observations, never an absolute live guarantee.

A literal `query`, all four existing sort options and pagination use snapshots
only when every requested root exactly matches an available registration. Terms,
kind, extension, date and size filters, unregistered/subdirectory ranges and
updates in progress conservatively fall back to the existing complete request.
Model-generated terms queries therefore may not accelerate yet. No default
search roots are removed. Indexed results use snapshot ordering and counts;
candidates are rechecked against current permission and identity before becoming
session references. Changed candidates are skipped rather than silently opened.
Every search receipt explains its source and coverage. CLI behavior is unchanged.

Self-tests use temporary synthetic directories for search parity, restart,
corruption, stopped updates, removal races and permission/identity changes.
The final reply carries the same source/coverage receipt as live events so late
IPC delivery cannot hide it. Bootstrap notifications are reconciled before the
first available snapshot is published.

The [P9 prototype evidence](../docs/devlog/2026-10-02-desktop-file-inventory.md)
records real no-model backend-to-WebView measurements on 20,000 synthetic files:
30 samples each for common/rare/absent queries, first-screen p95 153/133.5/127 ms.
These are warm-cache Debug results, not whole-machine or cold-disk guarantees.
The 100,010-file budget fixture pauses at the 100,000-entry limit and recovers
when narrowed. Use a fresh isolated configuration and synthetic directory first;
set `DAO_SHELL_DESKTOP_CONFIG` before launching the independent Debug executable.
Independent testing and review remain NOT_RUN; special ACLs, network/cloud paths,
long-running soak tests and million-file scale remain unverified.

## Windows index service candidate (P11.3)

The standalone `dao-index-worker` now includes fixed SCM lifecycle commands and
an ordinary-user service client. P11.4 connects it to desktop name discovery and
explicit settings actions as described below. P9 registrations remain compatible.

Build with `scripts/dev.ps1 build --locked --release --bin dao-index-worker`.
Read-only commands are `service-status`, `service-health`, and `discover-volumes`.
Production lifecycle commands accept no arbitrary installation/database/service
paths and reject `--report`. Install, upgrade and repair require a SHA256 frozen
before elevation. The SCM image is copied into a protected Known Folder
ProgramFiles digest directory; it never runs from the user-writable desktop
installation. Index data resides under protected ProgramData. The initial
version serves only the recorded installation user's authenticated interactive
logons, and still filters each name and complete ancestor path as that caller.

`service::request_volume` and the `service-volume-status` / `service-search` CLI
commands authenticate fixed SCM ownership and protected image identity. A volume
catalog's `supported` flag is eligibility, not coverage. `worker_started` means
initialization and pipe creation; per-volume Status carries publication/coverage.
An unavailable volume cannot silently stop or cover other volumes. Discovery is
performed at startup and, in the P11.5 recovery candidate, every five seconds.
Actual hotplug and sleep/resume validation remains outstanding.

On 2026-10-14 the 04B9986F worker was actually installed as the automatic LocalSystem
service. The corrected ordinary client authenticated it and completed synthetic
USN create, rename, hardlink and delete trials without widening operation Scope.
Real service restart/checkpoint recovery, upgrade, rollback, repair, uninstall,
host reboot and multi-account behavior remain NOT_VERIFIED. See the
[live service record](../docs/devlog/2026-10-14-p11-live-service.md) for the tested
versions and limitations; earlier [P11.3 evidence](../docs/devlog/2026-10-13-p11-3-index-service.md)
records the preceding isolated candidate. Independent testing and review remain NOT_RUN.

## Desktop service integration candidate (P11.4)

Run `scripts/prepare-index-payload.ps1` before `scripts/desktop.ps1 build` to
explicitly build the fixed worker payload and SHA256 manifest. The packaging
script performs this preparation and requires the payload. `build.rs` only
verifies and embeds those bytes; it never builds another Cargo package or opens
UAC. Developer builds without a payload keep compatible directory search and
show that installation is unavailable.

Settings → file search shows service, per-volume coverage, generation, publication
time and bounded reasons. Full access automatically uses the service; managing the
component requires an explicit local decision. Ordinary searches and status polling
never elevate. An already
stopped or incompatible service asks for the separate start/upgrade action.
The initial elevated installer copies a SHA-bound image to protected ProgramFiles;
SCM never runs the temporary or user-writable desktop payload path.

In Full access mode, a literal filename `query`, default relevance, first page and no
additional filters use supported online volumes. Every result is filtered by the
service under the actual Windows caller and rechecked for current GUID/mount,
volume serial and file identity before creating a session-owned reference.
Name discovery may extend beyond configured operation roots; opening, inspecting
or writing still requires the original Scope and current identity. Model opening and
all write requests retain local confirmations. No access setting
is widened. Reset invalidates references while retaining the selected backend.

Terms, paths, metadata filters, other sort orders and subsequent pages retain
their original semantics using only the listed compatible directories with the
existing bounded scan. An unavailable service similarly falls back only there.
Receipts explicitly distinguish this coverage; empty compatible roots explain
the limitation instead of silently scanning every volume. P9 databases are not
migrated or deleted. The old `file-index-service.json` preference is preserved but no longer determines
routing; the saved Access mode is authoritative.

The earlier P11 ordinary desktop was tried against the installed 04B9986F service
with isolated configuration and synthetic files. Disabling discovery uses compatible
search while preserving the running service; enabling it again needs no elevation
when the matching service is already running. Exiting the desktop left the service
maintaining changes, and reopening found the new synthetic name. This is local
prototype evidence, not a clean-machine or multi-account acceptance result.
Measured latency targets were not met and remain recorded as FAIL; the user has
deferred performance optimization to prioritize functional completeness. See the
[live service record](../docs/devlog/2026-10-14-p11-live-service.md),
[function-completeness record](../docs/devlog/2026-10-14-p11-function-completeness.md)
and earlier [P11.4 checkpoint](../docs/devlog/2026-10-13-p11-4-desktop-service-search.md).

## Recovery candidate (P11.5, isolated tests)

Runtime discovery reconciles GUID + serial identities rather than drive letters.
An offline/replaced worker is cancelled and joined before that identity is started
again; waiting for an offline worker does not block other workers or the control pipe.
Failed discovery preserves known coverage and displays an incomplete-discovery
reason. The catalog (including offline history) and retained storage directories
are bounded to 128; at most 16 volume workers run. Full storage directories are
retained rather than automatically deleted to make room. Known temporary device
or Journal unavailability retries with capped backoff; unknown failures and
Journal gaps do not silently rebuild.

Each available volume in settings offers **重建索引** with a local confirmation.
The rebuild request uses the fixed authenticated volume pipe and carries no path,
SQL, SID or arbitrary command. A single pending/running request is deduplicated,
and new requests are limited to one per 30 seconds. Accepted means queued, not
complete. The existing writer rebuilds in one transaction; a failure or service
stop retains the old published generation. Rebuild never changes the source
files or USN Journal and never launches UAC. A stopped worker must first become
available again; rebuilding is not an installer/repair substitute.

Identity binding, catalog coordination, pipe controls, cancellation and rollback
are covered by isolated simulation/ordinary Windows fixture tests. The installed
04B9986F SYSTEM service also passed synthetic live USN lifecycle trials. Physical
volume replacement, sleep/resume, service restart/checkpoint recovery and the newer
service-side F1 fixes remain NOT_VERIFIED in that installed service. P11 as a whole
remains IN_PROGRESS; see
[recovery evidence](../docs/devlog/2026-10-14-p11-5-recovery.md).

The 2026-10-14 function-completeness follow-up keeps a discovered unsupported
volume distinct from an offline volume across repeated discovery and worker exit.
Settings retain the published generation and time when maintenance fails, and
translate only bounded public error codes. A journal gap with a live writer still
offers the locally confirmed rebuild action. If maintenance or the volume worker
has exited, stop and start the service explicitly in Service Management before
checking it again; no automatic restart, rebuild or elevation is performed.
A storage-identity failure requires checking storage and repairing the service.
Stopped, disconnected and incompatible service states show the corresponding
manual recovery guidance. These changes are isolated self-tested source changes;
they do not upgrade the running service. See the
[function-completeness record](../docs/devlog/2026-10-14-p11-function-completeness.md).

The unified function candidate embeds the newly built worker and its matching
manifest digest in the Release desktop and NSIS installer. Building or bundling
does not install that worker. Against the still-installed 04B9986F service, this
new desktop is expected to show **service version mismatch** and use the listed
compatible search directories until an explicit service upgrade is authorized.
The separately frozen F1 Debug client retains the old 04B9986F payload only for
testing new client/UI behavior against that existing service; it is not the
unified deliverable. Do not silently mix these candidates or repeatedly retry
a canceled administrator prompt.

## Installed trial feedback candidate (P12)

Memory shows percentage plus used/total capacity. Disks adapt to available width;
CPU and memory rankings are shown together. Top summary cards jump to resource
and application details. Known running apps appear first.

Process rows and confirmed running app cards offer End with a local confirmation
listing the frozen identities. This can lose unsaved work. Only verified current-user,
same-session ordinary processes are eligible; system, self, unknown identities and
changed instances are refused without elevation. Ending an application affects only
its explicitly matched instances, not an inferred process tree. Pending and partial
outcomes remain distinct; no process history or end action is sent to the model.

Local search/operation receipts and historical paths collapse by default. Errors and
incomplete coverage retain a brief visible summary. The answer and current candidates
stay visible. These receipts are not model internal reasoning.

For persistent volume failures on another computer, Settings → File search → Generate
diagnostic report produces copyable JSON containing only version/build, drive letters
or anonymous volume ordinals, safe stage/code, generation and publication time. It
excludes private filenames/paths, account identifiers, volume GUIDs and raw errors.
An authenticated older v1 service can provide read-only status despite a build mismatch;
search and rebuild still require the matching build. Older services may lack stage
fields, which are explicitly unavailable rather than guessed. The remote trial's C/E
failure has not been reproduced on the development computer.
