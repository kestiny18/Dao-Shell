# P16 — Windows system and cache exclusions for file discovery

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: On 2026-10-20 the user approved designing and implementing the Windows portion of the discussed default exclusions. No macOS implementation, service elevation, historical migration, commit, push or publication is authorized.
Baseline: HEAD c7780ab4545028627d4e15de087165a6d9d21a43 plus the preserved P12 bounded-budget recovery changes, tests, Plan and devlog. The AF860FBC worker candidate passed implementation self-tests and the user's C/E budget-blockage trial; do not discard or overwrite that evidence or installer.
Delivery: Windows exclusion design, implemented local prototype, relevant self-tests, bilingual scope/coverage UI and a separately named local installer with matching worker fingerprint.

## Goal and behavior

Default name discovery should concentrate on user files and usable application
entries. Explicit Windows-internal and regenerable temporary/cache locations may
be excluded from ordinary file indexing. User-created folders, data drives,
documents and application-managed user data remain discoverable. The approved
advanced inclusion capability must have a real implementation: a setting or
search control must explain its scope and any indexing work required; it must not
pretend to reveal records that were never collected.

Exclusions change discovery coverage, not read/open authority. Existing Scope,
ordinary-client authentication, exact file/volume identity checks, cancellation,
confirmation, published-generation atomicity and P12 budget recovery remain.

## Proposed Windows policy

- Locate the installed Windows directory through the operating system, rather
  than assuming the OS is on C:. Windows components/update caches within that
  directory, volume recovery internals such as System Volume Information, known
  temporary/cache locations and Dao-Shell's own index files are candidates for
  default exclusion. Define the final narrow list and exceptions during T1.
- Do not exclude whole AppData, ProgramData, Program Files, user profiles or data
  drives. Do not classify all hidden/system attributes, file extensions, or all
  access-denied errors as exclusions. Known caches must be positively identified.
- Preserve application launch entries and keep the existing application catalog
  usable. Application component files may be hidden from ordinary results under
  declared rules; no implicit uninstall or file deletion is involved.
- Match real directory boundaries and volume identities, not naive path prefixes.
  Preserve included names of files with hard links in both excluded and included
  locations. Renames, moves and links crossing the policy boundary must update
  correctly. Reparse points must not redirect an exclusion onto unrelated data.
- macOS/Linux adapters remain unimplemented. Keep Windows rules at the platform
  boundary; do not introduce a general policy framework for hypothetical systems.

## Tasks and acceptance

### T1 — Finalize the smallest correct design

One Coder investigates current source/store/worker/settings boundaries and proposes
the specific exclusion list, platform classification point, advanced inclusion
behavior and existing-index compatibility. Main Agent records the decision before
production edits. Prefer a small implementation; do not sacrifice the inclusion
escape hatch or honest coverage to make it smaller. A change in index policy must
not silently use stale pre-policy results as if excluded data had been removed.
Do not migrate or delete existing P9/P10/P11 snapshots. Where a new build or
explicit rebuild is necessary, communicate it and preserve the old publication
until a valid replacement is published.

### T2 — Implement Windows collection and maintenance policy

Apply known exclusions early enough to avoid unnecessary expensive metadata
reconciliation where it is correct to do so; hiding final results alone does not
satisfy the collection goal. Retain only the minimal identity/parent information
needed for classification and correctness. MFT/USN enumeration may still traverse
volume-wide records; report this limitation without claiming unmeasured speedups.
Uncertain classifications remain included or explicitly unresolved, not silently
dropped. Prove that policy boundaries, outside hard links, new included files and
boundary-crossing renames/moves are handled honestly. Existing budgets stay bounded.

### T3 — Desktop inclusion control, receipts and safe diagnostics

Provide a usable advanced inclusion path and explain when it takes effect. Keep
save failures/cancel/config reload and session candidate invalidation correct.
Default UI and exported reports distinguish configured exclusion from access
denial, missing metadata, incomplete parent chains and other failures. Reports
contain safe policy identifiers/categories/counts without private paths or account
identifiers. Completeness is explicitly relative to the declared policy. Both
Chinese and English must be covered; permissions do not expand on a scope toggle.

### T4 — Self-test and local trial package

Use synthetic records/sources and isolated temporary directories. Cover default
exclusion and advanced inclusion, Windows on a non-C volume, directory-name prefix
collisions, AppData/ProgramData/user-data preservation, outside hard links, reparse
boundaries, incremental moves/renames, first build/existing snapshot behavior,
policy changes, cancellation/rollback, truthful coverage/diagnostics and bilingual
UI. Run affected existing core/desktop/UI tests, build, format and Clippy checks.
Main Agent will build one final matching desktop/worker installer and verify it
with isolated no-model configuration. Preserve the public 0.1.0 and P12 candidate.

