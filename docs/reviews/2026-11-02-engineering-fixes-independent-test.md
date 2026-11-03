# Independent tester evidence (2026-11-02)
Candidate: 41fc4cc0fbacf2202cca9d9e9162cc818a73a41a plus frozen uncommitted source.
Fingerprint: 643439CCCF4DB0E8C7C2CFCF4DD15D506820BFC7F0CDBDF1BCAFF59983E1323E
Conclusion: PASS for the assigned automated/isolated scope; no blocking implementation findings. Recommend generating the current-source private trial package, subject to the existing checked-source packaging gate. This is not real-machine acceptance or release approval.

| Item | Result | Evidence |
|---|---|---|
| F01 clean launch environment | PASS | rust-tests.log: synthetic EXE launched through CLI ordinary-open, fake configured key/product prefixes removed, SystemRoot retained; user-token clean environment tests and desktop legacy actions unit tests pass; legacy/direct association routes inspected |
| F02 no overview tools/history | PASS | independent-probe-final.log adversarial_overview test: prior history/private PID/path/app markers absent, exactly two current messages, tools empty, model tool_calls rejected, no follow-up HTTP |
| F03 bounded uncertainty/recovery | PASS | Rust engine/store/recovery tests; independent maintenance probe: same package case/version/scope gate, unrelated package/DNS permitted, legacy boot baseline, same boot/busy provider remain awaiting, new boot+idle becomes checked_unknown with attribution unknown/no replay, survives reopen |
| F04 multi-instance consistency | PASS | existing cross-process lock/external edits test and independent probe lock exclusion/release; failure baseline and old bytes preserved |
| F05 recoverable history lifecycle | PASS | UI82 includes active deletion wait/late callbacks, delete save failure and queued snapshots, 100 session reuse, 2000/8MiB early gating, oversized export; independent exact 2000 and 8MiB boundary, plus one rejected/preserves bytes, draft export, removal/save recovery |
| F06 local process-run budget | PASS | tests/model_budget.rs exercises max_tokens/max_completion_tokens, reported/missing/error/cancel reservations and recreated-session exhausted guard; independent prior usage + malformed toolcall reserve + reset + recreated context pre-HTTP guard |
| F07 provider isolation | PASS | Rust software::isolated tests enforce stuck timeout/cancel, output limit, descendant handle nonblocking completion and closed request schema; production 30/900s constants inspected; no real WinGet write |
| F08 package evidence | PASS | package-process.log, pwsh -File test-package-source.ps1 exit0; missing/stale/failed checks/ignored source mutation reject, matching/unrelated-doc snapshot accept; production call sites inspected |
| F09 dependency scans | PASS with explicit exceptions | core-audit.json and desktop-audit.json exit0 using cargo-audit0.22.2 and advisory DB99670cfc0bc574433034b75876a4b791ccaa2efb; npm-audit.json exit0; glib-windows-tree.log empty Windows GNU dependency path; documented RUSTSEC-2024-0429 Windows-only exception and maintenance notices remain visible; security CI/dependabot inspected, hosted execution not verified |
| F10 current version docs | PASS | README.md/README.zh-CN.md/ROADMAP.md/desktop README/DEVELOPMENT/DEPENDENCY_SECURITY read; prototype/public distinction, state recovery/budget/history/real-machine limits explicit |

Commands executed:
- scripts/dev.ps1 test -CargoArgs @('--locked','--all-targets'): exit0, 298 passed / 4 ignored.
- scripts/desktop.ps1 test: exit0, 26 passed.
- npm test --prefix desktop: exit0, 82 passed.
- pwsh -NoProfile -File scripts/test-package-source.ps1: exit0.
- scripts/dev.ps1 test -CargoArgs @('--offline','--manifest-path','.tools/independent-2026-11-02/fixtures/probe/Cargo.toml','--target-dir','target','--','--nocapture'): final exit0, 3 independent probes passed.
- cargo-audit two locked manifests, --no-fetch --deny unsound, desktop explicit --ignore RUSTSEC-2024-0429: both exit0. Scanning uses the recorded advisory DB snapshot, not a claim of live/latest database.
- npm audit --prefix desktop --json --ignore-scripts: exit0.
- scripts/dev.ps1 tree for desktop --locked --target x86_64-pc-windows-gnu -i glib: exit0, no path.

Probe development: initial fixture dependency path had one excess parent and was corrected. Two initial protocol assertions incorrectly demanded omitted tools/tool_choice=none; actual safe protocol sends tools=[]/tool_choice=auto. Test assertions were corrected to the acceptance behavior: no exposed executable tools, no history/private fields, rejecting returned tool calls/no second request. These were tester fixture errors, not implementation failures; original test sources/production were untouched.

NOT_VERIFIED: real document association handlers/UAC, paid/real model compatibility/billing, real WinGet writes/catalog recovery and reboot, clean-machine installer/service lifecycle, extended usage/performance, hosted CI. Four intentionally ignored Rust cases remain excluded (fixture-only helpers and explicit load/native prerequisites); no inference to real-machine acceptance.

Scope of writes: only .tools/independent-2026-11-02 acceptance/evidence/fixtures and allowed target build output. No production/original test changes, commits or publishing.
Final identity check: all 311 frozen manifest entries match, HEAD unchanged, fingerprint unchanged.

Independent review: NOT_RUN (not requested).
Evidence directory: .tools/independent-2026-11-02. Frozen source manifest and full source copies retained there. Packaging is a separate subsequent step; no installer was executed by the Tester.
