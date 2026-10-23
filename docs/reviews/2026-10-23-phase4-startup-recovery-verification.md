# Independent targeted recovery test result

Date: 2026-10-23. Candidate before/after: `81c5ed2f53437f682ca815cd6b95c24de83217fe`.
Baseline: `0c00e70fe067f1e7971a8cc810f0ae82e6467752`.
Result: targeted isolated checks PASS; no reproduced blocking finding. This does not accept the whole phase or real system operations.

## Acceptance mapping

| Item | Result and evidence |
| --- | --- |
| Store disable/restore persists both paths, frozen identity and digest before move | PASS, static composition: `startup_change` forms startup_proof before `local_transition`; engine persists dispatching before dispatch; both directions share this path. Dynamic engine/file tests validate persistence, not a substituted Store implementation. |
| Disable final save failure / child exit and reopen without replay | PASS, independently executed candidate software tests: oversize final save, exit(91), persisted proof and unknown reconciliation. |
| Restore final save failure / child exit | PASS, new independent SQLite BEFORE UPDATE RAISE(FAIL) fixture and separate child exit(93) after restore move. Persistent pre-move proof survives; reopening observes placement, historical attribution stays unknown and automatic_replay=false. |
| Reliable two-end check and fresh-confirmation restore | PASS, software final-save test refusal leaves retained file, subsequent new approval restores. Static Store composition calls check_startup, requires lifecycle checked_retained and retained_matches, creates fresh opaque ref; startup_change consumes that pinned object via a new confirm_maintenance. |
| Conflict, replacement, content drift, wrong root and both absent | PASS, candidate software tests executed independently, all keep unknown/gate. Unmoved original is observed as original without replay. |
| IO failure and retained ownership failure | PASS, new independent share_mode(0) locked retained file and foreign owner marker keep unknown/gate; repairing only fixture marker permits reliable observation. |
| Software unknown remains blocking | PASS, candidate crash test preserves separate install unknown and rejects another transition. Old independent provider false-success, idle/reboot and persistent unknown regression PASS. |
| Proof retained through cleanup | PASS, journal tests plus new old-dated startup_restore checked_retained fixture survive Journal reopen automatic pruning and explicit prune(true). Candidate final-save test checks disable+restore dual receipts become checked_original and both can then be pruned. |
| Bilingual observation/history UI | PASS, all 75 UI assertions; static bilingual strings state placement, unknown previous outcome and new confirmation/no replay. History maps checked lifecycles to unknown rather than success. |
| Native Store full path with temporary Startup root | NOT_VERIFIED dynamically: Store uses fixed current-user known folder and offers no fixture root. Production Store/Engine/targets/pinned-relocate composition checked statically; no real Startup touched. |
| Actual provider/install/UAC, real Startup/DNS, full WebView/native crash/reparse matrix, real disk power loss | NOT_VERIFIED, outside authorized isolated fixture scope. SQLite failure models a real database write rejection, not physical power loss. |

## Commands and exact counts

All commands from E:/WorkSpace/Dao-Shell, compiler commands sequential. No all-targets repeat: focused changed core, journal, affected session/protocol, independent regressions and desktop were sufficient for this narrow fix.

1. `./scripts/dev.ps1 test -CargoArgs @('--locked','software::','--','--nocapture')`: 25 passed, 0 failed; software.log. Other targets run zero filtered tests, not counted.
2. `node --test desktop/tests/ui.test.mjs`: 75 passed, 0 failed; ui.log.
3. `./scripts/dev.ps1 test -CargoArgs @('--locked','--test','phase4_recovery_independent_81c5ed2','--test','maintenance_session','--test','dialogue_protocol','--test','phase4_independent_7884984','--','--nocapture')`: protocol22 + session1 + old independent4 + initial new independent4 = 31 passed, 0 failed; integration.log. Old expiry test ran once, 90.12 seconds. New helper is counted once by libtest as a no-op without environment; its actual child run must exit93 and is checked by the parent test, not an additional passed-count.
4. `./scripts/dev.ps1 test -CargoArgs @('--locked','--test','phase4_recovery_independent_81c5ed2','sqlite_final_update_failure','--','--nocapture')`: 1 passed, 4 filtered, 0 failed; sqlite-failure.log. Added after initial integration run to independently test SQLite rejection rather than only oversize evidence.
5. `./scripts/dev.ps1 test -CargoArgs @('--locked','application_actions::journal::','--','--nocapture')`: 2 passed, 0 failed; journal.log.
6. `./scripts/desktop.ps1 test`: 24 passed, 0 failed; desktop-test.log.
7. `./scripts/desktop.ps1 build`: PASS, desktop-build.log (10.19 seconds).

