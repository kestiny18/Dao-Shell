# P16 — Windows system/cache exclusion prototype

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN

## Scope and source

The user approved Windows design and implementation after discussing default
exclusion of system internals and regenerable caches, preservation of user data
and an advanced inclusion path. The main Agent wrote
[P16](../plans/2026-10-20-p16-windows-index-exclusions.md), selected the design and
delegated to the existing single Coder. No independent roles were requested.

HEAD is `c7780ab4545028627d4e15de087165a6d9d21a43` plus the preserved, uncommitted
P12 bounded-budget implementation/tests/records and this task. The user supplied
AF860FBC evidence that P12 removed the remote C/E publication blockage; C remained
partially covered. That fact does not establish P16 behavior.

The public installer and the P12 trial installer were hash-checked before this
task. `.tools/p16-windows-exclusions-20261020/baseline.json` records the starting
state. This local work does not modify the public release or authorize service
management, real-volume indexing, file actions, migration or Git publication.

## Design decisions

- One active index per volume, with `windows_default_v1` and `windows_all_v1`.
  Advanced inclusion is an explicit per-volume Apply and Rebuild action, not an
  instantaneous query filter over data that was never collected.
- The service persists a fixed protected per-volume policy file. The authenticated
  operation accepts only the fixed inclusion boolean; no caller-provided path,
  database, SID, SQL or shell command is introduced.
- A snapshot's published policy is separate from the requested policy. Legacy
  snapshots retain a legacy label until an explicitly requested atomic rebuild
  publishes the new scope. Failed or cancelled builds preserve previous data.
- Rules locate the actual Windows directory using native APIs and bind directory
  roots to their canonical volume/directory identities without following reparse
  redirects. Paths are compared by directory boundaries, not arbitrary prefixes.
- Initial default roots: Windows WinSxS, servicing, Installer,
  System32/DriverStore/FileRepository, Temp and SoftwareDistribution/Download;
  each volume's System Volume Information and $Recycle.Bin. Dao-Shell's own index
  files remain excluded in both modes.
- No whole-folder exclusions for AppData, ProgramData, Program Files, user
  profiles or data drives; no blanket Windows/System32 exclusion. The first
  version does not guess user-profile or third-party caches. Application entries
  remain available through the existing catalog and preserved discovery paths.
- Cheap identity/link-count and canonical-path checks may skip expensive metadata
  reconciliation for provably excluded single-link objects. Multi-link objects
  verify their complete name set and retain included aliases. Unknown cases stay
  included or honestly unresolved.
- Ordinary included-to-included directory renames remain incremental. Boundary
  crossings, rule-root/ancestor changes or uncertain evidence that may affect
  coverage use bounded rebuilding rather than silently losing descendants. A
  full-volume parent graph cache and rebuilding on every user-folder rename were
  rejected as unnecessary or disruptive.
- Intentional exclusion is reported separately from access denial, sharing
  failures and unresolved metadata. Counts identify their units; they are not
  presented as unique excluded file counts or measured performance gains.

MFT/USN enumeration still visits volume records. This is a collection/scope feature,
not the deferred query-cache/FTS/SQL optimization or a new collector algorithm.
macOS and Linux implementations are outside scope.

## Verification and delivery

Implementation is frozen. Source is the baseline plus P12/P16 working-tree changes;
no commit, push or publication was performed. A transient Coder capacity error was
resumed at the same checkpoint, without a second writer or a model change.

The implementation adds a typed `Excluded` source outcome that removes stale
names atomically without treating deliberate exclusion as an access error. Repeated
excluded-only events do not publish empty generations. Existing budgets remain
bounded at 8x per attempt; two boundary-recovery retries add at most 16x to P12's
63x failure-episode bound (79x total initial work in the combined worst case).
This is a work-count limit, not a time, memory, unique-file or performance promise.

The fixed policy operation retains the existing installation-owner SID and
interactive-session authentication. Changing the scope clears service cursors and
desktop session candidates/confirmations, without changing Config or Scope. A
persisted, previously confirmed pending mode change resumes after simulated worker
restart. A legacy index with no saved explicit request waits for confirmation.
Searches during scope mismatch return a structured `policy_rebuild_required`
response rather than old-scope results. Preference-save failure is also structured.

