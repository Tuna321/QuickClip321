# Changelog

## 3.0.0-beta.8 - 2026-11-27

- Rebuilt Updates: installed version and channel, exact available version, last check and up-to-date status.
- Check both release tracks automatically at startup and hourly; manual check is always available.
- Dismissible in-app update notification remembers the dismissed version. A newer release shows it again.
- Single verified install action, separate explicit channel switches, queue-idle and stale-release protection.
- Download subtitles only in SRT or VTT, with language selection and optional automatic captions; no audio/video download.
- Added regression checks for both channels, banner dismissal, stale update responses, offline/queue safety and subtitle output groups.
- Stable-track Beta 8 and Beta 8 Nightly 1 share the same fixes; both remain prereleases.

## 3.0.0-beta.7 - 2026-11-27

- Recover corrupt or missing state using the last valid backup, preserving damaged bytes.
- Validate executable and output paths; repair verified managed tools; never silently redirect invalid organizer targets.
- Protect existing media and sidecars; persist committed output paths; bound and cancel media inspection.
- Fix concurrent settings edits, duplicate detection, queue progress, batch errors and superseded previews.
- Improve release pagination, staged-version selection and recovery-backup handling.
- Add Windows and frontend regression tests. Both Stable-track Beta 7 and Beta 7 Nightly 1 are prereleases with the same fixes.

## 3.0.0-beta.6 - 2026-11-27

- Signed public beta and Nightly channels, explicit verified switching, restart health check and rollback backup.
- Actual saved-file folder, duplicate warning and optional redownload, native completion/failure notices from the shared queue.
- Per-download speed cap, queue recovery, settings export/import, recommended media-tool update and rollback.
- Local fixture, real app, signed round-trip and packaging tests; no final 3.0 production-stable certification.

## 3.0.0-beta.5 - 2026-11-26

- Custom global and in-app shortcuts, native localized fade notices and last-used download settings.
- English/Turkish interface, size estimates, folder shortcut and always-on-top toggle.
- Expanded organizer: 25,693 records, contextual imported titles and automatic custom rule persistence.
- Fast clip download with full-download fallback, and bounded local partial-frame previews.
- Clear submitted links, reset finished preview, fix reopening unfocused popup and verify settings across restarts.

## 3.0.0-beta.4 - 2026-11-26

- Live first-launch setup stage and per-file byte progress.
- Longer allowance for slow downloads, with a 90-second no-progress watchdog and clearer retry errors.
- Keep automatic verified setup and existing tool configuration unchanged.

## 3.0.0-beta.3 - 2026-11-26

- Automatically download, verify and configure missing yt-dlp, FFmpeg, FFprobe and Node.js on first launch. Existing complete configurations are retained.
- One-time setup progress and Retry; no manual executable selection required.
- New installations use Dark theme, Premiere Ready MP4 and 1080p by default.
- Fix editor downloads from direct media sources without resolution metadata; check actual output height.
- Tested cold setup and first download in isolated state with no preinstalled media tools on PATH, then restart without reinstall.

## 3.0.0-beta.2 - 2026-11-26

- Public Windows x64 beta built with Rust, Tauri and WebView2.
- Tray-first interface, clipboard capture, independent preset, format and resolution controls.
- Queue, batch and playlist downloads, optional clipping, subtitles and metadata.
- Contextual organizer with 2,302 curated records, custom rules and destinations.
- Windows notifications, persistent appearance and popup position, startup preference.
- Version-pinned verified media tool setup and scoped single Original transfer resume.

See the [release notes](https://github.com/Tuna321/QuickClip321/releases/tag/v3.0.0-beta.2) for requirements and known limitations.
