# Changelog

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
