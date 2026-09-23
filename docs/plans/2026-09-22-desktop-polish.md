# Desktop usability follow-up

Status: complete 2026-09-24. Authorized 2026-09-22; continuation requested 2026-09-23.

- [x] Explicitly use the existing D/chevron PNG and ICO in bundle configuration.
- [x] Move Settings to the bottom of the sidebar.
- [x] Add a bounded draggable divider, keyboard controls and remembered width.
- [x] Add application cards/list switching, local resource icons and initials fallback.
- [x] Simplify service setup to DeepSeek / OpenAI / Custom with model choices and automatic official URLs.
- [x] Finish native verification of service/model selection, saved view/width and per-page scroll restoration. Use an isolated fixture; never edit real model configuration.
- [x] Run relevant final checks, record evidence, commit/push scoped changes and inspect CI.
- [x] Pause the five-hour heartbeat after all work and validation are complete.

## Checkpoint

Implementation was committed and pushed to `main` as `9fc7d37`. Root tests (68 passed,
one intentionally ignored), root strict Clippy and format checks, desktop bridge
test/Clippy/build, and five DOM tests passed. The final DOM run includes per-page
scroll restoration and automatic credential references for custom services.

[CI run 35876752432](https://github.com/kestiny18/Dao-Shell/actions/runs/35876752432)
passed for this commit: desktop, Windows tests and Ubuntu tests. Delivery and CI
were verified on 2026-09-24.

Native Windows checks on 2026-09-22/23 verified bottom Settings, divider dragging,
cards/list switching, real app icons, all three service options, DeepSeek default
selection, switching to OpenAI with automatic model selection, and other-model
entry. Normal restart retained sidebar width and list view. Navigating from a
scrolled settings page back to Home restored Home's own position. Task-owned
windows and fixture service are closed. No installer has been generated; this
task configures packaging icons only.

Isolated UI fixture: `node scripts/desktop-fixture.mjs .tools/desktop-polish-fixture`.
Launch with its regenerated `config.json` through child-only
`DAO_SHELL_DESKTOP_CONFIG`, then stop the task-owned app and fixture after checks.
Do not trust old process IDs after an interruption.

## Continuation

Heartbeat `dao-shell` was paused on 2026-09-24 after verified delivery. All five
items and their validation are complete; no automatic continuation remains.
