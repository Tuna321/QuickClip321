"""Privacy-safe text for user-initiated diagnostic sharing."""

from __future__ import annotations

import re

from .errors import redact_sensitive


def redact_support_text(raw: str) -> str:
    text = redact_sensitive(raw)
    text = re.sub(r"https?://[^\s<>'\"]+", "[URL removed]", text)
    text = re.sub(r"(?im)\b[a-z]:[\\/][^\r\n]+", "[local path removed]", text)
    text = re.sub(r"(?m)\\\\[^\r\n]+", "[network path removed]", text)
    return text
