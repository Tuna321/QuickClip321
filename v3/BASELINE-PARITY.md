# Original QuickClip321 baseline contract
Reference commit ec9e9a1e3ba81b2a14eece66d551bcc15d703de7, Python RC2. See generated BASELINE-SYMBOLS.json for exact source paths, line ranges, test names; original files remain in repo. Status below refers to V3 before this pass, not a claim of parity.
| Area | Original behavior and reason | V3 gap |
|---|---|---|
| Tray/popup | pystray, single instance, hidden mode, no taskbar, borderless, click icon toggles, monitor work area, DPI, icon/corner/custom placement, outside click and Escape, suppress dismiss for own dialogs | taskbar, frame, startup visible, positioning, dismiss, custom |
| Quick view | 600x365 adaptive, logo/Hide/Advanced, URL/Paste/Download, independent preset/format/resolution, preview title/thumb, destination Change/Auto, playlist controls conditional, stage/progress | partial; separate format, real preview and auto destination missing |
| Advanced | 930x720, retains quick area, Downloads/Queue/History/Settings/Diagnostics, batch drafts survive nav | partial |
| Presets | Premiere Ready MP4 1080, Fast Download original best, Audio MP3, Archive original + metadata; custom profiles | partial |
| Preview | debounce 800ms, bounded separate process, 25s deadline, title/duration/channel/quality/playlist count, optional bounded thumbnail, stale results dropped, never blocks download | missing |
| Playlist | one item default, recognized URL/preview, explicit full-list confirmation, reset consent on input/restart, no implicit full download | basic only |
| Clipboard | detect every 2s and on show; never auto-open or auto-download; settings switch; avoid replacing draft | manual paste only |
| Queue | persistent SQLite; queued/analyzing/downloading/processing/verifying/retrying/paused/completed/failed/cancelled, progress/speed/ETA, max 1-3 concurrent, pause/resume, cancel all/clear pending, retry, dedupe, retain partial by policy | sequential, limited states, no pause |
| Download | yt-dlp Python API, independent processes, explicit auth, bounded retries/fragments, JS runtimes, job-specific staging, no overwrite, safe path validation, manifest, verify outputs, collision-safe publish, fallback if organizer path unavailable | basic CLI, lacks many safety/resume details |
| Media | original format, H264/AAC/yuv420p MP4, MP3/M4A/WAV, clip start/end seconds, subtitle language/SRT/VTT/auto, trim subtitle timestamps, archive metadata; local FFprobe inspection | basic format and editor conversion only |
| Organizer | local deterministic Unicode normalization, custom title rules first, music/movie/TV/rap/beat signals, confidence/reason, per-category paths, manual destination wins, per-job override, no scanning existing files | missing |
| Auth | Chrome/Edge/Firefox/Brave or cookies.txt explicit per-job only; session credentials not stored; redacted error/support reports | missing |
| AI | local suggestions and explicit OpenAI/Ollama opt-in, off default, only displayed metadata, secret store | missing |
| Settings | startup quick/advanced/remember, System/Light/Dark, Purple/Graphite/Ocean, tray/corner/custom XY, clipboard/notifications/organizer, concurrent 1–3, fragments 1–8, partial policy, retries/stall, profiles | mostly missing |
| History | open/reveal folder, retry, partial folder, remove records only, preserve files, clear completed | partial |
| Diagnostics | classify errors, raw local log, safe copy/export redacted report, inspect local file | missing |
| Distribution | portable isolated state, bundled Node, checksum-pinned on-demand FFmpeg, license notices, frozen network/UI/destination smoke | missing |
| Reliability | single instance, process tree ownership, shutdown, SQLite recovery, safe storage, error taxonomy, no persisted auth | partial |
No public stable V3 until each row is implemented and verified, including real tray interaction. Keep the original usable.
