# Independent perspectives / iteration priorities
Editor: tray-to-timeline is the product. Never make me enter engine paths if installed; no extra clicks; target 1080 H264 by default; preserve destination and profiles.
New user: clearly identify first-run missing tools, explain next action, no scary internal CLI jargon; transparent dependency size; no account.
Power user: batch, original video/audio, explicit playlist, pause/resume, custom rules, predictable settings.
Accessibility: keyboard and screen-reader labels, text contrast, usable at high DPI, status not color-only, no focus stealing while hidden.
Security/privacy: no remote HTML in privileged UI; strict IPC; no automatic cookie reads; avoid URL/token persistence or log disclosure; only job-owned directories, no shell interpolation; process tree cleanup; no unverified dependency installation.
Reliability: old baseline is an executable specification. Keep old app and data untouched. Test actual workflows, not only startup. Failure recovery and file preservation matter more than a flashy redesign.
Performance: Rust for Windows lifecycle and orchestration, use existing yt-dlp and FFmpeg for extraction and encoding (do not recreate them). Benchmark before replacing a subsystem. Avoid rebuilding whole UI on every polling cycle.
Release: branch-only migration; no false stable tag. Pass parity matrix, regression, clean Windows, unsigned warning and license gate before promotion.
Pass order: 1 tray identity and setup; 2 preview + presets/format + queue; 3 organizer/media options; 4 remaining history/security/diagnostics/settings; 5 repeat independent review and test. Do not mark a row complete on intent alone.
