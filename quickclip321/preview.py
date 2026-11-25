"""Optional, bounded metadata process. Download submission never waits for preview."""

from urllib.request import Request, urlopen

from .playlist import is_playlist_url, safety_options, single_video_url
from .worker import _find_binary


def fetch_preview(url, events):
    try:
        from yt_dlp import YoutubeDL

        options = {
            "quiet": True,
            "no_warnings": True,
            "skip_download": True,
            "extract_flat": "in_playlist",
            "socket_timeout": 8,
            "retries": 0,
            "extractor_retries": 0,
            **safety_options(url, "single"),
            "js_runtimes": {},
        }
        for name in ("deno", "node"):
            path = _find_binary(name)
            if path:
                options["js_runtimes"][name] = {"path": path}
        if is_playlist_url(url):
            options.update({"noplaylist": False, "playlist_items": "1", "extract_flat": True})
        with YoutubeDL(options) as ydl:
            info = ydl.extract_info(url if is_playlist_url(url) else single_video_url(url), download=False) or {}
        playlist = info.get("_type") in {"playlist", "multi_video"}
        result = {
            "title": info.get("title", ""),
            "playlist": playlist,
            "count": info.get("playlist_count") or info.get("n_entries"),
            "duration": info.get("duration"),
            "channel": info.get("channel") or info.get("uploader"),
            "qualities": [str(height) for height in sorted({
                int(fmt["height"])
                for fmt in info.get("formats") or []
                if isinstance(fmt, dict) and isinstance(fmt.get("height"), (int, float))
                and fmt["height"] > 0 and fmt.get("vcodec") != "none"
            }, reverse=True)],
            "image": None,
            "metadata": {
                key: info.get(key)
                for key in (
                    "title",
                    "artist",
                    "album_artist",
                    "channel",
                    "uploader",
                    "categories",
                    "series",
                    "tv_show",
                    "movie",
                    "movie_title",
                    "vcodec",
                    "track",
                    "genre",
                )
            },
        }
        thumbnail = info.get("thumbnail")
        if thumbnail and thumbnail.startswith("https://"):
            try:
                with urlopen(Request(thumbnail, headers={"User-Agent": "QuickClip321/2.0"}), timeout=5) as response:
                    data = response.read(2_000_001)
                    if len(data) <= 2_000_000:
                        result["image"] = data
            except Exception:
                pass
        events.put(result)
    except Exception:
        events.put({"error": True})
