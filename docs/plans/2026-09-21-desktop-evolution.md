# Dao-Shell desktop evolution

Status: complete on 2026-09-22. Authorized on 2026-09-21.
Stay a small LLM-first computer entry.

## Milestones and acceptance

- [x] Core: rename cli/mod.rs to cli.rs; separate provider configuration, model transport and settings service; preserve legacy configuration.
- [x] Runtime: move execution ownership out of capability definitions, make the write journal optional for read-only sessions, extract one-time confirmations from Tauri. Test isolation, expiry, cancellation and writes without a journal.
- [x] Access: restricted mode by default, explicit full access within OS authority; separate search defaults from accessible paths, preserve confirmations and file identity checks. Test mode switching and default compatibility.
- [x] Desktop settings: Noval-inspired navigation, multiple provider connections and model choices, masked credentials, synthetic connection check, save validation, explicit scope and appearance settings. Configuration changes only between requests; keep session when navigating.
- [x] Files: names and distinguishing paths instead of visible object numbers; preserve internal IDs and CLI shortcut compatibility.
- [x] Home: local timestamped system / CPU / memory / disk overview; network counters and installed-app coverage where available. No automatic model requests or mutation; manual refresh and honest partial/unavailable states.
- [x] Verification: root and desktop fmt/tests/strict Clippy, frontend checks and native UI fixture for settings/search/cancel/confirmation/navigation/overview. No real credentials or user-file mutations during tests.
- [x] Delivery: update English/Chinese README, architecture and trial instructions; commit scoped changes, push, inspect CI. Record limitations and stop continuation automation only after completion.

## Decisions

- Full access changes path scope, not Windows elevation or confirmation policy.
- Desktop continues to expose search/metadata/open only. Existing CLI moves remain guarded and journaled; no new delete/overwrite capability.
- Full access does not scan all disks by default. Search starts at common user locations and accepts explicit locations within the granted scope.
- Settings metadata contains credential references only; keys stay in the OS credential store or process memory. Never return saved keys to UI.
- No daemon, goal planner, coding agent, automatic cleanup, GPU sensor project or inventory integration in this iteration.
- Repeat check every 5 hours in this task: automation id `dao-shell`. Do not duplicate running work; resume incomplete interrupted work when available; disable when all acceptance items are met.

## Evidence

Baseline: commit 1f01697. Implementation complete; 66 root tests pass / 1 ignored,
1 desktop admission/configuration regression passes, 3 DOM behavior tests pass.
Actual local computer overview sampling passes without contacting a model.

Implementation commit `8e3d924` was pushed to main. Windows, Ubuntu and desktop
[CI passed](https://github.com/kestiny18/Dao-Shell/actions/runs/35595925237).

After the initially locked desktop became available, native Windows acceptance
completed on 2026-09-22 using `.tools/desktop-evolution-fixture/config.json` and
`scripts/desktop-fixture.mjs`. The child process alone received the configuration
override. No cloud credentials, paid requests or user-file mutations were used.

- Local overview, application-list expansion and explicit snapshot explanation.
- Model connection/tool-call test, harmless label save and persistence on restart.
- Access/general settings layout; mode semantics remain covered by core tests.
- File names/paths without internal numbers, opening confirmation and rejection receipt.
- Navigation preserves the conversation; empty results clear candidates; Stop restores input.
- Fixed feedback placement beside test/save actions and outer-window overflow when
  expanding application details; rebuilt and rechecked the native window.

The isolated app and model fixture were stopped after acceptance. Real cloud-model
quality, clean-machine distribution and installers are outside this iteration.
Pause heartbeat `dao-shell` after the final delivery check; there is no unfinished
milestone requiring automatic continuation.
