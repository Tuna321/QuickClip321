<p align="center"><img src="resources/QuickClip321Logo.png" width="96" alt="QuickClip321 logo"></p>
<h1 align="center">QuickClip321</h1>
<p align="center">A Windows tray app for downloading and organizing video and audio.</p>
<p align="center"><a href="#run-on-windows">Run on Windows</a> · <a href="#development">Development</a> · <a href="LICENSE">MIT License</a></p>

![QuickClip321 quick view with a link field, download button, format and resolution controls](docs/quick-view.png)

## What it does

- Paste a link, preview the media, and choose a format and resolution from the quick view.
- Add batches, select playlists, trim clips, or save subtitle files in the advanced workspace.
- Follow active downloads, review history, and export redacted diagnostics.
- Organize saved files into Video, Audio, and Unsorted folders using local rules.
- Use the app without an AI provider. Optional AI suggestions are disabled by default.

## Run on Windows

Use Windows x64 with Python 3.11 or 3.12 and `ffmpeg` and `ffprobe` on your PATH. [FFmpeg's download page](https://ffmpeg.org/download.html) links to Windows builds. Some sites may also need a [supported JavaScript runtime](https://github.com/yt-dlp/yt-dlp/wiki/EJS).

From the repository root in PowerShell:

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

For local portable builds, put Windows x64 copies of `ffmpeg.exe`, `ffprobe.exe`, and `node.exe` in `bin/`, then run `build_portable.ps1`. The script runs checks before packaging. Local build output goes to `dist/`.

QuickClip321 is [MIT licensed](LICENSE). Third-party tools and dependencies are governed by their own licenses. See the [bundled tool notices](resources/bundled-notices/BUNDLED-TOOLS.txt).
