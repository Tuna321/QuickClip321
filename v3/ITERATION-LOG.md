# Alpha 2 iteration evidence

Reference: original Python RC2 commit ec9e9a1; pre-pass Rust commit abc660f. Kept both intact. No personal state committed.

Pass 1: editor and new-user workflow. Restored tray-first identity, compact quick view, independent format, remembered defaults, presets, optional bounded metadata, batch, queue controls, clipboard, basic organizer, clipping, subtitle and archive options.

Pass 2: Windows integration and reliability. Added monitor work-area geometry and tested negative monitor coordinates, custom placement and corner. Added native window smoke assertions rather than merely checking that HTML loads. Initial smoke found the native taskbar style wrong despite the Tauri configuration. Added explicit WS_EX_TOOLWINDOW and removed WS_EX_APPWINDOW on the QuickClip window only. Rebuilt and repeated the native smoke.

Verified on Windows: 9/9 Rust tests including actual local synthetic HTTP media preview/download, H.264 conversion, clip, organizer folder and subtitle time-shift unit test. Node syntax and Tauri cargo check passed. Optimized build succeeded. Native smoke all true: UI ready, snapshot, initially hidden, frameless, always-on-top, tray object, no taskbar native style, work area bounds, hide and show. Then launched the candidate normally in hidden tray mode.

Remaining release blockers: see README and baseline parity matrix. Physical tray click and outside-click behavior, real-site subtitle/archive, custom organizer/profiles, auth, pause/resume, concurrency, notification, thumbnail, diagnostics, engine bootstrap, AI and clean-machine tests remain unverified or incomplete. Do not treat startup smoke as full usability testing. No real participant studies or benchmark claims.

Next pass: NEXT-PASS.md. The current chat cannot be submitted to itself programmatically through available chat tools. Iterations above were performed in the current turn; no autonomous loop was scheduled.
