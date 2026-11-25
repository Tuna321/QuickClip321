# Quality and distribution roadmap

Baseline: 2026-11-25, v2.1.0-rc.2. GitHub reported 2 stars, 5 downloads of the current EXE and 2 of the previous EXE. Counts are downloads, not unique users or installations.

## P0 — Make the first download reliable

- Add Windows CI for locked dependencies, pytest, Ruff, and a synthetic local FFmpeg fixture; ensure required integration tests cannot silently skip.
- Run packaged smoke tests on clean Windows 10/11 environments without Python, Node or FFmpeg preinstalled.
- Add a testable processing watchdog with progress-aware timeouts and safe cancellation. Do not reuse the network timeout for valid long transcodes.
- Review history privacy and authorization lifetime; preserve retry behavior while discarding credentials when no longer needed.
- Confirm license notices and reproducible hashes; publish a stable release only after release checks pass.
- Document first-use FFmpeg size and verify unsigned executable behavior. Evaluate code signing without asking users to disable security.

## P1 — Distinguish the product

- Benchmark link-to-ready-file time, download throughput, conversion time and resource usage separately against Parabolic on the same hardware and authorized fixtures.
- Focus on tray workflow, Premiere-ready output, batch clipping, local organization and actionable diagnostics.
- Design a verified engine-update strategy with rollback; do not execute arbitrary downloaded binaries.
- Add clear first-run setup progress and recovery for missing or corrupt tools.
- Evaluate drag-and-drop editor workflow and configurable project destinations.

## P1 — Distribution

- Suggested GitHub topics: yt-dlp, youtube-downloader, video-downloader, audio-downloader, media-downloader, windows, python, ffmpeg, system-tray, open-source.
- Publish a stable version with checksums and a short demonstration of the tray-to-editor workflow.
- Create a small landing page with screenshots, exact Windows requirements, direct download, source, and troubleshooting.
- Evaluate WinGet portable manifests after stable release and clean-machine testing; submit to the community repository only after validation.
- Share a useful demo with relevant video-editing and open-source communities where self-promotion is permitted; no automated spam.
- Submit to software directories only with verified claims and current release links.

## Measurement

Track EXE asset downloads by version, stars, issues, and release regressions. GitHub download counts do not establish unique installations or active users. If measuring landing-page conversion, use aggregate privacy-preserving analytics and disclose it; avoid adding app telemetry by default.

References:
- https://github.com/NickvisionApps/Parabolic
- https://learn.microsoft.com/en-us/windows/package-manager/package/
- https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation
- https://docs.github.com/en/rest/releases/releases
