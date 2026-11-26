# Beta 5 feedback-first candidate

## Implemented
- English/Turkish UI, common messages, and language-aware shortcut notices.
- Separate Shortcuts page: global and in-app bindings, custom key capture, disable,
  reset defaults, registration conflicts, trigger counters, 30-second non-downloading test mode.
- Global copied-link download uses last format, quality and destination, and the
  last manual/organizer choice. MP3/MP4 shortcuts override format; MP4 uses last
  video quality. Never reuse previous clip range, archive or subtitles on a new link.
- Defaults: Ctrl+Alt+Space open/hide, Ctrl+Shift+D last format, Ctrl+Shift+3 MP3,
  Ctrl+Shift+4 MP4, Ctrl+Shift+O folder. Ctrl+Enter, Ctrl+L, Esc are in-app.
- Small top-center localized fade notice for global shortcut action; no focus grab.
- Always on top toggle, on by default; off demotes window and restores quick-popup
  blur hide. Batch URLs remain.
- Remove long outdated clip/pause explanation from Home.
- Source transfer size estimate, not exact re-encoded output size.
- Catalog: 2,861 curated records and 22,832 normalized additional Wikidata labels
  (4,853 games, 7,663 films, 4,748 series, 5,568 musical groups); total 25,693
  before runtime normalization. Imported labels require exact contextual title
  boundaries or metadata; not a universal recognition guarantee.
- Fast section download first for eligible single short video clips, full download
  then trim on pre-publication failure, validated output duration.
- Partial-file frame preview: bounded local decode, one attempt per second while
  active UI, no extra media request. Keep thumbnail when file is not decodable.

## Evidence
- 28 core tests passed (external metadata test excluded).
- Real HTTP range fixture: 1s clip from 15s, 786,432 / 7,074,938 bytes on latest
  run, no fallback; duration 1s. Not a public-site benchmark.
- Two distinct JPEG frames decoded from truncated faststart MP4 fixture.
- Initial native smoke caught taskbar style regression after changing topmost;
  fixed native z-order changes and repeat smoke is required before publication.

## Remaining gates
- Native UI/physical keyboard and actual enqueue/overlay verification.
- Verify unsupported source fallback on independent real sources; preview on
  incomplete MP4 with metadata at end cannot work until header is available.
- Broader language coverage for backend diagnostic messages.
- Clean machine/WebView2, slow-network validation, native parity remain open.
- Keep public Beta 4 until gates pass. No ordinary download selector/concurrency
  changes. No unverified speed superiority claim.


## Final candidate validation
Native notice destroys its HWND after fade; three consecutive tests passed without focus change. Final workflow-fixed EXE passed isolated preference write/read across process restart and native UI smoke. Link consumption preserves unrelated drafts and OS clipboard. Maintainer approved public beta after personal tests. Remaining limitations: site-specific preview, clip fallback coverage and diagnostic translations; clean VM certification remains open.
