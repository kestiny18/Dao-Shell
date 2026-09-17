# Dao-Shell

[简体中文](README.md) · [English](README.en.md) · [Documentation](docs/README.en.md)

**Use your computer by saying what you want to do.**

Dao-Shell is a lightweight, LLM-first shell. It helps you find files, inspect a proposed operation, make a local decision, and see what actually happened. The project is currently a Windows CLI development preview; the command is `daosh`.

![Dao-Shell interaction concept: find files in natural language, review a move plan, and confirm before execution.](docs/assets/dao-shell-preview.svg)

_This is an interaction concept, not a screenshot. Dao-Shell does not have a desktop UI yet._

## What it can do

- Find files by name and metadata, then refer to earlier results.
- Prepare a small same-volume file move, show the exact plan, and wait for local confirmation.
- Sample CPU, memory, disk, and process usage and ask the model for an explanation.
- Keep direct search and resource commands available without a model.

The first release deliberately excludes arbitrary command execution, autonomous background work, long-running goals, and a general coding-agent workflow.

## Current status

Dao-Shell has working code and deterministic tests, but it is not a stable end-user release. File search, guarded moves, resource sampling, and the natural-language tool loop are implemented. Real-model usability and clean Windows distribution still need broader validation. See the [implementation record](docs/IMPLEMENTATION.md) for evidence and limitations.

## Build and run

Install the Rust toolchain selected by `rust-toolchain.toml`, then run:

```powershell
git clone https://github.com/kestiny18/Dao-Shell.git
Set-Location Dao-Shell
cargo test --locked --all-targets
cargo build --release --locked
.\target\release\daosh.exe --help
```

The package remains named `dao-shell`; the executable and user-facing command are named `daosh`.

For configuration and a safe first trial, follow the [getting-started guide](docs/GETTING_STARTED.md) (Chinese). Contributors should read the [development guide](docs/DEVELOPMENT.md) and [contribution guide](CONTRIBUTING.md). The [English documentation index](docs/README.en.md) explains the repository structure.

## Build in public

The roadmap follows real usage rather than a promise to become a universal assistant. The most useful contribution is a concrete scenario: what you wanted to do, how you do it today, and which step was frustrating.

[Open an issue](https://github.com/kestiny18/Dao-Shell/issues/new/choose) · [Roadmap](ROADMAP.md) · [Development log](docs/devlog/README.md) · [License](LICENSE)

Maintained by [kestiny18](https://github.com/kestiny18). Copyright 2026 kestiny18. Licensed under Apache License 2.0.
