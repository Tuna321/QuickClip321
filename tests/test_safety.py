import copy
import json

import pytest

from quickclip321.errors import classify_error
from quickclip321.models import DownloadJob
from quickclip321.organizer import DEFAULT_DATABASE, classify, publish
from quickclip321.playlist import is_playlist_url, safety_options, single_video_url
from quickclip321.playlist import duplicate_key
from quickclip321.settings import SettingsStore
from quickclip321.worker import _safe_result_path


@pytest.mark.parametrize(
    "url",
    [
        "https://youtube.com/playlist?list=PL123",
        "https://youtube.com/watch?v=123&list=PL123",
        "https://soundcloud.com/user/sets/album",
    ],
)
def test_playlist_detection(url):
    assert is_playlist_url(url)


def test_single_safety_on_unknown_playlist():
    assert DownloadJob(url="https://example.com").playlist_mode == "single"
    assert safety_options("https://example.com/unknown", "single") == {"noplaylist": True, "playlist_items": "1"}
    assert safety_options("https://example.com/unknown", "all") == {"noplaylist": False}
    assert single_video_url("https://youtube.com/watch?v=abc&list=PL1&index=3") == "https://youtube.com/watch?v=abc"
    assert not is_playlist_url("https://youtube.com/watch?v=abc")


def test_playlist_duplicates_with_different_watch_urls():
    a = DownloadJob(url="https://youtube.com/watch?v=abc&list=PL123&index=1", playlist_mode="all")
    b = DownloadJob(url="https://www.youtube.com/playlist?list=PL123", playlist_mode="all")
    assert duplicate_key(a) == duplicate_key(b)


@pytest.mark.parametrize("title", ["Free Type Beat", "KARAOKE track", "Enstrümantal", "instrumental mix"])
def test_beats(title, tmp_path):
    result = classify({"title": title}, "mp3", tmp_path)
    assert result.category == "beats"
    assert result.confidence > 0.8
    assert result.reason


def test_ambiguous_names_and_aliases(tmp_path):
    assert classify({"title": "Ceza hakkında haberler"}, "mp4", tmp_path).category == "video"
    assert classify({"title": "Ceza official video"}, "mp4", tmp_path).category == "video"
    assert classify({"title": "a video", "artist": "CEZA"}, "mp4", tmp_path).category == "rap"
    assert classify({"title": "Sagopa official video"}, "mp4", tmp_path).category == "rap"
    assert classify({"title": "Eminem documentary"}, "mp4", tmp_path).category == "video"


def test_movies_and_destinations(tmp_path):
    db = copy.deepcopy(DEFAULT_DATABASE)
    db["movies"] = [{"name": "Example Movie", "aliases": ["Örnek film"], "ambiguous": False}]
    db["destinations"]["film"] = str(tmp_path / "clips")
    result = classify({"title": "Ornek film sahne"}, "mp4", tmp_path, db)
    assert result.destination == tmp_path / "clips"
    assert classify({"title": "Example Movie"}, "mp4", tmp_path, db).category == "video"
    assert classify({"title": "a", "categories": ["Education"]}, "mp4", tmp_path).category == "video"
    assert classify({"title": "song"}, "mp3", tmp_path).category == "audio"
    assert classify({"title": "type beat"}, "mp3", tmp_path, enabled=False).destination == tmp_path


def test_exclusive_publication(tmp_path):
    source = tmp_path / "stage" / "song.mp3"
    source.parent.mkdir()
    source.write_bytes(b"new")
    destination = tmp_path / "music"
    destination.mkdir()
    (destination / source.name).write_bytes(b"original")
    result = publish(source, destination)
    assert result.name == "song (1).mp3"
    assert (destination / "song.mp3").read_bytes() == b"original"
    assert result.read_bytes() == b"new"


def test_output_cannot_fall_back_to_unrelated_file(tmp_path):
    (tmp_path / "unrelated.mp4").write_bytes(b"existing")
    assert _safe_result_path({}, None, tmp_path) is None


def test_settings_migrate_unsafe_defaults(tmp_path, monkeypatch):
    monkeypatch.setenv("QUICKCLIP321_DATA_DIR", str(tmp_path))
    store = SettingsStore()
    store.path.write_text(
        json.dumps(
            {"playlist_mode": "all", "open_on_clipboard_url": True, "concurrent_downloads": 500, "max_retries": 500}
        ),
        encoding="utf-8",
    )
    settings = store.load()
    assert settings.playlist_mode == "single"
    assert not settings.open_on_clipboard_url
    assert settings.concurrent_downloads == 3 and settings.max_retries == 5
    store.save(settings)
    assert store.load() == settings
    store.path.write_text("{bad", encoding="utf-8")
    assert store.load().playlist_mode == "single"


@pytest.mark.parametrize(
    "error,code,retry",
    [
        ("FFmpeg conversion failed", "FFMPEG_ERROR", False),
        ("HTTP Error 403", "ACCESS_DENIED", False),
        ("DRM protected", "DRM_RESTRICTED", False),
        ("connection reset", "NETWORK_TIMEOUT", True),
        ("HTTP Error 429", "RATE_LIMIT", True),
    ],
)
def test_error_classification(error, code, retry):
    classified = classify_error(error)
    assert classified[0] == code and classified[2] == retry


@pytest.mark.parametrize(
    "category,info,format",
    [
        ("beats", {"title": "type beat"}, "wav"),
        ("audio", {"title": "song"}, "mp3"),
        ("rap", {"title": "music", "artist": "Ceza"}, "mp4"),
        ("film", {"title": "Example movie scene"}, "mp4"),
        ("video", {"title": "learning", "categories": ["Education"]}, "mp4"),
    ],
)
def test_every_destination_handles_publication(tmp_path, category, info, format):
    db = copy.deepcopy(DEFAULT_DATABASE)
    db["movies"] = [{"name": "Example movie", "aliases": [], "ambiguous": False}]
    for key, configured in db["destinations"].items():
        db["destinations"][key] = str(tmp_path / key / configured.split("Downloads")[-1].strip("\\/"))
    result = classify(info, format, tmp_path / "fallback", db)
    assert result.category == category
    source = tmp_path / ("fixture." + format)
    source.write_bytes(b"media fixture")
    output = publish(source, result.destination)
    assert output.parent == result.destination
    assert output.read_bytes() == b"media fixture"


def test_interrupted_publish_does_not_expose_incomplete_media(tmp_path, monkeypatch):
    import quickclip321.organizer as organizer

    source = tmp_path / "source.mp4"
    source.write_bytes(b"complete source")

    def fail_copy(input_file, output_file, length):
        output_file.write(b"partial")
        raise OSError("simulated write failure")

    monkeypatch.setattr(organizer.shutil, "copyfileobj", fail_copy)
    destination = tmp_path / "destination"
    with pytest.raises(OSError):
        publish(source, destination)
    assert not list(destination.glob("*.mp4"))
    assert not list(destination.rglob("*.part"))
    assert source.read_bytes() == b"complete source"