### Implementation self-tests

All logs below are under ignored `.tools/p16-windows-exclusions-20261020/`.

| Check | Result and evidence |
| --- | --- |
| Core all-targets | 202 passed / 3 ignored helpers or opt-in resource test; library subset 124 passed / 2 ignored (`core-final.log`) |
| Final policy regressions after narrow follow-up fixes | 8 passed (`policy-final-followup.log`) |
| Desktop Rust, latest bridge/diagnostic cases | 22 passed (`desktop-final-followup.log`) |
| Full UI suite, final UI-only correction | 64 passed (`ui-first-publication-final.log`); prior full suite 63 passed (`ui-final.log`) |
| Native temporary-file exclusion cases | 4 passed (`exclusions-initial.log`) plus junction case passed (`junction-native.log`) |
| Core all-target Clippy, warnings denied | PASS (`core-clippy-frozen.log`) |
| Desktop Clippy, format checks and JavaScript syntax | PASS (`desktop-clippy-final.log` and Coder handoff) |

The last narrow backend fixes preserve the whole-suite result for unchanged paths
and are covered by final policy/bridge regressions and frozen-source Clippy. After
the initial package, a UI-only first-publication label correction received its own
targeted bilingual regression, full UI suite and JavaScript syntax checks. Rust
and worker sources did not change during this correction.

Native fixtures contain only temporary files and synthetic cache/project roots:
single-link early skipping, included aliases of cross-boundary hard links, missing
then created roots, root replacement, ordinary directory rename pairs and junction
redirects. They do not scan the real Windows directory or an entire user volume.
Synthetic maintenance covers actual store publication/rollback, mode changes,
failed saves, cancelled/duplicate/rate-limited requests, policy persistence and
pending-mode restart, legacy snapshots, bounded boundary recovery, stale-cursor
clearing and structured search/scope races. Diagnostics reject arbitrary policy
identifiers/categories and private paths. Chinese/English UI checks cover scope
selection, confirmation, failure, reported scope and candidate invalidation.

The initial UI run found a real form bug: dynamically added buttons defaulted to
submit, accidentally saving unrelated settings and clearing candidates after a
cancelled scope confirmation. Both scope/rebuild buttons now use `type=button`;
the affected regressions and final full UI suite pass. The failed initial log is
retained. Scoped wrapping styles keep long status, selection and action text in
separate rows without changing other folder lists; no full native visual acceptance
is claimed.

Final inspection also found that an unpublished first build was labeled legacy.
The UI now distinguishes no publication from a generation with no policy, including
the generation-zero edge case. Only settings.js, i18n.js and the UI regression
changed; the affected bilingual test and all 64 UI tests pass. The initial package
and evidence are retained under `candidate-v1/`, with `native-final/result.json`
recording its startup smoke. They are not the final trial artifact.

### Final local artifact

Package: `dist/Dao-Shell-0.1.0-p16-exclusions-20261020-windows-x64-setup.exe`,
9,163,863 bytes, SHA256
`515FC220757BB41CBE3CB8EE31DED47B51DC104953F17DE2BA37B38498EA8F36`.
Worker SHA256:
`436F4DA234B67FC0C25DE9CE06268B17548551E13AADBF9B504E525361693D14`.
Final desktop SHA256:
`778608204E3D7E49B4F448547280AF40E3EF67247FD04159DCC2542D6990A18F`.
The UI-only correction left the worker hash unchanged. The product version is still
0.1.0; this separate working-tree artifact is unpublished and has no release tag.

`package-final.log` records the successful necessary rebuild. Verification passed
for matching embedded worker bytes/digest, x64 GUI subsystem, allowed resources,
loader, bilingual guides/notices, small 1,860,304-byte WebView2 bootstrapper and
checksum. The final exact executable started with isolated restricted/no-model
configuration, loaded the expected loader and exited normally without changing
configuration (`native-final-v2/result.json`). This is startup/exit smoke, not full
native UI or installer acceptance. No service control, UAC or model request ran.

Build diff/untracked hashes and the final manifest are under
`.tools/p16-windows-exclusions-20261020/`. Later edits are delivery documentation
only. The candidate has its own filename/checksum. Public 0.1.0 was restored to
its standard dist filename and hash-checked against EF4C004F…FB6F; the prior P12
trial still matches 6E74007A…776A. Superseded candidate-v1 evidence remains separate.

