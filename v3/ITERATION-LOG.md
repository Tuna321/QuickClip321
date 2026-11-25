# Alpha 2 iteration evidence

Reference: original Python RC2 commit ec9e9a1; pre-pass Rust commit abc660f. Kept both intact. No personal state committed.

Pass 1: editor and new-user workflow. Restored tray-first identity, compact quick view, independent format, remembered defaults, presets, optional bounded metadata, batch, queue controls, clipboard, basic organizer, clipping, subtitle and archive options.

Pass 2: Windows integration and reliability. Added monitor work-area geometry and tested negative monitor coordinates, custom placement and corner. Added native window smoke assertions rather than merely checking that HTML loads. Initial smoke found the native taskbar style wrong despite the Tauri configuration. Added explicit WS_EX_TOOLWINDOW and removed WS_EX_APPWINDOW on the QuickClip window only. Rebuilt and repeated the native smoke.

Verified on Windows: 9/9 Rust tests including actual local synthetic HTTP media preview/download, H.264 conversion, clip, organizer folder and subtitle time-shift unit test. Node syntax and Tauri cargo check passed. Optimized build succeeded. Native smoke all true: UI ready, snapshot, initially hidden, frameless, always-on-top, tray object, no taskbar native style, work area bounds, hide and show. Then launched the candidate normally in hidden tray mode.

Remaining release blockers: see README and baseline parity matrix. Physical tray click and outside-click behavior, real-site subtitle/archive, custom organizer/profiles, auth, pause/resume, concurrency, notification, thumbnail, diagnostics, engine bootstrap, AI and clean-machine tests remain unverified or incomplete. Do not treat startup smoke as full usability testing. No real participant studies or benchmark claims.

Next pass: NEXT-PASS.md. The current chat cannot be submitted to itself programmatically through available chat tools. Iterations above were performed in the current turn; no autonomous loop was scheduled.

# Alpha 3 iteration — 2026-11-26
Three scoped research passes (interaction/competition, Windows/accessibility, reliability/release), baseline source recheck and V3 diff checks. See research-alpha3.md and DECISION-JOURNAL.md. These are desk audits, not three independent comprehensive product audits or participant studies.
- Native clipboard-sequence watcher (350 ms), memory-only valid link inbox, focused UI accepts it automatically on show. New auto-fill may replace previous auto-fill; typed drafts protected with explicit Use copied link. No hidden network preview/download, no clipboard logging. Actual clipboard contents were not altered for tests; smoke uses seeded inbox, so real OS copy-to-tray interaction remains a manual check.
- Native 24 logical px rounded window diameter, GDI region refreshed on size/DPI change, cached geometry avoids redundant reapplication. OS corner clipping verified in smoke, no screenshot or multi-monitor visual validation.
- Six concrete appearance palettes (Dark, Light, Midnight, AMOLED, Forest, Sand), System follows OS, six accents (Purple, Graphite, Ocean, Emerald, Rose, Amber). Theme token coverage replaces partial overrides. Verified 36 computed WebView theme/accent combinations for body/button/input color contrast >=4.5:1 and distinct backgrounds/accents; preference serialization covers all 42 including System. Not a full accessibility certification.
- Quick queue button, clear link, Ctrl+L, protected pending clipboard action, stable job rendering and native progress bars. Atomic bulk cancellation/clear completed tested; no change to sequential scheduling.
- Review checks: JS syntax, cargo check, cargo fmt, 11/11 core tests including synthetic actual download, preference roundtrip and bulk cancellation. Release build succeeded in 34.79s. Native smoke all true including rounded, theme_matrix, clipboard_flow, hidden tray, no taskbar and work area; exit 0. Normal candidate launched PID 13572. SHA256 6E20636200C4BF227979FB1A61BBFCEAE5AECA9CCB4911D1BFF552D639350724 (11,128,832 bytes). Alpha 2 executable and state backups preserved locally. Remaining baseline gaps are listed in README; no public release or speed claim.
- Next: NEXT-PASS.md (bounded next turn). No automated message submitted to this chat.
