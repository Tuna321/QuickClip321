# Alpha 7 — ten independent hypothetical user perspectives
These are separate scenario-based evaluations, not real interviews, agents or observed participants. Each was considered from a distinct goal and then reconciled against original Python and Alpha 6.
1. First-time downloader: I want to copy a link, open the tray and press Download. Avoid setup jargon and redundant buttons. Keep the default path obvious, offer optional tool setup only when required.
2. Returning video editor: I want predictable H.264/AAC and a chosen resolution. Keep Premiere Ready and explicit format/quality; don't make me configure the same job each time. Manual destination must win.
3. Music collector: I want MP3 or original audio and organized folders without managing a database. Keep audio presets, custom paths and organizer behind settings, not quick checkbox.
4. Playlist user: I need a clear distinction between single media and entire playlist; don't queue hundreds accidentally. Show checkbox only with URL or extractor evidence; always reset consent for new links.
5. Slow connection user: I need pause and resume after interruptions, not download from zero. Actual active resume remains a release blocker. Keep partial files and report errors plainly.
6. Power user: I want batch jobs, concurrency, profiles, subtitles and clipping. Don't remove features; separate advanced pages from fast path. Queue opens and closes with same button.
7. Accessibility/keyboard user: I want stable focus, labels, readable contrast, reduced motion, and no controls that appear unexpectedly. Keep keyboard focus, i help and native inputs. Small help targets and full screen-reader checks remain.
8. Privacy-conscious user: I don't want a copied link to trigger hidden network requests or automatic downloading. Clipboard can be disabled; draft protected; no credentials in reports. Keep private local state out of commits.
9. Gamer/content creator: I want gameplay, trailers and game clips routed separately, but Rust programming and film adaptations shouldn't be mistaken for games. Add contextual catalog, aliases, optional folder; test negative cases.
10. New-machine user: I want a trustworthy install that works without searching for executables. Detect installed tools and explain missing ones. Verified bootstrap, signatures, clean Windows test still required before stable.

## Real external feedback collected this pass
- https://www.reddit.com/r/youtubedl/comments/1h5mksh — user wants a straightforward GUI without having to learn options.
- https://www.reddit.com/r/MediaDownlodr/comments/1p3l4p6/is_there_an_automatic_copypaste_feature/ — requested clipboard capture feeding queue/batch.
- https://www.reddit.com/r/macapps/comments/1f7vnu6/clipgrab_a_free_alternative_to_downie/ — appreciated clipboard capture waiting for confirmation.
- https://www.reddit.com/r/youtubedl/comments/1l4o8vt — discussion notes clipboard monitoring crash report.
- https://github.com/NickvisionApps/Parabolic/discussions/1798 — frustration with interrupted download recovery and playlist rediscovery.
- https://github.com/NickvisionApps/Parabolic/issues/1937 — queue stuck when overwrite disabled.
- https://github.com/NickvisionApps/Parabolic/issues/1857 — leftover partial files associated with HTTP 416.
- https://github.com/NickvisionApps/Parabolic/issues/1719 — individual playlist quality/format selection desired.
- https://github.com/NickvisionApps/Parabolic/issues/1597 — maximum resolution requested for playlist to control storage.
- https://github.com/o7q/MediaDownloader/releases — bootstrap progress, playlist folders and reuse raw media are recurring maintenance topics.
- https://www.nngroup.com/articles/progressive-disclosure/ and https://www.nngroup.com/articles/ten-usability-heuristics/ — defer rarely needed controls and prevent errors.
Feedback is anecdotal, not representative statistics. The ten personas above are modeled scenarios, not those commenters' identities.

## Consolidated actions
Remove Hide and quick Auto-organize; preserve Escape/tray dismissal and saved global organizer toggle. Replace always-visible Auto with Reset shown only after manual destination. Queue button toggles advanced queue; selecting queue from another page opens it rather than collapsing other view. Preserve contextual playlist. Avoid cosmetic changes that weaken usability.
## Catalog five stages
1. Identify gap: gameplay treated as ordinary video, overlapping game/movie titles.
2. Curate 171 game records; canonicalize entries with deduplicated names. Do not claim comprehensive catalog or use copyrighted cover art.
3. Add selected aliases (GTA 6, RDR2, CS2, Skyrim, etc.) and route games to optional Video/Games, preserve default Video.
4. Require game metadata/gameplay or distinctive title+trailer context; avoid bare Rust/Portal/Control. Preserve custom rules and explicit screen metadata.
5. Test every game record with explicit metadata, a 20-item labeled sample with six actual publicly documented titles and fourteen adversarial synthetic cases. Sample is small and partly used for tuning; no unbiased real-world accuracy estimate.
Source fixture: core/src/organizer_sample.json. Public title references included inline. Test report per-class TP/FP/FN.
## Three review passes
A: usability — compare source and Alpha6 real screenshots, remove three redundant quick controls, test queue/destination state.
B: classification — review aliases, precedence, game overlap, actual titles, custom mapping and data count.
C: safety/release — confirm backup and original unchanged, no destructive publication, repeated core tests and isolated native smoke, verify release blockers.
