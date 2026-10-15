# P14 — Repository preparation and behavior preservation

Plan: [P14](../plans/2026-10-15-p14-release-repository-preparation.md)
Baseline: ca051de6cb9517b6285a4dbeed33206786c0e956 plus all existing P9–P13 changes; 216 non-ignored file hashes in `.tools/p14-20261015/baseline.json`.
Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN

## T1 — Comments without CLI behavior changes

Only `src/cli/args.rs` (10 comments) and `examples/inventory_spike.rs` (5 comments) change source. Chinese Clap documentation comments become English comments with explicit original Chinese `help`/`about` metadata. No action, argument type/default, permissions, models or runtime control changes.

Before editing, built current CLI/example and captured 40 root/subcommand/experiment `--help`/`-h` cases. Rebuilt after editing and compared stdout bytes, stderr bytes and exit codes: PASS, all exactly equal. The built-in `help --help`/`help -h` forms retain their original exit 2; the initial fixture's assumption that every help form exits 0 was corrected before baseline capture, without changing product behavior. Clap exits before CLI configuration/action dispatch; no real scan, model call or file action was executed. Evidence: `.tools/p14-20261015/help-before.json` and `help-after.json`.

## T2 — Current entry points and preserved history

- Updated English/Chinese README and getting-started paths to distinguish bilingual Windows desktop from Chinese CLI, persistent visible sessions from replayable runtime state, default Full access index prerequisites from compatible-folder fallback, explicit process confirmation and direct clicked-file OS opening from model-requested confirmation. Desktop moves remain unavailable; guarded CLI moves remain available.
- Kept the SVG labeled as a concept rather than a running screenshot. No synthetic images added as product evidence.
- Updated current roadmap/Unreleased changes/contribution and engineering navigation, with user/engineering/history layers. Added Plan/experiment index pages and recent-checkpoint links to the existing devlog index.
- Historical Plans, individual devlogs, experiment reports, MVP design and implementation baseline remain at their original paths and retain their original states/evidence. Navigation explicitly dates old design/evidence instead of treating it as current acceptance.
- Kept another computer's C/E issue NOT_REPRODUCED, P11 lifecycle/clean-machine gaps and deferred query optimization. No stable-release, public-upload or independent-acceptance claim.
- No code or research fixture deleted: references/build dependencies do not justify removing public APIs, experiments or unfinished P11 work.

Current P13 installer is retained with SHA256 `8A180F1B52B34D0D574B662B63F19B9458445CC42AE81B79619BFED9DA83C42A`; no packaging/installation repeated. Trial TXT resources and desktop configuration/UI/runtime files must remain byte-identical to baseline. A formal release should be rebuilt from the final repository commit.

## T3 — Checks and handoff

All checks below apply to the P14 working-tree candidate on 2026-10-15, based on the recorded baseline plus preserved P9–P13 work. No commit/stage/push/release is performed by Coder; main Agent owns explicit-path staging and local commits after this writer finishes.

| Check | Result |
| --- | --- |
| Core `dev.ps1 test --locked --all-targets --jobs 2` | PASS: 178 passed, 0 failed, 3 ignored across 16 test executables |
| Desktop `desktop.ps1 test --jobs 2` | PASS: 19 tests |
| UI `npm test --prefix desktop` | PASS: 60 tests |
| Core and desktop `fmt --check` | PASS |
| Core and desktop all-targets Clippy, jobs 2, warnings denied | PASS |
| `node --check` for app/home/settings/i18n/layout and inventory benchmark script | PASS; benchmark was not executed |
| Help stdout/stderr/exit comparison | PASS: all 40 cases unchanged |
| `git diff --check` | PASS; Git reports line-ending normalization warnings only |
| Relative Markdown file/image targets | PASS: 224 checked, 0 missing; remote URLs and anchors not verified |
| Baseline file scope/hash comparison | PASS: no deleted files or unauthorized runtime changes |
| P13 installer hash | PASS: retained SHA256 above |

The three ignored core tests are isolated child-process helpers for crash recovery/process tests and a manual CPU-load test. Existing automated tests use synthetic temporary fixtures and self-owned process helpers; no existing user process was terminated. Logs and one-off engineering helpers are under `.tools/p14-20261015/`: `core-tests.log`, `desktop-tests.log`, `ui-tests.log`, `core-clippy.log`, `desktop-clippy.log`, help snapshots and `repository-check.json`. These ignored helpers are evidence tools, not new repository tests.

