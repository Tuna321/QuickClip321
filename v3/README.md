# QuickClip321 V3 — Windows tray downloader

Current public preview: 3.0.0-beta.6 (Stable track), with 3.0.0-beta.6.nightly.3 on the experimental track. Both are prereleases, not a production-stable release.

## What is implemented in the candidate
- Hidden startup, frameless rounded Windows popup, taskbar exclusion, tray click show/hide, quick/advanced modes, monitor work-area placement (Tray/Corner/Custom), Escape and quick-view blur dismissal.
- Familiar link/preset/format/resolution/preview/destination layout, default 1080p Premiere Ready, native clipboard change capture while hidden with draft protection, six palettes plus System, six accents and persistent preferences.
- Bounded asynchronous metadata and optional thumbnail preview, explicit playlist choice, batch links, bounded 1-3 simultaneous download queue (default one), waiting-job Hold/Resume, cancellation/retry/history and file-safe publishing.
- Local artist/movie/TV/game/audio classification with an expanded built-in catalog (default Audio/Video folders; manual selection wins).
- MP4/H.264/AAC verification and conversion, MP3/M4A/WAV/original, optional post-download clipping and subtitle timestamp adjustment, archive metadata option.
- Tools detected on PATH, explicitly configured, or installed automatically when missing through verified version-pinned managed setup.

## Build and test
```powershell
cargo test --locked --manifest-path core/Cargo.toml
$env:QC_TEST_YTDLP = (Get-Command yt-dlp.exe).Source
$env:QC_TEST_FFMPEG = (Get-Command ffmpeg.exe).Source
cargo test --locked --manifest-path core/Cargo.toml -- --include-ignored
cargo build --release --locked --manifest-path desktop/Cargo.toml
```
Run the EXE and use the notification area icon (possibly under the overflow arrow). The app has separate V3 state. Smoke tests use a separate directory.

## Stable release blockers / known gaps (historical Alpha 8 notes)
- Feature parity is incomplete: general active-transfer pause/resume (eligible single Original transfers now supported), named default profile, authenticated sessions, diagnostics/export, local and remote optional AI, clean-machine dependency setup, full recovery and privacy/lifecycle verification.
- Actual external-site thumbnail and physical tray click, outside click, keyboard, DPI and multi-monitor interactive checks are not replaced by geometry/native-window tests.
- Clip and subtitle handling are implemented, but subtitle extraction on real sites and archive output still need fixtures. Original-format video clips are converted to MP4; audio clips retain supported audio format.
- Idle measurements exist for the non-equivalent Tauri and Win32 prototypes; no equal-feature performance comparison or public signed installer. No claim of universal site support or complete parity.

## Design and audit files
UX-RESEARCH.md, BASELINE-PARITY.md, BASELINE-SYMBOLS.json, MULTI-PERSPECTIVE.md, research-alpha3.md, research-alpha4.md, research-alpha5.md, research-alpha6.md, research-alpha7.md, DECISION-JOURNAL.md, ITERATION-LOG.md, NEXT-PASS.md. These are the migration contract, not marketing claims.

Use media you have permission to download. Do not publish any personal app state, credentials, or generated executable as source.

Alpha 5 adds named format/resolution/archive profiles, ordered title rules, optional absolute category paths, installed-tool detection, redacted diagnostics, generic error categories, pre-output network retries and English form validation. Parallel jobs are experimental until repeated media regression and real-site gates pass. These Alpha 5 limitations are superseded in part by Alpha 8 below.

## Quick start (Windows development candidate)
Run the EXE, copy a media link, open QuickClip321 from the notification area, select a preset, and press Download. The app captures links when clipboard detection is enabled; it does not auto-download. Queue toggles its own view. Change selects a manual folder; Reset appears only after manual selection. The global organizer switch is in Advanced > Settings. Detailed categories and custom rules are in Organizer. Use only authorized media. Use existing tools or consent to the managed setup; it is not a clean-machine installer.

Alpha 7 catalog: 317 hip-hop/related artists, 336 films, 310 series and 171 games. 20-item development fixture, not a real-world accuracy estimate. Optional Clean, Glass-style, Carbon and Sand finishes. See Alpha 8 evidence and remaining gates below.

## Alpha 8 development evidence
2302 curated catalog entries; 1485 music artists/groups, 336 films, 310 series, 171 games. Conditional format controls, verified managed dependency setup, supervised processes and scoped byte-resume. Separate native prototype in native/ is a feasibility experiment, not the complete application. See RELEASE-GATES.md, research-alpha8.md and RELEASE-README-DRAFT.md.

## Public beta 2
Windows completion/failure notifications, startup toggle enabled by default, saved appearance and position, separate format/resolution, and clip reset on new URL. Beta 2 passed 25 core tests and isolated restart/appearance UI tests. Full native frontend parity, clean VM, signing and broad source coverage remain open. See the root README and release notes.

## Beta 3 first-launch test
Run python v3/first_run_test.py v3/desktop/target/release/quickclip321-v3.exe from the repository root with no normal instance running. The runner needs FFmpeg solely to create a synthetic media fixture. The EXE is launched twice with isolated state and a PATH containing no media tools. Real pinned HTTPS setup, first MP4 H.264/AAC download, restart and no-reinstall are checked. This is not a clean VM certification.
