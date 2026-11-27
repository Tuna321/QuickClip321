# Stability review — 2026-11-27

Base: main 1994cce914bbcbbfe27b181bef55a08489722d64. Work isolated on qa/stability-fixes. No release, signing, push, or production settings replacement.

## Original findings

| ID | Change / evidence |
|---|---|
| B01 | Atomic last-valid backup; preserve corrupt bytes; recover without automatically replaying old active work. Rust recovery cases and real executable corrupt-state launch pass. |
| B02 | Absolute folder/ancestor validation; structural PE executable validation. Rust invalid tool/path tests pass. PE validation is not an authenticity guarantee. |
| B03 | Preflight output directories; no silent organizer fallback. Real executable fails the invalid category, reports error, and retains staging. |
| B04 | Serialize rule edits against latest settings; same protection for profiles and shortcuts. JS concurrency fixture passes. |
| B05 | Ignore meaningless audio quality in active duplicate checks. Rust regression passes. |
| B06 | On confirmed history removal, clean verified owned partials only; keep unknown/completed media and report cleanup errors. Rust tests pass. |
| B07 | Track staged release version as well as channel; newer release is stageable. JS update scenarios pass. |
| B08 | Focused queue cards still receive progress and message updates; rebuild for changed state/actions. JS fixture passes. |
| B09 | Locale-independent pending preview state; resume on focus and cancel stale requests. JS and desktop generation tests pass. |
| B10 | Imported duplicate shortcut assignments resolved deterministically; concurrent edits preserve distinct bindings. Rust and JS tests pass. |
| B11 | Keep VTT STYLE/REGION/NOTE blocks and cue identifiers. Rust regression passes. |
| B12 | Case-insensitive WWW handling. Rust regression passes. |
| B13 | Fragment choices cover allowed range 1–8. Rust regression and UI source check pass. |
| B14 | Corrected six malformed Turkish strings. |

## Additional risks

| ID | Mitigation / residual |
|---|---|
| R01/R08 | Prepare outputs before publication, use common noncolliding stem, never overwrite, persist each committed path; late collisions retain records. Tests pass. Multi-file crash atomicity is not guaranteed; abrupt power loss between move and state commit remains possible. |
| R02 | Repair pinned managed tools using verified replacement; quarantine old directory, restore on failed replacement. Managed-tool fixture passes. |
| R03/R04 | Bounded supervised inspection; chunked cancellable publication. Timeout/cancel fixture passed isolated retest; timing assertion failed once under concurrent load. |
| R05 | Paginated release discovery, bounded at 300 records; explicit error at limit. Stable-after-30-nightlies test passes. |
| R06 | Check availability while active; stage/install only when queue idle. Code reviewed; live background watcher scheduling not fully E2E tested. |
| R07 | Handle exited process/path race without accepting a live unrelated process; preserve state recovery backup on swap/rollback. Cached published signed stable/nightly assets successfully swapped both directions. Forced health-failure rollback of new helper not independently repeated. |
| R09 | Valid batch entries accepted, invalid/skipped entries retained with individual errors. JS fixture passes. |
| R10 | Explicit current-stream progress wording rather than implying whole-playlist completion. Aggregate playlist progress remains unimplemented. |
| R11 | Conservative YouTube watch/share ID normalization; signed, timestamped and playlist URLs not merged. Rust tests pass. Full cross-extractor canonicalization and organizer-rule changes are not covered. |
| R12 | Atomic settings patch under engine lock; serialized frontend updates. Rust concurrent preferences and JS tests pass. |
| R13 | Locale-aware rendering and known message translation. Full dynamic backend-error localization remains incomplete. |
| R14 | Superseded previews cancel; stale results ignored. Desktop and JS tests pass. |
| R15 | Open: old update transaction backup retention. No automatic deletion implemented; preserving rollback evidence is preferable to unsafe cleanup. |
| R16 | Active-only snapshots and settings-only reads reduce polling overhead. 5,001-job fixture passes without deleting history. Long-running GUI stress and history pagination remain open. |

## Test scope and release gate

- First full core suite: 36 unit/fixture + 14 regression tests passed, including five ignored fixtures.
- Final core: 36/37 passed in one concurrent run; sole timeout wall-clock assertion passed isolated retest. Final regression suite: 16/16. Earlier wrong fixture path was corrected. Final full suite was not all-green in one invocation.
- Desktop: first 10/10 including live public signed-asset verification; final 10 normal tests passed and one network-dependent test ignored. Final debug Windows build succeeded.
- Two JS suites pass; CI now invokes both. git diff --check passes (line-ending warnings only).
- Real candidate startup after corrupt state: 14/14 reported checks true; exit 0. Real invalid organizer target: failed visibly, no fallback media.
- Signed helper: both published channel swaps succeeded using locally cached signed manifests when GitHub DNS failed. Tampered asset and wrong-channel checks rejected. Previous state backup retained. No new signed release produced.
- Actual GUI E2E suite results are recorded separately in stability-app-e2e-retest.log in QA parent folder.
- No clean VM, physical hotkey, long-term stress, all public extractor combinations, real disk-full/power-loss, or release-build/signed candidate test. Do not treat this branch as production-ready or publish without the remaining gate.

Evidence logs reside in the parent QA folder; fixture data and binaries are excluded from the source commit.
