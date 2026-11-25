from __future__ import annotations

import re
from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit


_PRIVATE_KEYS = {"token", "auth", "key", "sig", "signature", "password", "pass", "cookie", "session", "secret", "authorization", "api_key", "access_token"}


def redact_sensitive(raw: object, secrets: tuple[str, ...] = ()) -> str:
    """Remove credentials from yt-dlp diagnostics before queue, persistence or logging."""
    value = str(raw or "")
    for secret in secrets:
        if secret:
            value = value.replace(secret, "[redacted]")

    def redact_url(match: re.Match[str]) -> str:
        token = match.group(0)
        try:
            parsed = urlsplit(token)
            netloc = parsed.hostname or ""
            if parsed.port:
                netloc += f":{parsed.port}"
            query = urlencode(
                [(key, "[redacted]" if key.lower() in _PRIVATE_KEYS or any(piece in key.lower() for piece in ("token", "secret", "cookie", "auth", "sig", "key", "session")) else val)
                 for key, val in parse_qsl(parsed.query, keep_blank_values=True)]
            )
            return urlunsplit((parsed.scheme, netloc, parsed.path, query, ""))
        except ValueError:
            return "[redacted URL]"

    value = re.sub(r"https?://[^\s\]\[<>\"']+", redact_url, value)
    value = re.sub(r"(?im)\b(authorization|cookie|set-cookie)\s*[:=]\s*[^\r\n]+", r"\1=[redacted]", value)
    return value


def classify_error(raw: str) -> tuple[str, str, bool]:
    text = (raw or "").strip()
    low = text.lower()
    rules = [
        (
            ("ffmpeg setup failed:",),
            "FFMPEG_SETUP_FAILED",
            "FFmpeg setup failed. Check your connection and retry from History.",
            False,
        ),
        (("drm",), "DRM_RESTRICTED", "DRM protected content is not supported.", False),
        (
            ("requested format is not available",),
            "FORMAT_UNAVAILABLE",
            "The selected quality is unavailable; try another quality or Fast Download.",
            False,
        ),
        (
            ("javascript runtime", "challenge solving", "n challenge", "signature extraction"),
            "YOUTUBE_RUNTIME",
            "YouTube JavaScript support is required. Check the engine and Node/Deno setup.",
            False,
        ),
        (
            ("http error 403",),
            "ACCESS_DENIED",
            "The site denied access. Check the engine version and your access to this content.",
            False,
        ),
        (
            ("unsupported url", "no suitable extractor"),
            "UNSUPPORTED_URL",
            "This site or URL type is not currently supported.",
            False,
        ),
        (
            ("private video", "login required", "sign in"),
            "LOGIN_REQUIRED",
            "This content is private or requires sign-in.",
            False,
        ),
        (
            ("video unavailable", "not available", "removed"),
            "MEDIA_UNAVAILABLE",
            "The content was removed, hidden or is unavailable in your region.",
            False,
        ),
        (("copyright",), "COPYRIGHT_BLOCK", "The content is unavailable due to copyright restrictions.", False),
        (("disk full", "no space left"), "DISK_FULL", "There is not enough free disk space.", False),
        (("permission denied", "access is denied"), "PERMISSION_DENIED", "Cannot write to the download folder.", False),
        (
            ("ffmpeg", "ffprobe", "converted mp4"),
            "FFMPEG_ERROR",
            "Could not process the media. Source files were retained; check the log.",
            False,
        ),
        (
            ("timed out", "timeout", "connection reset", "temporary failure"),
            "NETWORK_TIMEOUT",
            "The connection was interrupted or timed out.",
            True,
        ),
        (
            ("http error 429", "too many requests"),
            "RATE_LIMIT",
            "The site detected too many requests. The download will retry shortly.",
            True,
        ),
        (("http error 5", "server error"), "SERVER_ERROR", "The site returned a temporary server error.", True),
    ]
    for needles, code, friendly, retryable in rules:
        if any(needle in low for needle in needles):
            return code, friendly, retryable
    return "DOWNLOAD_ERROR", "The download could not be completed. Details were saved to the log.", True