## Boundaries and verification status

No actual service management/UAC, source-file action, real-volume rebuild, host
restart, historical-index migration, query caching/FTS/SQL acceleration, new
collection algorithm, external-model request, Git mutation or public release.
No Tester/Reviewer is requested. Production implementation is by one Coder;
main Agent owns the Plan, integration, evidence and delivery. Record exact source,
scope and missing real-machine evidence rather than declaring P11 complete.

References: [workflow](../workflow/README.md), [development](../DEVELOPMENT.md),
[desktop](../../desktop/README.md), [P12 recovery](../devlog/2026-10-20-p12-index-budget-recovery.md).

## T1 design decision — 2026-10-20

Use one active index per volume with two fixed modes: `windows_default_v1` and
`windows_all_v1`. A separate per-volume control in File Search settings offers
the mode and an explicit, locally confirmed Apply and Rebuild action. This is a
component policy, persisted by the authenticated service in a fixed protected
`policy.json`; no arbitrary paths, SIDs or shell arguments are accepted. Do not
couple it to Config's unrelated model/credential save transaction. The published
snapshot records its actual policy, and reports show requested versus published
policy. Legacy snapshots are labeled `legacy_all` and are not silently described
as filtered. Rebuilding atomically replaces a publication only after success;
failures keep the old snapshot without representing it as the requested scope.

The initial exclusion list is deliberately narrow:

| Location | Default treatment |
| --- | --- |
| Real Windows directory: WinSxS, servicing, Installer, System32/DriverStore/FileRepository | Exclude internal component/install data |
| Real Windows directory: Temp, SoftwareDistribution/Download | Exclude known temporary/update cache data |
| Each fixed volume root: System Volume Information, $Recycle.Bin | Exclude volume-internal/recycle data |
| Dao-Shell index files and sidecars | Continue excluding in both modes |
| Other Windows/System32 content, Program Files, AppData, ProgramData, user folders and data drives | Preserve; no blanket exclusion |

Locate Windows using its native API. Bind existing rule roots to canonical
GUID/serial/directory identities, reject reparse traversal and compare whole path
components. Absent optional cache roots are distinct from failed/unsafe binding;
later creation must be recognized. Uncertain locations stay included with honest
policy-limitation reporting. User-profile caches and third-party caches are not
guessed in this first version.

Early skipping may perform cheap existing handle identity/link-count checks and
stable canonical-path checks before avoiding the more expensive per-file USN/name
reconciliation. Multi-link files still enumerate and verify all names, filtering
only excluded aliases. No all-volume parent graph cache is added, and no speedup
claim is made without measurement.

Normal included-to-included directory renames must retain incremental maintenance.
Only proven exclusion-boundary crossings, rule-root/ancestor changes, or genuinely
uncertain classification affecting coverage may require a bounded whole-volume
rebuild. Retain only bounded current-batch rename evidence; missing/racing evidence
uses honest recovery rather than silent loss. At most two policy-race rebuild
recoveries are allowed before an actionable state. Rebuilding every ordinary user
directory rename is explicitly rejected. File/link changes keep existing incremental
behavior and P12's bounded budget handling.

T1: design selected; T2/T3 implementation and self-tests delegated to the same
single Coder. Main Agent will integrate evidence and build the final local package.

## Delivery — 2026-10-20

T1–T4 are READY for the local prototype scope. Fixed Windows rules, early verified
exclusion, included hard-link aliases, bounded boundary recovery, per-volume
confirmed mode switching, published/requested scope reporting, protected policy
persistence and pending-change restart recovery are implemented. Ordinary project
renames remain incremental. Permission/identity checks, old snapshot preservation
and P12 bounded budgets remain; no historical schema migration was performed.

Core all-targets: 202 passed / 3 ignored; final narrow policy follow-up: 8 passed;
desktop Rust: 22 passed; final UI: 64 passed. Native temporary-file tests include
hard links, directory root replacement/creation, rename boundaries and junctions.
Both Clippy/format checks, JS syntax, package verification and isolated Release
startup/normal-close passed. An initial UI form-submit bug and a final first-build
scope-label finding were fixed and covered by affected regressions. Unchanged
core/backend evidence is retained across the last UI-only correction.

