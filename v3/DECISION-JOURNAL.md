# Design decision journal (engineering record)
- Keep original application identity: one tray click to compact controls, not an app dashboard. Avoid a language rewrite just to reduce bundle size.
- Clipboard capture is a convenience, not authorization to download. Allow off switch; no clipboard logs; preserve typed drafts, expose latest copied link explicitly. No hidden network preview.
- Prefer predictable visual contrast over novelty. Add genuinely different background palettes and separate accents; test the combination rather than counting theme names.
- Native region is chosen over relying solely on a cosmetic CSS radius or an OS hint; tradeoff is possible loss of DWM shadow. Verify real region on Windows.
- A startup smoke is not an interaction test. Include computed styles, persisted preferences, native window assertions and pure clipboard transition cases. Do not touch the user's clipboard for automated tests.
- Keep private decision record at the level of rationale, alternatives, risks and evidence, not hidden reasoning transcripts.

## Alpha 4 decisions
- The screenshot disproved the earlier visual gate: native clipping alone is insufficient. Disable Tauri shadow to remove its documented white border, use 32px physical-DPI diameter, theme-matched shell and control radii. The first window-only PrintWindow capture omitted WebView content; foreground-window-only crop with corner mask gave actual rendered pixels. First visual review exposed double scrollbars and clipped quick progress; revised layout to one advanced scroll region and visible save action. Retest the updated build before claiming it.
- Custom coordinates only relevant in Custom mode; live preview does not persist until saved. Save applies the position immediately. Native smoke now checks actual top-left geometry, not just enum.
- Reuse yt-dlp to obtain thumbnail in an isolated temp folder; no remote WebView image request. Metadata can survive thumbnail failure. Offline base64 crate was unavailable; small encoding function with test vectors instead. Local HTML/PNG and actual synthetic media test passes. Real external sites remain unverified.
- Actual yt-dlp percent, speed and ETA text plus postprocessing stage; idle bar no longer reflects old completed jobs. Contextual help is accessible by button or tooltip.
- Ten targeted topics documented in research-alpha4; not ten full independent rebuilds. Pause/resume/concurrency and other larger parity items remain release blockers.

## Alpha 5 decisions
- Three design lenses and 30 paired research/concept topics, 15 refinements documented in research-alpha5.md; not independent human research. Keep tray quick view unchanged and organize newly restored controls in secondary pages.
- Scheduler uses bounded active map and per-job staging; cancellation applies to specific worker. Hold/Resume only for waiting jobs. Existing active-pause semantics would require persistent staging and restart-safe manifest, so not presented as implemented.
- Custom profiles and title rules stored in backwards-compatible Preferences; separate section commits avoid silently applying unrelated drafts. Manual destination wins. Redacted diagnostics excludes arbitrary URLs and file paths; error classification from stderr is generic.
- Repeated synthetic concurrent fixture caught intermittent failure after initial pass. Isolate yt-dlp cache and make HTTP fixture concurrent, then repeat. This is a correctness gate, not a performance benchmark.

## Alpha 6 decisions
- Default remains clean; optional CSS glass-style, carbon, sand finish are separate from color palettes. No real desktop refraction or expensive backdrop sampling. Respect reduced-motion and forced-colors preferences.
- Contextual playlist visibility uses URL clues and extractor results; stale responses ignored, consent reset per input. Single-item default unchanged.
- Curated local catalog expanded; counts are entries not verified unique artists globally. Conservative contextual matching and positive/negative fixtures, no real-world accuracy claim. Default Audio/Video routing retained; more specific paths opt-in. User rules and manual path override still win.
- Three desk-review passes and three repeated core regressions; not three repetitions of every individual engineering operation. Active-transfer resume, dependency bootstrap and notifications remain deferred.

## Alpha 7 decisions
- Ten distinct hypothetical user scenarios in research-alpha7.md, grounded in actual external feature requests but not real user testing. Retain tray-first workflow; remove Hide and per-download organizer toggle, keep saved global setting, manual override and contextual Reset. Queue toggles rather than always opening. No feature count target.
- Actual Alpha6 native smoke and three foreground finish captures verified. Keep restrained static finishes; avoid a new design language for its own sake. Use source and native UI state tests for the new quick workflow.
- Games: 171 curated entries, aliases, context gate, optional detailed folder, custom mapping. Shared film/game names need evidence, not raw substring matching. Six public titles plus fourteen adversarial examples in a labeled development fixture; no general accuracy assertion.
- Release gates remain separate from usability tests. Active resume with verified byte reuse, dependency bootstrap on a clean system, native notifications and full parity cannot be claimed from a passing startup test. No stable tag or merge to main.

## Alpha 8 decisions
- Keep simple presets and expose format/quality on demand. Foreground visual audit found clipping in the Custom view; expand quick window vertically when details are open.
- Do not suppress or disguise WebView2 processes: a separate native frontend proves single-process idle feasibility, but replacing the complete frontend requires native feature parity.
- Use version-pinned HTTPS artifacts with archive/binary SHA-256, scoped installation and explicit consent. Never execute an unverified download; no global PATH changes.
- Restrict true pause/resume to jobs that can safely reuse partial bytes. Resume result requires actual Range and exact-byte evidence rather than UI labels.
- Use real sanitized screenshots and existing branding in release README draft; no invented metrics, download URL or stable status. Release only after every mandatory gate.
