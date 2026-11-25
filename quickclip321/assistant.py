"""Opt-in download suggestions. Local suggestions require no network access."""

from __future__ import annotations

import json
import math
import re
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from .organizer import classify


class AssistantError(RuntimeError):
    """A safe, user-facing assistant failure without request or response secrets."""


_FORMATS = {"mp4", "mp3", "m4a", "wav", "original"}
_CATEGORIES = {"video", "audio", "unsorted"}
_WINDOWS_RESERVED = {"CON", "PRN", "AUX", "NUL", *(f"COM{n}" for n in range(1, 10)), *(f"LPT{n}" for n in range(1, 10))}


def _filename(title: object) -> str:
    value = re.sub(r'[<>:"/\\|?*\x00-\x1f]', " ", str(title or "Download"))
    value = re.sub(r"\s+", " ", value).strip(" .")[:120].rstrip(" .") or "Download"
    if value.split(".", 1)[0].upper() in _WINDOWS_RESERVED:
        value = f"_{value}"
    return value


def _public_metadata(info: dict) -> dict:
    """Whitelist fields before any remote request; never include URL or local paths."""
    nested = info.get("metadata") if isinstance(info.get("metadata"), dict) else {}
    duration = info.get("duration", nested.get("duration"))
    if not isinstance(duration, (int, float)) or not math.isfinite(duration) or duration < 0:
        duration = None
    formats = info.get("available_formats") or info.get("qualities") or []
    if not isinstance(formats, list):
        formats = []
    return {
        "title": str(info.get("title") or "")[:200],
        "duration_seconds": duration,
        "channel": str(info.get("channel") or nested.get("channel") or info.get("uploader") or nested.get("uploader") or "")[:120],
        "available_formats": [str(value)[:40] for value in formats[:20]],
    }


def suggest_local(
    metadata: dict,
    output_format: str,
    quality: str,
    fallback_dir: str,
    organizer_database: dict | None = None,
    organizer_enabled: bool = True,
) -> dict:
    """Return a transparent local suggestion without changing an existing job."""
    chosen_format = output_format if output_format in _FORMATS else "mp4"
    available = {str(height) for height in metadata.get("qualities") or []}
    chosen_quality = quality if quality == "best" or not available or quality in available else "best"
    classification = classify(
        metadata.get("metadata") or metadata,
        chosen_format,
        fallback_dir,
        database=organizer_database,
        enabled=organizer_enabled,
    )
    return {
        "format": chosen_format,
        "quality": chosen_quality,
        "filename": _filename(metadata.get("title")),
        "category": classification.category,
        "destination": str(classification.destination),
        "reason": classification.reason,
        "source": "local",
    }


_SCHEMA = {
    "type": "object",
    "properties": {
        "format": {"type": "string", "enum": sorted(_FORMATS)},
        "quality": {"type": "string"},
        "filename": {"type": "string"},
        "category": {"type": "string", "enum": sorted(_CATEGORIES)},
        "reason": {"type": "string"},
    },
    "required": ["format", "quality", "filename", "category", "reason"],
    "additionalProperties": False,
}


def _parse_remote(value: object) -> dict:
    if not isinstance(value, dict):
        raise AssistantError("The assistant returned an invalid suggestion.")
    fmt = value.get("format")
    category = value.get("category")
    if fmt not in _FORMATS or category not in _CATEGORIES:
        raise AssistantError("The assistant returned an unsupported format or category.")
    quality = str(value.get("quality") or "best")
    if quality != "best" and quality not in {"2160", "1440", "1080", "720", "480"}:
        quality = "best"
    return {
        "format": fmt,
        "quality": quality,
        "filename": _filename(value.get("filename")),
        "category": category,
        "reason": str(value.get("reason") or "AI suggestion")[:240],
        "source": "remote",
    }


def suggest_remote(provider: str, metadata: dict, model: str, api_key: str = "", timeout: int = 12) -> dict:
    """Send only the shown, whitelisted metadata after an explicit UI action.

    This function does not run automatically and never changes a download job.
    Ollama is restricted to loopback. Call it from a worker thread, not Tk's UI thread.
    """
    if not model or len(model) > 100:
        raise AssistantError("Choose an assistant model first.")
    if provider == "openai":
        if not api_key:
            raise AssistantError("Add your OpenAI API key in Settings first.")
        endpoint = "https://api.openai.com/v1/chat/completions"
    elif provider == "ollama":
        endpoint = "http://127.0.0.1:11434/api/chat"
    else:
        raise AssistantError("Choose OpenAI or local Ollama.")

    public = _public_metadata(metadata)
    prompt = (
        "Suggest a download format, quality, short filename, and broad media category. "
        "Only use the supplied metadata. Never invent access to unavailable media. "
        + json.dumps(public, ensure_ascii=False)
    )
    if provider == "openai":
        payload = {
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": "download_suggestion", "strict": True, "schema": _SCHEMA},
            },
        }
        headers = {"Authorization": f"Bearer {api_key}"}
    else:
        payload = {
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "format": _SCHEMA,
            "stream": False,
        }
        headers = {}
    request = Request(
        endpoint,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Content-Type": "application/json", **headers},
        method="POST",
    )
    try:
        with urlopen(request, timeout=timeout) as response:
            if response.status != 200:
                raise AssistantError(f"Assistant service returned HTTP {response.status}.")
            body = response.read(1_000_001)
            if len(body) > 1_000_000:
                raise AssistantError("Assistant response was too large.")
        answer = json.loads(body)
        content = answer["choices"][0]["message"]["content"] if provider == "openai" else answer["message"]["content"]
        return _parse_remote(json.loads(content))
    except HTTPError as exc:
        raise AssistantError(f"Assistant service returned HTTP {exc.code}.") from None
    except (URLError, TimeoutError):
        raise AssistantError("Assistant service could not be reached.") from None
    except (ValueError, KeyError, IndexError, TypeError):
        raise AssistantError("Assistant service returned an invalid response.") from None
