# Design decision journal (engineering record)
- Keep original application identity: one tray click to compact controls, not an app dashboard. Avoid a language rewrite just to reduce bundle size.
- Clipboard capture is a convenience, not authorization to download. Allow off switch; no clipboard logs; preserve typed drafts, expose latest copied link explicitly. No hidden network preview.
- Prefer predictable visual contrast over novelty. Add genuinely different background palettes and separate accents; test the combination rather than counting theme names.
- Native region is chosen over relying solely on a cosmetic CSS radius or an OS hint; tradeoff is possible loss of DWM shadow. Verify real region on Windows.
- A startup smoke is not an interaction test. Include computed styles, persisted preferences, native window assertions and pure clipboard transition cases. Do not touch the user's clipboard for automated tests.
- Keep private decision record at the level of rationale, alternatives, risks and evidence, not hidden reasoning transcripts.
