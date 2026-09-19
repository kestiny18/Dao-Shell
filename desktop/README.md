# Dao-Shell desktop preview

A small desktop entry for finding and opening your files with natural language.
This is a Windows development preview, not a packaged release.

## Try it on Windows

From the repository root in PowerShell:

```powershell
# Reuse your existing Dao-Shell configuration, or configure it first:
.\scripts\dev.ps1 run -CargoArgs @('--', 'setup')
.\scripts\desktop.ps1 run
```

The window reuses the CLI model configuration, credential lookup and selected
directories. Restart it after changing configuration. A key stored only in a
previous CLI process is not available to the desktop process; use the existing
Windows credential option or the configured environment variable.

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
- Existing configured directories only; no desktop file move/write tools.
- Session-local candidates, model history, cancellation and one-time confirmation.
- Existing production search, not the experimental inventory backend.
- Setup still uses the CLI. No tray service, global shortcut, installer or background agent.

The frontend receives only session information and file results, never API keys.
Like the CLI, natural-language requests send the prompt and relevant tool results
(including file names, paths and metadata) to the configured model provider.
File contents are not read by these tools. Direct filename search stays local.

## Development checks

Close a running desktop build before rebuilding or checking it on Windows.

```powershell
.\scripts\desktop.ps1 fmt -CargoArgs @('--check')
.\scripts\desktop.ps1 test
.\scripts\desktop.ps1 clippy -CargoArgs @('--all-targets', '--', '-D', 'warnings')
.\scripts\desktop.ps1 build
node --check desktop/ui/app.js
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

See the [experiment record](../docs/experiments/desktop-entry.md) for boundaries and evidence.
