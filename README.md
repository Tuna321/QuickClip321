<p align="center"><img src="resources/QuickClip321Logo.png" width="96" alt="QuickClip321 logo"></p>
<h1 align="center">QuickClip321</h1>
<p align="center">Free, open-source media downloader for Windows — from a link to an editor-ready file.</p>
<p align="center"><a href="https://github.com/Tuna321/QuickClip321/releases/download/v2.1.0-rc.2/QuickClip321-2.1.0-rc.2-win-x64.exe">Download for Windows (preview)</a> · <a href="https://github.com/Tuna321/QuickClip321/releases">All releases</a> · <a href="#development">Development</a> · <a href="LICENSE">MIT License</a></p>

![QuickClip321 quick view with a link field, download button, format and resolution controls](docs/quick-view.png)

## Built for a faster creative workflow

QuickClip321 lives in your Windows system tray. Choose **Premiere Ready** for editor-friendly MP4 files, **Fast Download** to retain the original format, or **Audio** for MP3. Keep a small quick view for everyday downloads and switch to the advanced workspace when you need more control.

## What it does

- Paste a link, preview the media, and choose a format and resolution from the quick view.
- Add batches, select playlists, trim clips, or save subtitle files in the advanced workspace.
- Follow active downloads, review history, and export redacted diagnostics.
- Organize saved files into Video, Audio, and Unsorted folders using local rules.
- Use the app without an AI provider. Optional AI suggestions are disabled by default.

## Run on Windows

1. [Download QuickClip321 for Windows x64](https://github.com/Tuna321/QuickClip321/releases/download/v2.1.0-rc.2/QuickClip321-2.1.0-rc.2-win-x64.exe).
2. Open the EXE, paste a link, and click **Download**.

Python and Node.js are included. If FFmpeg and ffprobe are missing, QuickClip321
downloads a verified FFmpeg archive (about 114 MiB) on the first download and
stores both tools in its app data folder. Later downloads reuse them. Existing
FFmpeg and ffprobe installed together on PATH or in a `bin` folder next to the EXE
also work.

### Before your first download

- The current release is an unsigned Windows x64 release candidate, not a stable release. Windows may display a reputation warning. Verify the executable against the release's `SHA256SUMS.txt`; do not disable Windows security features.
- The EXE is about 67.5 MiB. First-use FFmpeg setup may download an additional 114 MiB if compatible tools are not already available.
- Supported sites and formats depend on [yt-dlp](https://github.com/yt-dlp/yt-dlp); a specific site or resolution may be unavailable or change over time.

### Troubleshooting and feedback

If a download fails, check the queue's error details and the Diagnostics page. For a reproducible issue, [open an issue](https://github.com/Tuna321/QuickClip321/issues/new) with the app version, Windows version, steps, and a reviewed diagnostic log. Never attach cookies, API keys, private links, or unreviewed personal data.

Have a feature idea? Explain the workflow you want to improve and how you would use it. Contributions and constructive feedback are welcome.

### Run from source

With Python 3.12 and FFmpeg installed, run these commands from the repository root in PowerShell:

```powershell
py -3.12 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements-test-lock.txt
.\.venv\Scripts\python.exe -m quickclip321
```

Use the system tray icon to reopen the window after hiding it. Pass `--start-hidden` to start with only the tray icon. Use media you have permission to download.

## Development

```powershell
.\.venv\Scripts\python.exe -m pytest -q
.\.venv\Scripts\python.exe -m ruff check quickclip321 tests tools
```

To build the public EXE, put the Windows x64 `node.exe` in `bin/`, make FFmpeg and ffprobe available for tests, and run `build_portable.ps1 -PublicRelease`. The script runs checks before packaging. Build output goes to `dist/`.

QuickClip321's source code is [MIT licensed](LICENSE). Bundled components retain their own terms. See [third-party licensing and rebuild instructions](docs/THIRD-PARTY.md).