The baseline comparison covers all 216 non-ignored files. Source changes made by P14 are limited to the two comment/Clap metadata files described in T1. All other source, desktop UI/configuration, packaging scripts/resources and both trial TXT guides retain baseline bytes. Documentation changes are limited to the current entry/navigation files listed in T2, new Plan/experiment indexes and this report; the main Agent separately owns the P14 Plan and desktop-trial Plan index update. There are no removals or moves. A source-comment scan found no remaining Chinese comment locations in the inspected Rust/JavaScript/PowerShell source; Chinese help/resource/user-fixture strings remain intact.

The prospective non-ignored-file safety scan found no token, private-key or literal credential candidates. Six SID-shaped strings were inspected and are synthetic unit-test inputs, not actual-user identity evidence: `src/file_index/worker/security.rs:332`, `src/file_index/worker/service/lifecycle.rs:256`, `src/file_index/worker/service/package.rs:443`, `src/file_index/worker/service/package.rs:499`, `src/file_index/worker/service/peer.rs:163`, and `src/file_index/worker/tests.rs:582`. Values are not reproduced here. This heuristic scoped check is not an independent security audit; ignored local data/configuration were not copied or scanned into the report.

Recommended minimum coherent local commit groups:

1. Complete current runtime, tests and required desktop/Cargo/permission/build/packaging resources, excluding the P14-only CLI help/comment file. Include both trial guides and index-payload preparation dependency. This preserves the interdependent P9–P13 implementation as one buildable unit.
2. Research examples and their complete path modules (`file_index_probe`, `ntfs_inventory_spike`, `ntfs_inventory/`) plus the desktop inventory benchmark script, depending on the runtime from group 1.
3. Current documentation/history navigation/Plans/devlogs and the two P14 comment/Clap metadata files. Their CLI output is already proven unchanged; they do not supply a runtime dependency to groups 1–2.

The main Agent will verify explicit staged manifests and compile a Git archive of the first runtime group without ignored payloads, then make authorized local commits. No push/publication or new package is authorized by this preparation. Coder has finished source/document writing at this checkpoint. Independent testing and independent review remain NOT_RUN; native P13 trials and service lifecycle evidence are not rerun or reclassified by P14.

## Main Agent — Local Git delivery

The preparation handoff above was followed by explicit-path staging and local delivery:

- Runtime: `4cb25f570424d780f414443706331841b230fc59`, 71 files. The staged path set exactly matched the recorded runtime manifest; cached whitespace and working-tree/index equality checks passed.
- Exploration: `472de19dcb2a78172a24b1f9f903db361f185f0b`, 5 files. The complete experiment path modules and benchmark script were staged explicitly; the same manifest/whitespace/equality checks passed.
- Documentation and the two help/comment files are delivered by the subsequent local commit containing this record. Commit messages are English; existing history is retained. The final commit identity and clean-state check are recorded in the local delivery response and ignored `.tools/p14-20261015/local-commits.json`, avoiding a self-referential commit hash in this file.

Exported runtime commit `4cb25f570424d780f414443706331841b230fc59` using `git archive` into a new `.tools/p14-20261015/runtime-checkout` directory. No ignored index payload or untracked exploration source was present. Using its manifests and existing dependency/build caches, `cargo check --locked --offline --all-targets` passed for the core, and `cargo test --locked --offline --jobs 2` passed all 19 desktop tests. `DAO_SHELL_REQUIRE_INDEX_PAYLOAD` was unset for this source-only check; this is not a release-payload packaging or installation test. Evidence: `runtime-archive.json`, `archive-check.json`, `archive-core-check.log` and `archive-desktop-test.log` in the ignored P14 evidence directory. No service, elevation, installation or real user file/process action was performed.

The source archive check supplements the working-tree self-tests above by proving the committed runtime source is complete. The final comment metadata is covered by the original self-tests and exact help comparison; final record-only edits do not change executable behavior. Independent testing/review remain NOT_RUN. P11 completion, another machine's C/E failure, clean-machine/runtime installation and formal release artifacts remain outside this delivery's verified scope.

After updating the delivery records, the final scope/installer/safety comparison passed again; 225 local Markdown file/image targets were present, with no missing targets (remote URLs and anchors remain unchecked). The additional target is the Plan's link to this report. `git diff --check` passed, and the remaining explicit-path manifest matched the documentation/help group exactly.

When the previously untracked historical reports were staged, default `git diff --cached --check` reported two existing Markdown hard breaks at `2026-10-12-p11-2-index-worker.md:5–6`. Its bytes still match the P14 baseline; both line endings are exactly two spaces after text, with no other trailing whitespace. The historical report is preserved. The final staged check uses the default whitespace rules for every other file and permits end-of-line blanks only for that report via a command-local `core.whitespace=-blank-at-eol` setting; both checks pass. No repository-wide Git setting or source file was changed to suppress the check.
