# Licensing for the Windows release

QuickClip321's own source code is under the [MIT License](../LICENSE).
Bundled third-party components retain their own terms. The EXE contains their
license texts in its `THIRD_PARTY_LICENSES` directory. The same notices are
available as a separate download on the release page.

| Component | License and source |
| --- | --- |
| pystray 0.19.5 | LGPL-3.0. [Exact upstream revision](https://github.com/moses-palmer/pystray/tree/1907f8681d6d421517c63d94f425f9cdd74d0034). Unmodified corresponding source is attached to the release. |
| certifi 2026.10.23 | MPL-2.0. [Upstream distribution](https://pypi.org/project/certifi/2026.10.23/). Corresponding source is attached to the release. |
| Python 3.12.14 and Tcl/Tk | The Python runtime notice includes the PSF and Tcl/Tk terms. [Python sources](https://www.python.org/downloads/release/python-31214/). |
| OpenSSL 3.5.8 | Apache-2.0. [Upstream source](https://github.com/openssl/openssl/tree/openssl-3.5.8). |
| Node.js 24.20.0 | MIT and third-party terms listed in `NODE-LICENSE.txt`. [Upstream source](https://nodejs.org/dist/v24.20.0/). |
| Other Python dependencies | Individual project notices are under `THIRD_PARTY_LICENSES/python`. |

The public EXE does not redistribute FFmpeg, ffprobe, or Mutagen. QuickClip321
invokes the user's FFmpeg and ffprobe executables as separate processes.

## Rebuild with a modified pystray

Download the source archive for this QuickClip321 tag and the accompanying
pystray source archive from [GitHub Releases](https://github.com/Tuna321/QuickClip321/releases).
Set up the Python environment from `requirements-test-lock.txt` first, then
replace the installed library with your modified source and rebuild:

```powershell
.\.venv\Scripts\python.exe -m pip install --force-reinstall --no-deps 'C:\path\to\modified-pystray'
.\build_portable.ps1 -PublicRelease
```

Put `node.exe` in the source tree's `bin` folder before building. Supply FFmpeg
and ffprobe on PATH for the build's media tests. The resulting public EXE is in
`dist`. You may modify or replace pystray and reverse engineer the combined
work for debugging those modifications under the LGPL.
