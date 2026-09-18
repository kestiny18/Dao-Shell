# Try Dao-Shell

[English](GETTING_STARTED.md) · [简体中文](GETTING_STARTED.zh-CN.md)

This Windows CLI development preview is for early users willing to share feedback. The interface currently uses Chinese. Start with disposable files in a separate trial folder.

## Get the program

There is no official download release yet. With Git, Rust, and the appropriate Windows linker toolchain installed, build from source:

```powershell
git clone https://github.com/kestiny18/Dao-Shell.git
Set-Location Dao-Shell
cargo build --release --locked
Set-Location target\release
.\daosh.exe --help
```

The [development guide](DEVELOPMENT.md) (Chinese) describes the toolchain and a local Windows bootstrap option. Future downloads will appear on [GitHub Releases](https://github.com/kestiny18/Dao-Shell/releases), with their verification scope stated.

Run the examples below in **PowerShell**, from the directory containing `daosh.exe`. If your prompt is `C:\...>` instead of `PS C:\...>`, enter `powershell` first. CMD handles single quotes differently.

## First setup

Run the program. If no model is configured or no usable key is available, it opens a terminal setup guide automatically:

```powershell
.\daosh.exe
```

1. Choose DeepSeek, another compatible service, or a local model. DeepSeek prefills its endpoint and `deepseek-flash`; the model ID is editable. Custom services accept a full endpoint or a service `/v1` URL; setup appends `/chat/completions` when needed.
2. Paste your API key with hidden input. No environment-variable name is required. Press Enter to reuse an existing key; a local service can work without one.
3. Choose whether to verify the connection. This sends up to two synthetic requests, may incur provider charges, and reads no local files. On failure, change the key, endpoint, or model in place, or retry.
4. For a newly entered key, choose session-only use (the default) or secure storage in Windows Credential Manager. Saving it there enables reuse after restarting.
5. Select Downloads, Documents, or custom search directories. Setup adds no write access. Existing write scope is shown and preserved; unavailable old scope can be cleared.
6. Review the settings and enter `y` to save and continue directly into `dao >`.

Use `:q`, or Ctrl+C followed by Enter, to cancel. Configuration and stored keys are unchanged until you save. Invalid directory choices can be corrected in place. You can skip the model or connection check, but skipping does not establish that a model works. Resource sampling remains available without search directories.

To change settings later:

```powershell
.\daosh.exe setup
```

Saved Windows keys belong to the current user on this computer and are bound to the full endpoint and environment-variable name. Changing an endpoint does not implicitly reuse its saved key. Keys never enter JSON configuration or operation records. Lookup priority is the key entered in this process, an existing environment variable, then Windows Credential Manager. Session-only input leaves older saved credentials unchanged. To remove a saved key, delete its `Dao-Shell/model/` entry under Windows Credentials → Generic Credentials. Deleting a configuration file does not delete credentials.

If secure storage fails, setup explicitly reports that the key remains session-only and continues the current session. Other platforms currently support session-only keys and environment variables, without credential persistence.

## Manual configuration and file organization

Existing commands remain available for scripts and advanced configuration. Create a disposable trial folder and grant access:

```powershell
$demo = Join-Path $env:USERPROFILE 'Documents\DaoShellDemo'
New-Item -ItemType Directory -Force -Path $demo | Out-Null
.\daosh.exe config add-read "$env:USERPROFILE\Downloads"
.\daosh.exe config add-write $demo
```

Writable directories are also readable. Both source and destination of a move must be within writable scope; each move still requires confirmation. Copy a few disposable files into the trial folder first.

Without a model, you can use:

```powershell
.\daosh.exe search contract --in $demo
.\daosh.exe resources --sample-ms 2000 --limit 10
```

`--in` grants reading for that invocation, not writing. Startup options `--read-root` and `--write-root` add scope for that run only.

Models use a Chat Completions compatible endpoint with tool calling; compatibility requires verification per service. Remote endpoints require HTTPS and a key; local loopback services may use HTTP without a key. Environment-variable configuration still works:

```powershell
.\daosh.exe config model --endpoint 'https://your-provider.example/v1/chat/completions' --model 'your-model-id'
$secret = Read-Host 'API Key' -AsSecureString
$env:DAO_SHELL_API_KEY = [Net.NetworkCredential]::new('', $secret).Password.Trim()
.\daosh.exe doctor --check-model
.\daosh.exe
```

That environment variable lasts for the current PowerShell process and its children. Normal conversations send user input, candidate file names and metadata, and resource samples to your provider. Dao-Shell does not read file contents, process command lines, or process environments. Exiting ends the session without background work.

## Start a conversation

The input prompt is `dao >`. The following Chinese examples mean: find contract files in the trial folder, open the second result, preview moving the first result into an archive folder, and explain current resource pressure.

```text
dao > 找试用目录中的合同文件。
dao > 打开第二个。
dao > 把第一个移到试用目录的归档文件夹，先给我看方案。
dao > 查看当前资源压力，解释可能原因。
```

English requests depend on your model and have not yet completed acceptance testing. CLI messages remain in Chinese.

Opening and moving show the concrete objects locally. A move plan shows sources, destinations, and directories to create, and requires `y` before execution. `/open 2` directly selects the second current result for opening.

Moves currently support 1–20 ordinary files on the same volume. Existing destinations are not overwritten; directory moves and following links are unsupported. Cancellation stops later steps and does not undo completed changes. An accepted open request means the operating system accepted the handoff, not that the application has displayed the document.

## Useful commands

| Input inside `dao >` | Action |
| --- | --- |
| `/search contract` | Search directly without a model request |
| `/results` | Show current candidate numbers; an empty search retains the previous group with a notice |
| `/open 2` | Open the second current result |
| `/move 1,2 C:\full\destination` | Show a move plan and wait for local confirmation |
| `/resources` | Sample resource usage |
| `/history` | Show local operation receipts |
| `/reset` | Clear the conversation and candidates, keeping operation records |
| `/quit`, `quit`, `exit`, `退出` | Exit |

These commands remain available after a model connection failure. Natural-language requests are not silently converted to keyword searches. `Ctrl+C` requests cancellation; when waiting for input or confirmation, also press Enter to return.

Run these commands from PowerShell, outside the conversation:

```powershell
.\daosh.exe doctor
.\daosh.exe config show
.\daosh.exe history --reconcile
.\daosh.exe history --clear
.\daosh.exe search --help
```

`config`, `setup`, and `doctor` are terminal commands. If entered into the conversation, they produce guidance rather than being forwarded to the model.

By default, `doctor` checks local configuration without model requests. **`doctor --check-model` sends up to two requests and may incur provider charges.** It uses a fixed synthetic example to check tool calling and result submission, without reading local files, directory contents, resource metrics, or operation records. It reports elapsed time and provider-reported token usage when available. Passing this check does not verify real file workflows.

Diagnostics distinguish invalid key characters, HTTP authentication or permission errors, endpoint errors, rate limits, server failures, connection failures, and timeouts. A connection failure does not establish a particular DNS or TLS cause.

Operation records live under `%LOCALAPPDATA%\Dao-Shell`. Completed records are retained for seven days; unresolved records are kept. Restarting checks known objects without replaying the previous operation. Clearing history requires local confirmation.

## Limits and feedback

Search uses names and metadata only. A scan that hits time or count limits reports incomplete coverage. Resource readings are short samples, not proof of a slowdown's root cause; a few listed processes do not explain all memory usage.

An initial Windows trial with DeepSeek covered search, follow-up references, and PDF opening. Full acceptance across the 15 fixed scenarios, small file moves, and a clean Windows environment is still pending.

[Report an issue](https://github.com/kestiny18/Dao-Shell/issues/new/choose) with reproduction steps and sanitized errors. Do not include API keys or private file contents.

[Back to the project](../README.md) · [Verification record](IMPLEMENTATION.md) (Chinese)
