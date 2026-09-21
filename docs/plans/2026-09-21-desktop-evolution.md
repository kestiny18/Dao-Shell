# Dao-Shell desktop evolution

Status: in progress. Authorized on 2026-09-21. Resume from the unchecked tasks;
inspect the working tree before editing. Stay a small LLM-first computer entry.

## Milestones and acceptance

- [x] Core: rename cli/mod.rs to cli.rs; separate provider configuration, model transport and settings service; preserve legacy configuration.
- [x] Runtime: move execution ownership out of capability definitions, make the write journal optional for read-only sessions, extract one-time confirmations from Tauri. Test isolation, expiry, cancellation and writes without a journal.
- [x] Access: restricted mode by default, explicit full access within OS authority; separate search defaults from accessible paths, preserve confirmations and file identity checks. Test mode switching and default compatibility.
- [x] Desktop settings: Noval-inspired navigation, multiple provider connections and model choices, masked credentials, synthetic connection check, save validation, explicit scope and appearance settings. Configuration changes only between requests; keep session when navigating.
- [x] Files: names and distinguishing paths instead of visible object numbers; preserve internal IDs and CLI shortcut compatibility.
- [x] Home: local timestamped system / CPU / memory / disk overview; network counters and installed-app coverage where available. No automatic model requests or mutation; manual refresh and honest partial/unavailable states.
- [ ] Verification: root and desktop fmt/tests/strict Clippy, frontend checks and native UI fixture for settings/search/cancel/confirmation/navigation/overview. No real credentials or user-file mutations during tests.
- [ ] Delivery: update English/Chinese README, architecture and trial instructions; commit scoped changes, push, inspect CI. Record limitations and stop continuation automation only after completion.

## Decisions

- Full access changes path scope, not Windows elevation or confirmation policy.
- Desktop continues to expose search/metadata/open only. Existing CLI moves remain guarded and journaled; no new delete/overwrite capability.
- Full access does not scan all disks by default. Search starts at common user locations and accepts explicit locations within the granted scope.
- Settings metadata contains credential references only; keys stay in the OS credential store or process memory. Never return saved keys to UI.
- No daemon, goal planner, coding agent, automatic cleanup, GPU sensor project or inventory integration in this iteration.
- Repeat check every 5 hours in this task: automation id `dao-shell`. Do not duplicate running work; resume incomplete interrupted work when available; disable when all acceptance items are met.

## Evidence / next action

Baseline: commit 1f01697. Implementation complete; 66 root tests pass / 1 ignored,
1 desktop admission/configuration regression passes, 3 DOM behavior tests pass.
Actual local computer overview sampling passes without contacting a model.

Native UI validation attempted with `.tools/desktop-evolution-fixture/config.json`
and a local synthetic model; Windows was locked. The user was asked to unlock.
Do not mark native UI acceptance complete from DOM results. Resume with the
fixture (restart `node scripts/desktop-fixture.mjs .tools/desktop-evolution-fixture`
to regenerate the current port), launch the built executable with
`DAO_SHELL_DESKTOP_CONFIG` set only for that child, then inspect home/settings,
test the existing synthetic model connection, save a harmless connection label,
verify navigation/search/empty results/cancel/confirmation. Do not edit real
user configuration or use a paid provider in tests. Stop the fixture afterwards.

Finish delivery/CI and update this checklist. Keep automation `dao-shell` active
until native checks and delivery are complete; then pause it via automation_update.
