"""Print local engine/runtime versions; --url performs one metadata-only extraction."""

import argparse
import json
import sys
from yt_dlp import YoutubeDL
from yt_dlp.version import __version__
from quickclip321.playlist import safety_options, single_video_url
from quickclip321.worker import _find_binary

parser = argparse.ArgumentParser()
parser.add_argument("--url")
args = parser.parse_args()
result = {"python": sys.version, "yt_dlp": __version__, "binaries": {}}
options = {
    "quiet": True,
    "socket_timeout": 12,
    "retries": 0,
    "extractor_retries": 0,
    "skip_download": True,
    **safety_options(args.url or "", "single"),
    "js_runtimes": {},
}
for name in ("ffmpeg", "ffprobe", "node", "deno"):
    binary = _find_binary(name)
    result["binaries"][name] = binary
    if binary and name in ("node", "deno"):
        options["js_runtimes"][name] = {"path": binary}
if args.url:
    try:
        with YoutubeDL(options) as ydl:
            info = ydl.extract_info(single_video_url(args.url), download=False)
        result["metadata"] = {
            "id": info.get("id"),
            "title": info.get("title"),
            "format_count": len(info.get("formats") or []),
        }
    except Exception as exc:
        result["error"] = str(exc)
print(json.dumps(result, indent=2, ensure_ascii=False))
raise SystemExit(int('error' in result))
