# QuickClip321 V3 — tray-first experimental migration

Current candidate: 3.0.0-alpha.7. This is not a replacement for the published Python RC2 until baseline parity and release gates pass.

## What is implemented in the candidate
- Hidden startup, frameless rounded Windows popup, taskbar exclusion, tray click show/hide, quick/advanced modes, monitor work-area placement (Tray/Corner/Custom), Escape and quick-view blur dismissal.
- Familiar link/preset/format/resolution/preview/destination layout, default 1080p Premiere Ready, native clipboard change capture while hidden with draft protection, six palettes plus System, six accents and persistent preferences.
- Bounded asynchronous metadata and optional thumbnail preview, explicit playlist choice, batch links, bounded 1-3 simultaneous download queue (default one), waiting-job Hold/Resume, cancellation/retry/history and file-safe publishing.
- Local artist/movie/TV/game/audio classification with an expanded built-in catalog (default Audio/Video folders; manual selection wins).
- MP4/H.264/AAC verification and conversion, MP3/M4A/WAV/original, optional post-download clipping and subtitle timestamp adjustment, archive metadata option.
- Tools detected on PATH, or explicitly configured. No unverified auto-downloads.

## Build and test
```powershell
cargo test --locked --manifest-path core/Cargo.toml
$env:QC_TEST_YTDLP = (Get-Command yt-dlp.exe).Source
$env:QC_TEST_FFMPEG = (Get-Command ffmpeg.exe).Source
cargo test --locked --manifest-path core/Cargo.toml -- --include-ignored
cargo build --release --locked --manifest-path desktop/Cargo.toml
```
Run the EXE and use the notification area icon (possibly under the overflow arrow). The app has separate V3 state. Smoke tests use a separate directory.

## Release blockers / known gaps
- Feature parity is incomplete: active-transfer pause/resume, named default profile, notification, authenticated sessions, diagnostics/export, local and remote optional AI, clean-machine dependency setup, full recovery and privacy/lifecycle verification.
- Actual external-site thumbnail and physical tray click, outside click, keyboard, DPI and multi-monitor interactive checks are not replaced by geometry/native-window tests.
- Clip and subtitle handling are implemented, but subtitle extraction on real sites and archive output still need fixtures. Original-format video clips are converted to MP4; audio clips retain supported audio format.
- No measured startup/RAM/CPU comparison or public signed installer. No claim of universal site support or complete parity.

## Design and audit files
UX-RESEARCH.md, BASELINE-PARITY.md, BASELINE-SYMBOLS.json, MULTI-PERSPECTIVE.md, research-alpha3.md, research-alpha4.md, research-alpha5.md, research-alpha6.md, research-alpha7.md, DECISION-JOURNAL.md, ITERATION-LOG.md, NEXT-PASS.md. These are the migration contract, not marketing claims.

Use media you have permission to download. Do not publish any personal app state, credentials, or generated executable as source.

Alpha 5 adds named format/resolution/archive profiles, ordered title rules, optional absolute category paths, installed-tool detection, redacted diagnostics, generic error categories, pre-output network retries and English form validation. Parallel jobs are experimental until repeated media regression and real-site gates pass. No automatic dependency installation or active-transfer pause/resume yet.

## Quick start (Windows development candidate)
Run the EXE, copy a media link, open QuickClip321 from the notification area, select a preset, and press Download. The app captures links when clipboard detection is enabled; it does not auto-download. Queue toggles its own view. Change selects a manual folder; Reset appears only after manual selection. The global organizer switch is in Advanced > Settings. Detailed categories and custom rules are in Organizer. Use only authorized media. This candidate requires installed yt-dlp and FFmpeg/FFprobe; it is not a clean-machine installer.

Alpha 7 catalog: 317 hip-hop/related artists, 336 films, 310 series and 171 games. 20-item development fixture, not a real-world accuracy estimate. Optional Clean, Glass-style, Carbon and Sand finishes. Active byte resume, verified bootstrap, notifications and full original parity remain release gates.
