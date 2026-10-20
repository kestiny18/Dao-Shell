# P12 — Bounded recovery from index build budget exhaustion

Implementation: READY (bounded recovery prototype; user-machine C/E budget blockage resolved)
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN

## Scope and evidence

On 2026-10-20 the user supplied a remote trial report with C/E in
`error/budget_exhausted`, no published generation, and mismatching F588 service /
90E1 expected builds. The user subsequently reported that C still reached the
budget. The user then confirmed that Service Management's update action had
succeeded and C still failed after refreshing. Updated build fingerprints have
not been supplied; the action confirmation is not a replacement for that readback.
The actual C-volume failure has not been reproduced locally, and this change
does not claim to establish its precise budget branch or fix that machine yet.

Candidate source is HEAD `c7780ab` plus this task's uncommitted changes. The only
pre-existing change was the P12 Plan's diagnostic follow-up, which is preserved.
The main Agent recorded scope and delegated implementation/self-test to one Coder.
No independent roles were started.

## Mechanism and change

The service previously used a fixed budget of 1,000,000 metadata resolutions and
10,000 pages per update. A failed transaction rolled back, after which maintenance
blocked until an explicit rebuild. Rebuilding with the same budget could fail
again. Resolution counts include parent/duplicate/retry work; they are not a file
count or a memory percentage.

- Store errors now identify metadata, enumeration and catch-up exhaustion with a
  typed error. Only these errors permit budget growth; matching arbitrary error
  text does not trigger recovery.
- Maintenance doubles the budget up to eight times the initial limits. A pure
  budget-recovery episode makes at most four attempts (1x, 2x, 4x, 8x; total 15x).
  Existing metadata-race retries remain separately limited to six; when interleaved,
  the combined episode can use at most 63x initial work. Successful limits are
  retained during the worker lifetime; explicit rebuilding resets the policy.
- Every attempt remains atomic. Exhaustion/cancellation keeps the prior published
  generation and checkpoint, or leaves a first build unpublished. No schema or
  historical-data migration, identity check removal, or permission widening occurs.
- Bounded retries show `budget_retrying`; exhausting the maximum stops with
  `budget_limit_exhausted`. Safe diagnostics distinguish the three budget stages.
  Settings and search receipts show Chinese/English guidance. A request-level
  `index_not_ready` no longer hides the underlying maintenance failure.

This fixes a functional recovery gap. Query caching, FTS, SQL acceleration,
collection algorithm replacement and the user's deferred performance targets are
outside scope. Larger retry transactions can make initial indexing take longer;
this is not a resumable partial-build or performance improvement claim.

## Implementation self-test

Logs are in ignored `.tools/p12-budget-20261020/`.

- Core library: 111 passed, 2 auxiliary helpers ignored (`core-lib-final.log`).
  The suite covers real maintenance-loop recovery for initial/existing indexes,
  each budget branch, final-limit blocking, prompt retry-wait cancellation,
  cancellation during resolution, old publication/checkpoint preservation,
  non-budget rejection, and overflow/zero/reset policy boundaries.
- Search receipt regression preserves both request and maintenance errors while
  rejecting arbitrary diagnostic codes. Existing search/index/file-session cases
  ran as part of the core library suite.
- Desktop Rust: 20 passed (`desktop-tests.log`), including safe diagnostic
  allowlists. UI: 61 passed (`ui-tests.log`), including both languages' budget
  recovery/final-limit presentation.
- File-session integration: 3 passed; search contract: 4 passed
  (`search-session-tests.log`).
- Core all-target Clippy with warnings denied, desktop Clippy, both format checks,
  JavaScript syntax and diff whitespace checks passed. The first budget test run
  had one fixture-count expectation failure; corrected to account for enumeration
  plus catch-up resolution, then passed in the complete affected suite. The first
  log is retained, not presented as a successful run.

No actual service management, UAC, real-volume rebuild, source-file action, host
reboot, model request, commit, push or release is authorized by this test record.
The already-published 0.1.0 installer is preserved and hash-checked before packaging.

## Candidate delivery

Local candidate:
`dist/Dao-Shell-0.1.0-p12-budget-20261020-windows-x64-setup.exe`,
9,103,720 bytes. SHA256:
`6E74007A2BD32FC6F6F7D428A33846202DC9CD9FDD25A98B27A368A46ED5776A`.
Its product version remains 0.1.0; this is an unpublished working-tree candidate,
not a replacement for the public release or a new tag.

Worker SHA256:
`AF860FBC6CB943D181C29C124BC071496A5F09D0E4D74D6E833FD88AF00FE619`.
Desktop SHA256:
`3FBE6C233982977A9E58A181784C75104B43EE01C784358A957FC5F771574081`.

The standard packaging script completed successfully (`package.log`). Package
verification passed: x64 GUI, exact matching embedded worker bytes/digest, loader,
English/Chinese guides, resource allowlist, small WebView2 bootstrapper and checksum.
The own Release process started with an isolated restricted/no-model configuration,
loaded the expected WebView2 loader and exited normally; configuration bytes were
unchanged (`native-final/result.json`). This is startup/exit self-test, not actual
installation or UI acceptance. An initial verifier invocation used the wrong full
HEAD argument and stopped before package inspection; rerunning with `git rev-parse
HEAD` passed. It was a verification invocation error, not a product failure.

Build source/diff, untracked hashes and artifact details are recorded under
`.tools/p12-budget-20261020/`. Subsequent changes are delivery documentation only.
The public 0.1.0 installer was restored at its standard dist path and hash-checked
against `EF4C004F…FB6F`; the candidate uses its own filename and checksum.

Trial steps: install the explicitly named local candidate, open Settings → File
Search → Service Management → Update Service, then refresh and check that both
expected/actual builds match the AF860FBC… worker. An index with no prior publication
will rebuild automatically when the new service starts. Export another diagnostic
after it publishes or reaches the final error; installing the desktop alone does
not update the service.

The actual remote C-volume result, service upgrade path, long-running resource
impact and other P11 lifecycle gaps remain unverified. The user-reported issue is
not closed, no Git commit/push/release or service operation was performed, and P11
is not completed by this fix. Independent testing/review remain NOT_RUN.

## Subsequent user-machine verification — 2026-10-20

The user first supplied a matching 79944B public-release report: both C/E still
failed with `budget_exhausted/index_maintenance` and no published generation.
The next report has actual and expected worker builds both matching the full
AF860FBC…FE619 candidate digest above, `state=running` and `full_access=true`.
The user explicitly reported that this trial now worked.

| Volume | Reported result |
| --- | --- |
| E | ready, complete=true, generation=1, no error |
| C | coverage_incomplete, complete=false, generation=335, no error/worker_error |
| D | ready, complete=true, generation=6228, no error |
| F | ready, complete=true, generation=1, no error |
| Anonymous volume | unsupported_filesystem |

This closes the specific remote C/E failure to publish due to exhausted budgets
for this candidate and machine. It supersedes the earlier pending-trial statement
above; it is user-supplied real-machine evidence, not local reproduction or an
independent test report. The final snapshot does not disclose which budget branch
triggered recovery or how many attempts ran.

C still reports `access_denied`, `metadata_unavailable`, `sharing_violation` and
`unresolved_parent_chain`. Its published index remains partially covered; these
categories must not be relabeled as complete or all assumed harmless. F's unchanged
generation alone does not prove live update health. Full visibility, long-running
behavior, restart recovery, rollback and other P11 lifecycle items remain outside
this evidence. No additional product change, service operation, commit, push or
publication was performed when recording this report; independent roles remain
NOT_RUN.
