# V3 dependency and license information

QuickClip321's own code is MIT-licensed. The Windows portable ZIP includes the project's license and collected registry dependency license texts and notices. The resolved dependencies and versions are listed in [desktop/Cargo.lock](../v3/desktop/Cargo.lock). Tauri and its WebView2 integration have their own upstream terms. The Windows WebView2 Runtime is installed separately.

The ZIP does not distribute yt-dlp, FFmpeg, FFprobe or Node.js. Managed setup downloads pinned releases directly from upstream and checks SHA-256 hashes before use. See the [setup source](../v3/core/src/setup-tools.ps1) for exact assets and checksums.

- [yt-dlp](https://github.com/yt-dlp/yt-dlp/releases/tag/2026.11.23) ? upstream release and licensing.
- [FFmpeg Windows build](https://github.com/GyanD/codexffmpeg/releases/tag/9.1) ? publisher, licensing and source information.
- [Node.js](https://nodejs.org/dist/v24.22.3/) ? runtime distribution; license extracted by managed setup.

For older Python release notices, see [V2 licensing](THIRD-PARTY.md).
