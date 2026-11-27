# QuickClip321 — Free Windows video &amp; audio downloader

**Copy a link. Press a shortcut. Keep creating.**

QuickClip321 is a free, open-source Windows video and audio downloader that lives in your system tray. Get editing-ready MP4 or MP3 without opening a downloader window. Powered by yt-dlp and FFmpeg.

<p align="center"><a href="https://github.com/Tuna321/QuickClip321/releases/download/v3.0.0-beta.7/QuickClip321-3.0.0-beta.7-win-x64.exe"><img src="v3/assets/beta2-download-graphite.png" width="288" alt="Download for Windows x64 — portable EXE"></a></p>
<p align="center"><strong>V3 Beta 7 · 12.77 MiB EXE</strong> &nbsp; / &nbsp; <a href="https://github.com/Tuna321/QuickClip321/releases/download/v3.0.0-beta.7/QuickClip321-3.0.0-beta.7-win-x64-portable.zip">Portable ZIP (4.00 MiB)</a> &nbsp; / &nbsp; <a href="https://github.com/Tuna321/QuickClip321/releases/tag/v3.0.0-beta.7">Release notes</a></p>
<p align="center"><sub>Public beta · Signed update manifests · Windows x64 + WebView2 · First-run media tools may require an additional 170–200 MB.</sub></p>

<p align="center"><picture><img src="v3/assets/readme-hero.svg" width="100%" alt="QuickClip321 — Copy a link. Hit a shortcut. Keep creating. Actual Beta 5 interface shown."></picture></p>

<p align="center"><picture><img src="v3/assets/readme-metrics.svg" width="100%" alt="12.77 MiB portable EXE, 25,693 local catalog records, 8 customizable shortcuts, free and MIT licensed."></picture></p>
<sub>EXE size excludes separately installed media tools. Catalog count includes curated and contextual title records.</sub>

