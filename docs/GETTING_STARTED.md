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

Start the guided configuration:

```powershell
.\daosh.exe setup
```

Enter readable directories, writable directories, the full model endpoint, model ID, and the **name of the API-key environment variable**, not the key itself. Directories must exist. A blank directory answer preserves that category; new paths replace it; `-` clears it. Review the settings and enter `y` to save, or `:q` to cancel. Setup makes no model requests. Run it again to change your settings.

Alternatively, create a trial folder and configure access with commands:

```powershell
$demo = Join-Path $env:USERPROFILE 'Documents\DaoShellDemo'
New-Item -ItemType Directory -Force -Path $demo | Out-Null
.\daosh.exe config add-read "$env:USERPROFILE\Downloads"
.\daosh.exe config add-write $demo
```

Writable directories are also readable. A move's source and destination must both be within writable scope. Startup options `--read-root <directory>` and `--write-root <directory>` add scope for that run only. The model cannot expand these permissions.

You can try search and resource sampling before connecting a model:

```powershell
.\daosh.exe search contract --in $demo
.\daosh.exe resources --sample-ms 2000 --limit 10
```

`--in` allows reading for that invocation only; it does not grant write access. Copy a few disposable sample files into the trial folder before testing moves.

## Connect a model

Dao-Shell uses a Chat Completions compatible endpoint with function calling. Compatibility varies between providers and has not been verified for every service.

Replace the placeholders below. The endpoint must be the **full request URL**, including `/chat/completions`; use your provider's actual model ID.

```powershell
.\daosh.exe config model --endpoint 'https://your-provider.example/v1/chat/completions' --model 'your-model-id'
$secret = Read-Host 'API Key' -AsSecureString
$env:DAO_SHELL_API_KEY = [Net.NetworkCredential]::new('', $secret).Password.Trim()
.\daosh.exe doctor --check-model
.\daosh.exe
```

`API Key` is just the input label: paste the key at the prompt that appears after running the command. The environment variable lasts for this PowerShell process and its children; set it again in a new terminal. If you chose a different variable name during setup, use that name instead.

Remote endpoints require HTTPS and a key. A local endpoint such as `http://127.0.0.1:<port>/v1/chat/completions` can work without a key. Configuration is validated before saving. Leading and trailing key whitespace is trimmed; invalid characters or internal whitespace are rejected before sending a request, without printing the key.

Keys are read from environment variables, not written to configuration or operation records. Normal conversations send your input, selected file names and metadata, and resource samples to the configured provider. This version does not read file contents, process command lines, or process environments. Exiting ends the session; no task continues in the background.

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
