# Release readiness — Alpha 8 gate (2026-11-26)

The requested public release is **conditional** on passing all required gates, including idle single-process native UI. Do not change the main branch, release tag, download link, or repo visibility before these pass. Keep RC2 and Alpha7 backups.

| Gate | Required evidence |
|---|---|
| Single-process idle | Measure full child process tree on equivalent isolated hidden startup, three repeats; no WebView/Chromium child at idle. yt-dlp/FFmpeg/Node subprocesses may exist only when doing work. |
| Native frontend parity | All current tray, quick/advanced, metadata/thumbnail, theme, settings, queue, clipboard, profile, organizer, accessibility, DPI, and keyboard workflows. Raw Win32 candidate is a feasibility prototype, not an equivalent replacement. |
| Transfer correctness | Exact-byte HTTP Range resume after controlled pause and persisted restart; safe cancellation and process supervision. Broader resume and crash recovery, playlists, sidecars and output publication need separate fixtures. |
| Dependencies | Official immutable version and archive/binary SHA-256 pins, bounded HTTPS download, exact archive members, no global installs, tamper refusal, actual empty-root install and tool execution, clean VM and upgrade/rollback. |
| Compatibility | Authorized public-source preview and download checks per extractor, quality, metadata, subtitles, archive; local fixture tests cannot establish universal support. |
| Security/privacy | No secret/token/cookie/state/clipboard/user paths in git, verify allowed IPC, dependency audit and licenses, safe output names, no overwrites, process lifetime and crash recovery, signed installer or clearly disclosed unsigned package. |
| UX | Actual screenshots, compact workflow, optional advanced controls, contrast, keyboard, high DPI, multiple monitors, accessibility. Real user task testing and installation on a clean machine. |
| Packaging | Reproducible build, x64 release assets, SHA256SUMS, clean installation/update/uninstall, license notices and versioned release notes. |
| GitHub | Product README using existing brand and real scrubbed screenshots, working download link only when artifact exists, concise getting started, limitations, support/contribution, MIT license, reviewed topics/repository metadata. |

## Primary references
- Microsoft WebView2 process model: https://learn.microsoft.com/microsoft-edge/webview2/concepts/process-model
- Microsoft Windows development best practices: https://learn.microsoft.com/en-us/windows/apps/get-started/best-practices
- Windows packaging and direct distribution: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/publish-first-app
- Microsoft Store readiness (adapted for direct distribution): https://learn.microsoft.com/en-us/windows/apps/publish/store-readiness-checklist
- Windows Job Objects: https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects
- GitHub repository best practices: https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories
- Slint native backend and licensing options: https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/ and https://slint.dev/faqs
- yt-dlp official release https://github.com/yt-dlp/yt-dlp/releases/tag/2026.11.23
- FFmpeg Windows build https://github.com/GyanD/codexffmpeg/releases/tag/9.1
- Node LTS checksum https://nodejs.org/dist/v24.22.3/SHASUMS256.txt

Local Codex skill read: readme-design/SKILL.md. Apply its product landing structure and genuine visual proof; no invented metrics or download links. Existing public RC2 page remains truthful until replacement is certified.

## Alpha 8 observed results (not a release authorization)
- Core: three final suites of 25 passing tests each, external network test separate. Verified Range reuse and exact output, scoped safe resume, actual child kill, managed-tool integrity reuse and tamper refusal. Earlier overlapping build failed LNK1104; rerun sequentially passed.
- Native Win32 prototype: 3/3 idle measurements had one process; working set 10.82-10.86 MB, private bytes about 1.6 MB. Three isolated basic UI checks passed.
- Tauri: 3/3 isolated idle measurements had seven processes; working set 362.32-365.14 MB, private 181.74-183.89 MB. Not an equivalent-feature comparison; prototype does not satisfy frontend parity.
- Managed setup: actual new isolated root installed and executed four verified binaries. Clean VM and install/upgrade/uninstall not verified.
- Real-source preview: YouTube title+thumbnail passed, Wikimedia title only, Vimeo unavailable. No general compatibility certification.
- Native full UI parity, notifications, broad resume/recovery, clean-machine package, signing/license/dependency audit and physical DPI/accessibility/monitor checks remain OPEN. Public release blocked.
## Beta 2 release scope
The owner explicitly requested public beta feedback without waiting for all stable gates. The current Tauri UI, not the separate native prototype, is the released artifact. Stable gates above remain open; unsigned ZIP and WebView2 dependency disclosed. Core, UI, package extraction, SHA-256 and actual launch checks passed. Previous V2 releases retained.
