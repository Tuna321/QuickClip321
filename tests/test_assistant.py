"""Security and behavior checks for opt-in download suggestions."""

import io
import json
import os

import pytest

from quickclip321 import assistant
from quickclip321.credentials import CredentialStore


def test_local_suggestion_uses_local_rules_and_available_quality(tmp_path):
    result = assistant.suggest_local(
        {"title": "CON: sample/clip", "metadata": {"title": "CON: sample/clip", "vcodec": "avc1"}, "qualities": ["720"]},
        "mp4",
        "1080",
        str(tmp_path),
        organizer_database={"destinations": {"video": str(tmp_path / "Videos")}},
    )
    assert result["source"] == "local"
    assert result["quality"] == "best"
    assert result["destination"] == str(tmp_path / "Videos")
    assert "/" not in result["filename"] and ":" not in result["filename"]


def test_remote_request_never_contains_url_cookies_or_local_path(monkeypatch):
    captured = {}

    class Response(io.BytesIO):
        status = 200

    def fake_urlopen(request, timeout):
        captured["request"] = request
        return Response(json.dumps({"choices": [{"message": {"content": json.dumps({
            "format": "mp4", "quality": "720", "filename": "Video", "category": "video", "reason": "Available quality"
        })}}]}).encode())

    monkeypatch.setattr(assistant, "urlopen", fake_urlopen)
    result = assistant.suggest_remote(
        "openai",
        {
            "title": "A video",
            "duration": 24,
            "metadata": {"channel": "Creator"},
            "qualities": ["720"],
            "url": "https://example.com/private?token=secret-url-token",
            "cookies": "secret-cookie-token",
            "output_dir": "C:/private/downloads",
        },
        "test-model",
        "sk-test-key",
    )
    body = captured["request"].data.decode()
    assert result["source"] == "remote"
    assert "A video" in body
    assert "Creator" in body
    assert "secret-url-token" not in body
    assert "secret-cookie-token" not in body
    assert "C:/private/downloads" not in body
    assert "sk-test-key" not in body


@pytest.mark.skipif(os.name != "nt", reason="Windows DPAPI is required")
def test_api_key_is_encrypted_for_current_user(tmp_path):
    store = CredentialStore(tmp_path / "key.dpapi")
    store.save("private-test-key-123")
    assert b"private-test-key-123" not in store.path.read_bytes()
    assert store.load() == "private-test-key-123"
    store.clear()
    assert store.load() == ""
