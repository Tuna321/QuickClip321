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