Install/relaunch this candidate, explicitly update the component and verify actual
and expected builds both match 436F4DA2…. In File Search, choose Standard scope or
Include system internals and caches for each volume and confirm Apply and Rebuild.
Legacy snapshots require that explicit rebuild; scope mismatch temporarily blocks
indexed results until a matching publication. The settings note and receipt state
what is excluded, what remains unresolved and which scope is currently published.

Actual protected-system-directory trials, service upgrade/restart/rollback,
long-running resource impact and full machine coverage are not established by
synthetic tests or packaging. P11 remains IN_PROGRESS. No independent acceptance,
public release or complete-system coverage is claimed.

## User-machine checkpoint — 2026-10-20

The user supplied a running-service report with actual and expected builds both
matching `436F4DA234B67FC0C25DE9CE06268B17548551E13AADBF9B504E525361693D14`.

| Volume | Evidence in this checkpoint |
| --- | --- |
| F | Published generation 2, windows_default_v1, ready, complete=true; two bound volume_internal/recycle_bin roots, zero unavailable roots, 13 skipped expensive resolutions |
| D | Rebuilding toward windows_default_v1; retained legacy generation 6596, no published policy yet |
| E | Rebuilding toward windows_default_v1; retained legacy generation 22, no published policy yet |
| C | Rebuilding toward windows_default_v1; retained legacy generation 3537 with access_denied/sharing_violation/unresolved_parent_chain gaps, no published policy yet |
| Anonymous volume | unsupported_filesystem |

This verifies default-policy publication on F only. The 13 skipped resolutions
are operation counts, not unique files. It does not verify actual Windows rule
roots, measured performance or inclusion-mode switching. C/D/E are pending rather
than reported failed: their previous policy_rebuild_required error/diagnostic is
retained while phase becomes rebuilding. Source inspection confirms those fields
are cleared on a successful update, not when the rebuild is queued/started. This
transitional presentation is recorded as a follow-up finding. A single report
does not establish a stalled rebuild or elapsed-time limit.

The generation, completeness and coverage fields on those three rows describe
old snapshots; a matching published policy is required before claiming the new
scope is applied. Wait for later publication/status evidence. No product code,
SCM/UAC/index state or unrelated workspace files were changed while recording this
checkpoint; independent testing/review remain NOT_RUN.

## User-machine default-scope publication — 2026-10-20

Follow-up supplied by the user, with actual and expected worker both
`436F4DA234B67FC0C25DE9CE06268B17548551E13AADBF9B504E525361693D14`,
running service and Full access:

| Volume | Generation / phase | Coverage reasons | Default-policy roots bound / unavailable | Skipped resolutions |
| --- | --- | --- | --- | --- |
| D | 6691 / ready | None; complete=true | 2 / 0 | 25 |
| F | 2 / ready | None; complete=true | 2 / 0 | 13 |
| E | 37 / coverage_incomplete | unresolved_parent_chain | 2 / 0 | 57,030 |
| C | 3854 / coverage_incomplete | access_denied, sharing_violation, unresolved_parent_chain | 8 / 0 | 519,088 |

Every supported volume now has matching published/requested windows_default_v1,
null maintenance/worker errors and no active diagnostic. C binds all eight roots
across the seven intended categories; D/E/F bind the two volume-specific roots.
This is PASS evidence for default-policy binding/publication on the user's
machine, including Windows-specific categories. Earlier rebuilding rows have
successfully published; the recorded transitional error-label issue is no longer
an active error in this report. The anonymous unsupported volume is unchanged.

C/E publication remains partial. A parent-chain gap means some paths could not
be reconstructed; it does not establish file corruption/deletion or classify
those paths as intentionally excluded. Skipped resolutions are operation counts,
not unique files and not a measured speedup. No repeat rebuild/service action or
code change was performed. Inclusion-mode switching, restart/lifecycle recovery,
complete visibility and resource impact still need their own evidence; no full
P11 completion or independent acceptance is claimed. Independent testing/review:
NOT_RUN. Documentation-only diff check is the relevant local verification.
