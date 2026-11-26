# Nightly 2 QA — 2026-11-26

Local release candidate: 3.0.0-beta.6.nightly.2. Public Beta 5 and main are unchanged.

## Evidence
- Core suite: 30 passed, 0 failed, 1 live metadata test excluded. Additional crash recovery / idle update guard test passed separately.
- Desktop unit suite: 5 passed; UI smoke: 14/14.
- Actual app, local HTTP: 1,868,278 bytes, unlimited 3.14 s, cap 256 KiB/s 11.09 s, identical output bytes. Includes startup overhead; not an internet speed benchmark.
- Exact nested Explorer folder including spaces; duplicate warning and deliberate redownload; no overwrite.
- Actual completed job produced native completion notification, which did not steal focus and was destroyed.
- Settings export/import round trip; private paths excluded.
- Real HTTP Range recovery: nonzero offset reused, completed output identical to original. Unsafe staged output preserved. Simulated abrupt shutdown state recovery and idle update guard passed.
- Pinned tool verification/reuse and previous yt-dlp rollback fixture passed.
- Cold isolated installation: actual H.264/AAC MP4, 23.2 s including setup; second launch reused installed tools and downloaded successfully.
- Signed portable replacement Nightly 1 to Nightly 2 on isolated copies: health marker, old EXE backup, settings preserved. Modified manifest, wrong channel, modified executable rejected. Signed invalid fixture executable triggered rollback.
- Local fixture partial preview and 1 s clip extraction passed.

## Release gates still open
- Public signed stable/nightly releases and full network update/notification path, all three update modes, cross-channel switch, clean Windows VM and separate PC.
- Power interruption during updater swap is not certified; previous EXE/state backup retained, manual recovery may be required.
- Broad live-site/protocol/format coverage, fallback clipping and preview compatibility, real network speed limits and real OS forced shutdown are not universally certified.
- Observe Nightly for 1–2 days before promoting a stable candidate. Beta 5 is an unsigned legacy public prerelease with no updater, not a signed stable channel. Manual verified download is provided separately; back up settings before downgrading.
- Drag/drop deliberately omitted. Custom file names deferred pending user decision. Browser extension deferred; no SponsorBlock.
- Current normal Nightly 2 restarted successfully. No GitHub publication performed.
