"""Regression coverage for link entry, authorization and safe cleanup."""
from unittest.mock import Mock

import pytest

from quickclip321.engine import DownloadEngine
from quickclip321.models import DownloadJob
from quickclip321.playlist import duplicate_key, is_playlist_url
from quickclip321.storage import JobStore
from quickclip321.validation import validate_media_url


@pytest.mark.parametrize("url", [
    "https://[", "https://[broken", "https://example.com:abc/video",
    "https://example.com:65536/video", "https://example.com:0/video",
])
def test_incomplete_or_invalid_url_is_safe(url):
    assert not validate_media_url(url)[0]
    assert not is_playlist_url(url)


def test_lookalike_host_is_not_a_youtube_playlist():
    trusted = DownloadJob(url="https://youtube.com/playlist?list=PL123", playlist_mode="all")
    lookalike = DownloadJob(url="https://notyoutube.com/playlist?list=PL123", playlist_mode="all")
    assert duplicate_key(trusted) != duplicate_key(lookalike)


def test_authorized_duplicate_uses_in_memory_auth(tmp_path):
    store = JobStore(tmp_path / "jobs.db")
    engine = DownloadEngine(store)
    first = DownloadJob(url="https://example.com/media", auth_browser="firefox")
    second = DownloadJob(url=first.url, auth_browser="firefox")
    assert engine.submit(first)
    assert not engine.submit(second)
    assert store.get(first.id).auth_browser == ""


def test_cleanup_never_follows_redirected_job_directory(tmp_path, monkeypatch):
    engine = DownloadEngine(Mock())
    job = DownloadJob(url="https://example.com/media", output_dir=str(tmp_path), partial_policy="delete")
    external = tmp_path / "unrelated"
    work = external / "work"
    work.mkdir(parents=True)
    (work / "keep.txt").write_text("do not delete", encoding="utf-8")
    parent = tmp_path / ".quickclip321"
    parent.mkdir()
    try:
        (parent / job.id).symlink_to(external, target_is_directory=True)
    except (OSError, NotImplementedError):
        pytest.skip("Creating directory symlinks requires OS support")
    engine._cleanup_partial(job)
    assert (work / "keep.txt").exists()
