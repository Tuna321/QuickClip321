# Architecture decision: improve first, migrate only with evidence

Status: proposed. The RC3 Python app remains the reference implementation.

## Why not rewrite the downloader

The network extraction and codec workloads are performed by yt-dlp and FFmpeg. A Rust shell cannot, by itself, make remote servers send bytes faster or eliminate transcoding. Keep the known-good extraction and media-processing tools and compare orchestration alternatives.

| Approach | Benefit | Risk |
| --- | --- | --- |
| Python + CustomTkinter (current) | Existing behavior, tests and distribution | UI and packaging overhead |
| Rust + Tauri v2 + web UI | Strongly typed native orchestration, flexible accessible UI, WebView2 | Sidecar lifecycle, WebView2 availability, packaging and migration |
| Rust + native GUI | No WebView dependency | Windows tray, keyboard and accessibility implementation cost |

## Proposed V3 experiment

1. Add an isolated Rust/Tauri shell on a separate experimental branch, not a replacement for this app.
2. Use bounded structured messages for progress and errors; invoke an explicitly selected yt-dlp/FFmpeg executable with argument arrays, never shell interpolation.
3. Keep credentials session-only, use narrow Tauri capabilities and do not load remote web content into privileged UI.
4. Preserve existing history and settings via versioned import (copy, never mutate old state), cancellation, playlist consent, filenames, collision handling and redacted diagnostics.
5. Compare a clean Windows VM baseline with the same network, media fixtures, versions, quality and destination.

## Acceptance gates (targets, not measured results)

- No regression in existing unit tests, packaged smoke, file preservation, authorization or playlist safety.
- Measure cold and warm startup, idle working set, end-to-end time, transcode time, artifact size and first-run dependency cost. Report medians and variability for repeated trials.
- Target at least 30% lower cold-start time or idle memory with no material regressions in core workflows. Otherwise keep the current implementation.
- Keyboard navigation, DPI scaling, screen-reader labels, cancel/resume, offline errors, clean install, rollback and licensing must be checked before release.

## References

- https://v2.tauri.app/develop/sidecar/
- https://v2.tauri.app/distribute/windows-installer/
- https://v2.tauri.app/start/prerequisites/
- https://github.com/yt-dlp/yt-dlp
- https://github.com/NickvisionApps/Parabolic
