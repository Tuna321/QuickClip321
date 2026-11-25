# Changelog

## 2.1.0-rc.3 — development candidate (unreleased)

- Preserve chosen resolution and presets while the metadata preview loads or fails.
- Avoid replacing an existing link with new clipboard content.
- Validate malformed URL ports and safely handle incomplete URLs during typing.
- Deduplicate authorized downloads using session-only authorization data.
- Restrict partial cleanup to the owned job directory and reject redirected paths.
- Preserve important worker phase transitions; avoid redundant media verification.
- Recover valid settings individually when other settings are malformed.
- Add regression tests, Windows CI, issue templates and product documentation.
- Add a static product website preview under docs/; hosting is not enabled by this change.

This is source code for a candidate, not a signed or verified binary release. Windows CI, a clean-machine packaged smoke test, and license verification are required before publishing.

## 2.1.0-rc.2 — published preview

- Portable Windows EXE with on-demand, checksum-verified FFmpeg setup.
- Tray quick view, advanced queue/history/settings, clipping, subtitles, presets and local organizer.
