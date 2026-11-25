# QuickClip321 V3 — experimental Rust/Tauri migration

This is an alpha, not a replacement for the published Python app. It uses Rust for queue and process orchestration, a local web UI, and existing trusted yt-dlp and FFmpeg tools.

## Build on Windows

Install Rust stable (MSVC), Visual Studio C++ Build Tools, and WebView2. From this folder:

```powershell
cargo test --manifest-path core/Cargo.toml
cargo build --manifest-path desktop/Cargo.toml
cargo run --manifest-path desktop/Cargo.toml
```

In Settings, select existing trusted yt-dlp.exe and ffmpeg.exe, then choose an absolute destination. Paste a URL and enqueue. Use the tray icon to show/hide the window. Quit is blocked during active downloads to avoid abandoning child processes.

The V3 app has separate app data, and this migration does not alter the Python version. Current gaps include authenticated downloads, automatic updates, clip trimming, metadata preview, subtitles, organizer rules, full codec/container compatibility testing across real sites, pause/resume, full diagnostics, and bundled tools. Retry starts a new single-item job. The alpha intentionally uses an explicit Paste button instead of monitoring the clipboard. Editor MP4 inspects H.264/AAC and converts incompatible video before publishing; it requires ffprobe next to FFmpeg. A local synthetic HTTP fixture exercises the actual downloader and conversion.

Only download media you have permission to use. No stability or performance improvement is claimed until measured.
