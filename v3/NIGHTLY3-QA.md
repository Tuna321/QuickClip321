# Nightly 3 / public Beta 6 release validation

## Scope
- Public Beta 6 is the Stable *track*, still a prerelease. Nightly 3 is the experimental track. A fresh installation defaults to its own channel; existing preferences are retained.
- Stable/Nightly switches require explicit user intent, a verified signed manifest and artifact, and an idle queue. The same-channel downgrade guard remains active.
- Release checks reject stale UI responses, mismatched channels, missing or unsigned assets for automatic installation, and invalid signatures. Failed automatic network checks retry after five minutes.
- Isolated test copies use a separate WebView profile and do not hand off to a running normal app. Completion tests exercise the UI and shortcut dispatch without touching the real clipboard.

## Evidence collected during this pass
- Core: 31 local/fixture tests passed, including managed tool reuse, actual Range resume, crash-state recovery, exact output, process supervision and idle-update guard. Separate external preview test passed its coverage criterion (YouTube title and thumbnail; Wikimedia title only; Vimeo unavailable).
- Desktop: 8 unit tests passed on both channel versions. Node UI regression fixture passed stale response, offline reset, explicit switch, queue-busy recovery and wrong-channel staging.
- Actual Nightly 3 app prior to fixture-only extensions: 1,868,278-byte loopback media, unlimited 3.22 s, capped 256 KiB/s 10.28 s; identical bytes. Duplicate redownload and actual nested Explorer location, settings backup round trip and native completion notice passed.
- Actual signed portable replacement Nightly 3 -> public Beta 6 -> Nightly 3 on isolated copies passed; both replacement EXEs reported healthy UI, previous EXEs and settings preserved. Wrong-channel manifest rejected.
- A first-run test initially failed because notification watchers were deliberately disabled in smoke mode; the completion-only fixture flag now enables that watcher for the regression test. Normal behavior is unchanged.

- Final Nightly 3 executable: UI-origin completion notifications at both speed settings, native completion window/focus/destruction, actual shortcut dispatcher, duplicate folder and settings round trip all passed. Unlimited 4.06 s; 256 KiB/s 11.11 s.
- Signed portable packages: stable 13,327,872-byte EXE / 4,180,918-byte ZIP; nightly 13,325,312-byte EXE / 4,180,424-byte ZIP. Signatures and extracted artifact bytes verified.

- Final public Beta 6 executable passed the same real application tests: unlimited 3.42 s, capped 10.80 s, exact bytes, both completion paths, duplicate/folder and settings. Final signed swap in both directions passed with settings and prior EXEs preserved.

## Limitations
- No clean Windows VM or second physical machine in this session; no claim of universal site/format compatibility, abrupt physical power-cut safety, or full native frontend parity. WebView2 is still required.
- No Authenticode certificate; signed updater manifests authenticate release payloads but do not remove Windows SmartScreen warnings.
- Performance cap test is local HTTP, not an internet speed benchmark. No measured general speed improvement.
- Drag/drop and custom filenames are not implemented; existing automatic filename behavior remains.
- Production 3.0 stable promotion remains blocked by the outstanding gates in RELEASE-GATES.md.
