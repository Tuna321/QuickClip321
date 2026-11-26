# Alpha 5 — design research, concept generation, refinement (2026-11-26)
Scope: three structured desk-review passes of ten topics and corresponding concepts, each followed by five refinement decisions. Not 30 separate studies or 15 real participant sessions. Inputs: original Python popup/settings/profiles/organizer, Alpha 4 Rust/JS/CSS and actual prior captures, NEXT-PASS and project notes. Sources are documentation and competitor descriptions, not hands-on competitor benchmarks.

## Pass A — returning editor, speed and recognition
| Research topic / evidence | Concept |
|---|---|
| 1. NN/g recognition vs recall https://www.nngroup.com/articles/recognition-and-recall/ | Named reusable presets rather than remembering format/quality |
| 2. NN/g progressive disclosure https://www.nngroup.com/articles/progressive-disclosure/ | Keep new settings away from quick download controls |
| 3. Parabolic concurrent jobs https://github.com/NickvisionApps/Parabolic | User-selectable 1–3 simultaneous jobs |
| 4. Parabolic release thumbnail https://github.com/NickvisionApps/Parabolic/releases | Preview doesn't impede download |
| 5. MediaHuman interface https://www.mediahuman.com/howto/user-interface-in-detail5.html | Remove history record without deleting output |
| 6. MediaHuman clipboard changelog https://www.mediahuman.com/youtube-downloader/changelog.html | Filter copied URLs and protect typed draft |
| 7. Open Video Downloader shortcuts https://github.com/jely2002/youtube-dl-gui | Maintain keyboard access and quick queue |
| 8. Original profile schema | Save name, format, resolution, archive flag |
| 9. Original organizer rules | User-defined phrase/category mapping plus folder override |
| 10. Original Quick View | Retain tray identity; no dashboard redesign |
Refinements: (1) separate Profiles page instead of adding another quick-view row; (2) named presets selectable in existing dropdown; (3) case-insensitive duplicate detection; (4) separate organizer page with explicit save and ordered rules; (5) preserve manual destination and no automatic downloads.
## Pass B — new user, aesthetics, accessibility
| Research topic / evidence | Concept |
|---|---|
| 1. Windows spacing https://learn.microsoft.com/en-us/windows/apps/design/basics/content-basics | Align existing controls, consistent spacing |
| 2. Windows typography https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography | Clear labels and hierarchy without decorative banners |
| 3. Windows inclusive design https://learn.microsoft.com/en-us/windows/apps/design/accessibility/designing-inclusive-software | Keep keyboard order, labels and theme contrast |
| 4. WCAG contrast https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html | Verify theme matrix in WebView |
| 5. WCAG focus https://www.w3.org/WAI/WCAG22/Understanding/focus-visible | Visible keyboard focus and no destructive rerender |
| 6. WCAG targets https://www.w3.org/TR/WCAG22/ | Keep actual action hit areas sufficient, review small help targets later |
| 7. Windows tooltips https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/tooltips | Context help for concurrent downloads |
| 8. NN/g error prevention https://www.nngroup.com/articles/ten-usability-heuristics/ | English validation, no browser-localized tooltip |
| 9. Windows responsive sizes https://learn.microsoft.com/en-us/windows/apps/design/layout/screen-sizes-and-breakpoints-for-responsive-design | Preserve compact flyout and single advanced scroll |
| 10. Alpha4 screenshot comparison | Persistent save action, avoid white border or clipped controls |
Refinements: (1) compact configuration cards with explicit actions; (2) diagnostics separate from general settings; (3) use native form semantics but manual English validation; (4) preserve actual theme tokens; (5) test rendered captures, not just HTML. No full screen UI rewrite.
## Pass C — reliability, release, recovery
| Research topic / evidence | Concept |
|---|---|
| 1. yt-dlp continue semantics https://github.com/yt-dlp/yt-dlp | True active resume needs durable staging; don't fake it |
| 2. yt-dlp progress templates (same docs) | Keep engine-derived speed and ETA |
| 3. NN/g status https://www.nngroup.com/articles/visibility-system-status/ | Distinguish paused waiting jobs, active jobs, failures |
| 4. NN/g complex apps https://www.nngroup.com/articles/usability-heuristics-complex-applications/ | Actionable failure categories |
| 5. Parabolic concurrent jobs | Bound scheduler slots, job-specific temp files |
| 6. MediaHuman settings backup FAQ https://www.mediahuman.com/youtube-downloader/faq.html | Preserve state and EXE before changes |
| 7. Open Video Downloader settings https://github.com/jely2002/youtube-dl-gui/wiki/Settings | Avoid unnecessary queries/polling when hidden |
| 8. Original dependency handling | Detect installed tools; verified bootstrap remains separate |
| 9. Original diagnostics | Redacted report rather than leaking arbitrary stderr |
| 10. Original recovery queue | Persist paused waiting jobs; test cancellation and restart |
Refinements: (1) cap concurrency 1–3, default one; (2) queued Hold/Resume only until real active resume implemented; (3) use atomic persisted settings and separate settings-section writes; (4) classify errors without including credentials or URL in diagnostic report; (5) run repeated regression tests with real local HTTP/FFmpeg and attempt licensed external preview. No claims of universal site support.
## Deferred ideas
Native notifications, active pause/resume with durable job-owned staging, verified engine bootstrap, authentication, per-monitor selection, original AI opt-in, full accessibility/DPI tests, detailed diagnostics, custom profiles as default by name. Do not substitute a simulated feature for these. Current design choices prioritize correctness and original workflow.
