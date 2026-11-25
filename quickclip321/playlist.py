from urllib.parse import parse_qs, urlparse, urlunparse, urlencode


def is_playlist_url(url):
    parsed = urlparse(url)
    return bool(parse_qs(parsed.query).get("list")) or any(
        token in parsed.path.lower() for token in ["/playlist", "/sets/", "/albums/", "/album/"]
    )


def single_video_url(url):
    parsed = urlparse(url)
    host = (parsed.hostname or "").lower()
    if host in {"youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com", "youtu.be"}:
        params = parse_qs(parsed.query)
        if params.get("v") or host == "youtu.be" or parsed.path.startswith(("/shorts/", "/live/")):
            for key in ("list", "index", "start_radio"):
                params.pop(key, None)
            return urlunparse(parsed._replace(query=urlencode(params, doseq=True)))
    return url


def safety_options(url, mode):
    # playlist_items also protects playlist-only URLs and extractor-discovered playlists.
    return {"noplaylist": mode != "all", **({"playlist_items": "1"} if mode != "all" else {})}


def duplicate_key(job):
    parsed = urlparse(job.url)
    playlist_id = parse_qs(parsed.query).get("list", [])
    url = job.url
    if job.playlist_mode == "all" and playlist_id and (parsed.hostname or "").endswith("youtube.com"):
        url = "https://youtube.com/playlist?list=" + playlist_id[0]
    elif job.playlist_mode != "all":
        url = single_video_url(url)
    return (
        url,
        job.output_format,
        job.quality,
        job.playlist_mode,
        job.output_dir,
        job.profile,
        job.manual_destination,
        job.selected_destination if job.manual_destination else "",
        job.organizer_enabled,
        job.clip_start_seconds,
        job.clip_end_seconds,
        tuple(job.subtitle_languages),
        job.subtitle_format,
        job.subtitle_auto,
        job.auth_browser,
        job.auth_cookie_file,
    )
