# Roadmap

Current as of 2026-10-15. Priorities follow trial feedback, not delivery-date promises. [中文项目入口](README.zh-CN.md) · [Changelog](CHANGELOG.md)

## Implemented preview capabilities

- Windows desktop with an overview, remembered session tabs/history and model/access/theme/language settings.
- English and Simplified Chinese desktop; system UI default plus saved override. CLI messages remain Chinese.
- Natural-language file tools and filename fallback; current candidate clicks use the OS default application directly, with scope and identity checks.
- CPU/memory rankings together, responsive disk cards, registered apps, local app actions and a compact computer profile.
- Explicitly confirmed ending of verifiable current-user process instances; no model process-termination tool.
- Directory snapshots and a Windows index service for supported fixed NTFS name discovery, selected automatically in desktop Full access when a matching component is installed.
- Separate CLI with guarded same-volume moves, cancellation and durable operation receipts.
- A local small WebView2-bootstrapper installer and bilingual trial guides. No installer is uploaded by repository preparation.

These are prototype capabilities, not completed independent acceptance. The [desktop reference](desktop/README.md), [plans](docs/plans/README.md) and [development records](docs/devlog/README.md) separate code, self-tests and real-machine evidence.

## Current priorities

1. Diagnose persistent C/E indexing failures on another trial computer using the safe report; the cause is still NOT_REPRODUCED locally.
2. Complete missing Windows service installation/update/rollback/repair/uninstall and reboot/recovery evidence through explicitly authorized trials. Do not retry canceled elevation automatically.
3. Validate clean-machine installation, missing-WebView2 network/offline cases and representative English Windows environments. Local builds and synthetic model checks do not prove these paths.
4. Gather real-model compatibility and everyday file-search/resource-explanation feedback, including costs and failures. Keep the original broader scenario/move acceptance gaps visible.
5. Rebuild a traceable preview from the final repository commit before a public release; attach checksum and verification limits.

Query-time acceleration, name caches/FTS and collector-algorithm tuning are deferred in favor of functional completeness. Index coverage does not grant reading/opening/writing authority. Background index maintenance does not imply an autonomous background agent.

## Later exploration

Other operating systems remain behind platform boundaries; Windows is the only current desktop delivery target. Further app capabilities and deeper integration should follow concrete useful scenarios. A generic SDK, coding agent, long-running goals and plugin platform are outside the next step.

[Share a use case](https://github.com/kestiny18/Dao-Shell/issues/new?template=use_case.yml).