Total observed passed counts: core/integration 59 + desktop24 + UI75 = 158. No repeated test passes are added from child helper invocations. No failures. Candidate existing tests were independently executed here; prior developer self-test results were not counted as independent evidence.

## Writes and identity

Only authored test: `tests/phase4_recovery_independent_81c5ed2.rs`, final formatted SHA256 `796C6E4E6043E151E41151CA1DFEF9406C57552B694513FEB03B14110D058EA9`. Ran rustfmt directly on this test only after compilation; no semantic change.
Evidence authored under `.tools/phase4-recovery-independent-81c5ed2/`: this report, software.log, ui.log, integration.log, sqlite-failure.log, journal.log, desktop-test.log, desktop-build.log, store-ui-diff.txt. Normal build outputs under target/ and desktop/src-tauri/target/. Native file tests used automatically cleaned tempfile directories only; actual child fixtures share their parent's temporary root.

Initial dirty paths: modified docs/plans/2026-10-17-p15-release-0.1.0.md, untracked docs/plans/2026-10-20-v0.1.0-independent-verification.md, docs/reviews/2026-10-20-v0.1.0-independent-verification.md, docs/plans/2026-10-23-phase4-startup-recovery-fix.md. Parent independently added docs/devlog/2026-10-23-phase4-startup-recovery-fix.md during testing. These were left untouched. Final tracked diff still only initial modified release plan; candidate HEAD unchanged. Baseline-to-candidate diff remains 7 files, 361 additions/5 deletions.

No production source, existing tests/configuration/docs changed by Tester. No commit, child agent, installation, elevation, external model, real Startup/software/DNS mutation, push or release.

## Independent review and orchestrator decision

Independent review: NO_BLOCKING_FINDINGS. R1: CLOSED.

Reviewer以空白上下文先独立检查候选7文件差异和调用链，再核对Tester报告和新增故障夹具，与测试结论无分歧。HEAD始终为81c5ed2，源码未变更，Reviewer未写入文件或运行写工作区命令。

关闭依据：store.rs:349在禁用/恢复两方向构造完整startup_proof，Engine先持久保存dispatching再移动；engine.rs:35只核对当前位置并保留unknown历史归因与automatic_replay=false；startup.rs:9仅接受明确NotFound，不将IO失败当不存在。store.rs:268以checked_retained及再次身份/摘要匹配生成新引用，仍需新本地确认。journal.rs:81保留checked_retained/unknown证明，可靠观察回original的记录可清理且不再次认领新文件。旧verified证明兼容、软件unknown门禁及UI无成功误报均已检查。

没有发现有充分证据的新阻塞问题。SQLite最终UPDATE拒绝、恢复后子进程退出、独占读取拒绝、所有权冲突及清理夹具与静态实现一致；明确区分Engine/文件动态覆盖与Store静态组合，不冒充固定Known Folder原生入口动态验证。

Implementation: READY（仅R1修复范围）；Self-test: PASS；Independent testing: PASS（定向隔离范围）；Independent review: NO_BLOCKING_FINDINGS。Whole phase remains IN_PROGRESS, real system operations NOT_VERIFIED.

未覆盖真实Startup/DNS/软件装卸、provider/UAC/重启、完整原生崩溃/重解析点矩阵及真实断电。旧便携包phase4-trial-62bec9b未含修复，不用于本轮R1验证；本轮交付源码与独立报告。桌面Debug构建SHA256：0FB5B5AC7C405A0D1FAEBE5F59D415F9490473427C5841C2114407D6B3A3F3B7。
