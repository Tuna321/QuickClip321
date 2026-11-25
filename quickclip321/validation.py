from __future__ import annotations

import ipaddress
import math
import re
from urllib.parse import urlparse


_HOST_RE = re.compile(r"^(?=.{1,253}$)(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,63}$")


def validate_clip_range(start: float | None, end: float | None) -> tuple[float | None, float | None]:
    """Normalize a requested clip; values are seconds, with an open end allowed."""
    try:
        start = None if start is None or start == "" else float(start)
        end = None if end is None or end == "" else float(end)
    except (TypeError, ValueError) as exc:
        raise ValueError("Clip time must be a number.") from exc
    if any(not math.isfinite(value) or value < 0 for value in (start, end) if value is not None):
        raise ValueError("Clip time must be zero or greater.")
    if end is not None and end <= (start or 0):
        raise ValueError("Clip end must be after clip start.")
    return start, end


def validate_media_url(raw: str) -> tuple[bool, str, str]:
    """Return (valid, normalized_url, friendly_error)."""
    value = (raw or "").strip()
    if not value:
        return False, "", "Paste a video or audio URL first."
    if len(value) > 4096:
        return False, "", "This URL is unusually long."
    if any(char.isspace() for char in value):
        return False, "", "This does not look like a URL; it must not contain spaces."
    if "://" not in value:
        if value.lower().startswith("www."):
            value = "https://" + value
        else:
            return False, "", "Enter a media URL starting with http:// or https://."

    try:
        parsed = urlparse(value)
        host = (parsed.hostname or "").rstrip(".")
        if parsed.scheme.lower() not in {"http", "https"}:
            return False, "", "Only http and https URLs are supported."
        if not host:
            return False, "", "The URL has no valid host."
        try:
            ipaddress.ip_address(host)
            return False, "", "Direct IP addresses are not accepted for safety."
        except ValueError:
            pass
        if host != "localhost" and not _HOST_RE.match(host):
            return False, "", "The site address does not look valid."
        if parsed.username or parsed.password:
            return False, "", "URLs containing user credentials are not accepted."
    except ValueError:
        return False, "", "Could not read the URL. Copy it and try again."

    return True, value, ""