Local trial package:
`dist/Dao-Shell-0.1.0-p16-exclusions-20261020-windows-x64-setup.exe`,
9,163,863 bytes, SHA256
`515FC220757BB41CBE3CB8EE31DED47B51DC104953F17DE2BA37B38498EA8F36`.
Embedded worker:
`436F4DA234B67FC0C25DE9CE06268B17548551E13AADBF9B504E525361693D14`.
The public 0.1.0 and prior P12 trial installers remain hash-verified and preserved;
the public installer's standard dist filename was restored. Product version remains
0.1.0, so identify this unpublished candidate by its filename and build fingerprint.

Trial: install/relaunch this candidate, explicitly update the service, verify both
reported build fingerprints match 436F4DA2…, then select a scope for each volume
and confirm Apply and Rebuild. Existing legacy indexes wait for that explicit
action; fresh volumes can build their default scope. While scopes differ, indexed
search waits for the requested scope's valid publication. No source files are
deleted, and inclusion does not expand access permissions.

Actual protected-system-directory behavior, real service management/restart/
rollback, full machine coverage and resource impact remain NOT_VERIFIED for this
candidate. No measured performance claim, independent acceptance, Git commit/push
or public release is made. P11 remains IN_PROGRESS. See the
[implementation record](../devlog/2026-10-20-p16-windows-index-exclusions.md) and
[desktop instructions](../../desktop/README.md).

## User-machine checkpoint — 2026-10-20

The supplied report matches the exact 436F4DA2…93D14 worker in both actual and
expected build fields; service is running with Full access. F has published
`windows_default_v1`, generation 2, complete=true within that scope, with two bound
roots (`recycle_bin`, `volume_internal`), zero unavailable roots and 13 skipped
expensive resolutions. This is positive real-machine evidence for the default
policy publication on F, not a unique excluded-file count or Windows-directory
policy/performance proof.

C/D/E are still `rebuilding`, requested policy `windows_default_v1`, published
policy null. Their reported generations (C3537/D6596/E22), completeness, coverage
reasons and timestamps belong to retained legacy snapshots. The code changes the
phase when rebuilding starts but retains the prior `policy_rebuild_required`
error/diagnostic until a successful publication; this is a confusing transitional
display, not evidence from this single report that rebuilding failed or stalled.
Record this state-presentation finding for follow-up; do not infer new scope
completion from complete=true on D/E. C's old access/sharing/parent-chain gaps stay
unresolved by this checkpoint. The anonymous unsupported-filesystem volume is
unchanged.

Wait for C/D/E to publish matching policy IDs with ready/coverage_incomplete and
cleared maintenance errors before assessing their new scope. Windows-specific
root binding, inclusion-mode switching and persistent service lifecycle evidence
remain pending on this machine. Only documentation was updated; no service action,
rebuild request, code mutation or release occurred when processing this report.

## User-machine default-scope publication — 2026-10-20

The next user report has actual and expected worker builds both matching
`436F4DA234B67FC0C25DE9CE06268B17548551E13AADBF9B504E525361693D14`,
with running service and Full access. All four supported volumes now publish
`windows_default_v1`, matching their requested policy. Maintenance errors and
worker errors are null. This supersedes the pending C/D/E publication checkpoint;
it does not erase its historical transitional-display finding.

| Volume | Generation | Published coverage | Bound / unavailable roots | Skipped resolutions |
| --- | --- | --- | --- | --- |
| D | 6691 | ready, complete=true | 2 / 0 | 25 |
| F | 2 | ready, complete=true | 2 / 0 | 13 |
| E | 37 | coverage_incomplete; unresolved_parent_chain | 2 / 0 | 57,030 |
| C | 3854 | coverage_incomplete; access_denied, sharing_violation, unresolved_parent_chain | 8 / 0 | 519,088 |

C reports all seven intended categories: driver_cache, installer_cache,
recycle_bin, system_temp, update_cache, volume_internal and windows_components.
D/E/F report recycle_bin and volume_internal. Default-scope root binding and
publication on this machine: PASS for the reported scope. C/E have usable
published snapshots with remaining coverage gaps, not a maintenance failure.
An unresolved parent chain does not establish deletion/corruption or prove that
the affected names belong to intentionally excluded directories. The anonymous
unsupported-filesystem volume remains unsupported.

Skipped resolutions count avoided expensive operations, potentially repeated
across updates; they are not unique excluded files or measured performance gains.
No further rebuild is justified by this report alone. Inclusion-mode switching,
persistent service lifecycle recovery, full path visibility and measured resource
impact remain NOT_VERIFIED by these reports. P11 remains IN_PROGRESS; independent
testing/review remain NOT_RUN. Only Plan/devlog evidence was updated.