**[Product website](https://tuna321.github.io/QuickClip321/) · [Keyboard shortcuts](#01--download-without-opening-a-window) · [Smart organizer](#03--25693-records-less-file-chaos) · [Why QuickClip321?](#why-quickclip321) · [Get started](#get-started) · [FAQ](#good-to-know) · [Feedback](#help-shape-v3)**

**Release channels:** [Stable track / public Beta 7](https://github.com/Tuna321/QuickClip321/releases/tag/v3.0.0-beta.7) · [Beta 7 Nightly 1](https://github.com/Tuna321/QuickClip321/releases/tag/v3.0.0-beta.7.nightly.1). Both are prereleases. Switch between them from Advanced > Updates; installation verifies the signed release and waits for an idle queue. Beta 5 must be upgraded manually once.

## 01 / Download without opening a window

**Copy a link. Press a shortcut. Keep working.**

No need to switch to a downloader window, paste a URL or choose the same settings over and over. QuickClip321 runs in your system tray and can start a download from your keyboard while you are editing, browsing or working in another app.

| Copy a link, then press | Get |
| --- | --- |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>3</kbd> | MP3 audio |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>4</kbd> | Editor-ready MP4 |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>D</kbd> | Your last-used format |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Space</kbd> | Open / hide QuickClip321 |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>O</kbd> | Open your download folder |

<p align="center"><picture><img src="v3/assets/beta5-shortcuts.png" width="760" alt="Actual Beta 5 shortcuts workspace with configurable keyboard actions"></picture></p>

Your destination and quality are remembered. A small, native fade notification confirms the action without taking focus away from your work. All eight global and in-app shortcuts can be changed, disabled or tested in **Advanced > Shortcuts**. Global shortcuts work while QuickClip321 is running, even with its popup hidden.

## 02 / A small download. A complete workflow.

**12.77 MiB portable EXE. Just 4.00 MiB as a ZIP.**

QuickClip321 is built with Rust and uses the Windows WebView2 runtime. Download the standalone executable, save it in a permanent folder and open it. There is no large application installer or account setup.

On first launch, missing media tools are automatically downloaded, verified and configured. Live progress shows what is being installed. No manual FFmpeg setup, administrator privileges or system PATH edits.

The EXE size refers to QuickClip321 itself. First-time media-tool setup may require approximately 170-200 MB of additional downloads; WebView2 is required. Small file size is not a claim about RAM use.

## 03 / 25,693 records. Less file chaos.

**Download your media. Let QuickClip321 help sort it.**

The local organizer recognizes artists, games, movies, TV shows and music genres using curated titles, contextual matching and available media metadata. The library works locally, without sending classification requests to an AI service.

| **5,215** | **8,066** | **5,124** | **7,288** |
| :---: | :---: | :---: | :---: |
| Games | Films | TV series | Music records |

Choose broad or detailed categories, add your own title rules, or assign custom folders. A manually chosen destination takes precedence. Matching is contextual, not guaranteed for every title.

<details>
<summary><strong>See the organizer interface</strong></summary>
<br><picture><img src="v3/assets/beta5-organizer.png" width="900" alt="Actual Beta 5 organizer interface showing custom title rules and destinations"></picture>
</details>

## 04 / One simple popup. Everything you need.

**Simple by default. Advanced when you want it.**

Prefer clicking instead of keyboard shortcuts? Copy a link and click the Q icon in the system tray. With clipboard capture enabled, your link is already waiting. Choose a format, resolution and destination, then download. Keep working as transfers continue in the background.



| **Premiere Ready** | **Fast Download** | **Audio** |
| --- | --- | --- |
| Editing-friendly MP4 | Original format without extra compatibility conversion | MP3, M4A or WAV |

<p align="center"><picture><source media="(prefers-reduced-motion: reduce)" srcset="docs/advanced-still.png"><img src="v3/assets/beta5-advanced-demo.gif" width="720" alt="Actual Beta 5 switching from compact Quick View to the advanced Queue workspace"></picture></p>
<p align="center"><sub>Recorded from the published Beta 5 Windows EXE. The GIF shows the real Quick View to Advanced transition.</sub></p>

<details>
<summary><strong>Watch a real local test download complete</strong></summary>
<p align="center"><picture><source media="(prefers-reduced-motion: reduce)" srcset="v3/assets/beta5-download-complete.png"><img src="v3/assets/beta5-download-demo.gif" width="720" alt="Actual Beta 5 local fixture download, from starting to completed"></picture></p>
<sub>Authorized synthetic local video fixture; the test output folder is redacted. Output was verified as H.264/AAC. This edited loop is a functional demonstration, not an internet speed benchmark.</sub>
</details>

Switch between quick view and the advanced workspace. Customize themes, accents, popup position and always-on-top behavior. English and Turkish interface options are available.

<p align="center"><picture><img src="v3/assets/beta5-quick-light.png" width="600" alt="Actual Beta 5 quick view in light theme"></picture></p>

## 05 / Download what you need, not the whole video.

**Need a five-second clip? Try downloading just that part.**

Set clip start and end before downloading. For eligible short video clips, QuickClip321 first tries to retrieve the requested section, checks the result, and falls back to a full download and trim when necessary. Supported sources can save substantial transfer time and bandwidth.

Estimated source transfer size helps you choose quality. During eligible downloads, the preview can sample frames from the existing partial file without fetching a second video. Source support and container layout determine availability.

## More control, without clutter

| Feature | What you can do |
| --- | --- |
| Batch and playlists | Queue multiple links or supported playlists. |
| Background queue | Run up to three simultaneous jobs, hold waiting jobs, cancel or retry, and review history. |
| Notifications | Get Windows notifications when downloads complete or fail. |
| Profiles | Save your favorite format and resolution combinations. |
| Subtitles and metadata | Retrieve available subtitles and optional media metadata. |
| Local settings | Keep custom rules, preferences and history between launches. |

## Why QuickClip321?

QuickClip321 is built for **repeated downloads in a Windows creative workflow**, rather than simply adding another downloader window. It shares the underlying yt-dlp ecosystem with other graphical downloaders, but optimizes the path from copied link to usable media.

| Workflow | QuickClip321 V3 Beta 7 | Manual yt-dlp CLI | Parabolic |
| --- | --- | --- | --- |
| Start a download | Global hotkey or tray popup | Terminal command; can be scripted | Graphical downloader |
| Video/audio formats | MP4, MP3, M4A, WAV, original | Configurable via command-line options | MP4, WebM, MP3, Opus, FLAC, WAV |
| Organization | Built-in contextual catalog and custom folder rules | Output templates and user scripting | See its documentation for available options |
| Windows distribution | 12.77 MiB portable EXE; separate first-use media tools | CLI binaries and optional tools | Installer / portable options depending on release |
| Platforms | Windows x64 | Windows, macOS, Linux | Windows, macOS, Linux |

[yt-dlp documentation](https://github.com/yt-dlp/yt-dlp) · [Parabolic documentation](https://github.com/NickvisionApps/Parabolic) · [Parabolic releases](https://github.com/NickvisionApps/Parabolic/releases)

This is a workflow comparison based on documented features as of 26 November 2026, not a performance ranking. Other tools may be customized; no claim is made that they lack undocumented features. QuickClip321 remains a public beta.

## Get started

1. **[Download Beta 7 for Windows x64](https://github.com/Tuna321/QuickClip321/releases/download/v3.0.0-beta.7/QuickClip321-3.0.0-beta.7-win-x64.exe).** Save the EXE in a permanent folder and open it.
2. Let the one-time verified media-tool setup finish if required.
3. Copy a media link. Press a shortcut or click the Q icon in the notification area and hit Download.

**Requirements:** Windows x64 and Microsoft Edge WebView2. First-use media-tool setup may download approximately 170-200 MB.

> [!IMPORTANT]
> This is an unsigned public beta, not a stable release. Verify the download against the release SHA-256 checksum and keep Windows security enabled. Run on startup is enabled by default and can be disabled in Settings. Media compatibility depends on the source. Only save media you have permission to download.

## Help shape V3

QuickClip321 V3 is in public beta. Try it in your everyday workflow and tell us what feels slow, confusing or missing.

- [Join GitHub Discussions](https://github.com/Tuna321/QuickClip321/discussions) for questions and ideas.
- [Report a bug or request a feature](https://github.com/Tuna321/QuickClip321/issues/new/choose).
- Include the version, Windows version, steps to reproduce and a reviewed error message. Do not share private links, cookies or credentials.
- If it makes your workflow easier, [star the project](https://github.com/Tuna321/QuickClip321/stargazers).

## Good to know

<details>
<summary>Is QuickClip321 an alternative to browser-based video downloaders?</summary>
<br>Yes, if you prefer a local Windows desktop workflow with a tray popup and global hotkeys. It is a graphical downloader built around yt-dlp and FFmpeg, not a new media extraction engine. Source support depends on the underlying tools and the website. Use it only for media you have permission to download.
</details>

<details>
<summary>How do I verify the download?</summary>
<br>Download <a href="https://github.com/Tuna321/QuickClip321/releases/download/v3.0.0-beta.7/SHA256-EXE.txt">SHA256-EXE.txt</a> from the release. In PowerShell run <code>Get-FileHash .\QuickClip321-3.0.0-beta.7-win-x64.exe -Algorithm SHA256</code> and compare the hash. For the ZIP, use SHA256SUMS.txt. A matching hash confirms the download matches the published artifact, not that the standalone EXE is Authenticode-signed. In-app updates also verify the signed release manifest.
</details>

<details>
<summary>Does it work with every website?</summary>
<br>Media support depends on <a href="https://github.com/yt-dlp/yt-dlp">yt-dlp</a> and the source. Some websites, thumbnails, formats and resolutions may be unavailable. Only save media you are authorized to download.
</details>
<details>
<summary>Can I pause and resume downloads?</summary>
<br>Eligible single Original downloads can reuse partial data. Other formats and playlists do not yet have general active-transfer resume. Waiting jobs can be held and resumed.
</details>
<details>
<summary>Why are there WebView2 processes in Task Manager?</summary>
<br>The current V3 interface uses Microsoft's WebView2 runtime. The separate native Rust prototype is not the published app.
</details>
<details>
<summary>How do I stop startup or remove it?</summary>
<br>Turn off Run on startup in Advanced &gt; Settings, exit using the tray menu, and delete the downloaded EXE or extracted application folder. Settings, history and managed tools remain in the V3 application data folder; remove them only if you no longer need them. Downloads are stored separately.
</details>

## Developers

The V3 application uses Rust, Tauri and WebView2, with separately installed yt-dlp, FFmpeg/FFprobe and Node.js media tools. Source, lockfiles and build commands are available in [V3 development documentation](v3/README.md). See [V3 licensing and dependencies](docs/V3-THIRD-PARTY.md), [license and notices for the EXE](https://github.com/Tuna321/QuickClip321/releases/download/v3.0.0-beta.7/LICENSE-AND-NOTICES.txt) and [release notes](https://github.com/Tuna321/QuickClip321/releases/tag/v3.0.0-beta.7).

## Follow QuickClip321

[YouTube](https://www.youtube.com/channel/UCM_z8cYWo9CCQeNa3CmuE6w) · [TikTok](https://www.tiktok.com/@quickclip321) · [Instagram](https://www.instagram.com/quickclip321/)

<p align="center"><strong>Made by <a href="https://github.com/Tuna321">321</a>. Built to stay out of your way.</strong><br><a href="LICENSE">MIT License</a> | <a href="https://github.com/Tuna321/QuickClip321/issues">Community</a></p>
