"""Destination regressions exercise the real worker publisher, never only a UI label."""

import copy
import json
import queue
import threading
from pathlib import Path
from unittest.mock import patch

import pytest
import yt_dlp

from quickclip321 import worker
from quickclip321.engine import DownloadEngine
from quickclip321.models import DownloadJob, JobStatus
from quickclip321.organizer import DEFAULT_DATABASE, resolve_destination
from quickclip321.storage import JobStore


CASES = [
    ("instrumental", {"title": "Free dark type beat instrumental"}, "mp3", "beats", False, True),
    ("song", {"title": "A regular song"}, "wav", "audio", False, True),
    ("movie", {"title": "Yıldızlararası sahne"}, "original", "film", False, True),
    ("tv", {"title": "Kurtlar Vadisi Pusu sahne"}, "original", "film", False, True),
    ("rap", {"title": "Ceza - Suspus (Official Music Video)"}, "original", "rap", False, True),
    ("unknown", {"title": "An unknown video"}, "original", "video", False, True),
    ("ambiguous", {"title": "Ceza hakkında röportaj"}, "original", "video", False, True),
    ("manual", {"title": "Free type beat"}, "mp3", "manual", True, True),
    ("disabled", {"title": "Free type beat"}, "mp3", "default", False, False),
]


@pytest.mark.parametrize("case,metadata,output_format,category,manual,enabled", CASES)
def test_worker_physically_publishes_selected_destination(
    tmp_path, case, metadata, output_format, category, manual, enabled
):
    database = copy.deepcopy(DEFAULT_DATABASE)
    for key, path in database["destinations"].items():
        relative = Path(path).relative_to(Path.home() / "Downloads")
        database["destinations"][key] = str(tmp_path / "Downloads" / relative)
    db_path = tmp_path / "organizer.json"
    db_path.write_text(json.dumps(database), encoding="utf-8")
    default, override = tmp_path / "default", tmp_path / "manual"
    job = DownloadJob(
        url="https://example.com/media",
        output_dir=str(default),
        output_format=output_format,
        organizer_database=str(db_path),
        organizer_enabled=enabled,
        manual_destination=manual,
        selected_destination=str(override) if manual else "",
    )
    expected = override if manual else default if not enabled else Path(database["destinations"][category])
    assert resolve_destination(metadata, job.to_dict(), database).destination == expected
    original_ydl = yt_dlp.YoutubeDL

    class FixtureYDL(original_ydl):
        def __init__(self, options):
            self.callbacks = {}
            self.options = options
            super().__init__(options)

        def add_post_processor(self, processor, when="post_process"):
            self.callbacks[when] = processor

        def extract_info(self, url, download):
            assert download
            path = Path(self.options["paths"]["home"]) / (
                "fixture." + ("mp3" if output_format == "mp3" else "wav" if output_format == "wav" else "mkv")
            )
            data = {**metadata, "id": case, "filepath": str(path)}
            self.callbacks["before_dl"].run(data)
            path.write_bytes(b"real destination fixture bytes" * 100)
            self.callbacks["after_move"].run(data)
            return data

    events = queue.Queue()
    with (
        patch.object(yt_dlp, "YoutubeDL", FixtureYDL),
        patch.object(worker, "_verify_media"),
        patch.object(worker, "_find_binary", return_value="C:/fixture/ffmpeg.exe"),
    ):
        worker.execute_job(job.to_dict(), events, threading.Event())
    emitted = list(events.queue)
    assert emitted[-1]["type"] == "completed", emitted
    result = Path(emitted[-1]["path"])
    assert result.parent == expected
    assert result.read_bytes() == b"real destination fixture bytes" * 100
    assert emitted[-1]["destination"] == str(expected)
    store = JobStore(tmp_path / "jobs.sqlite3")
    engine = DownloadEngine(store)
    for event in emitted:
        engine._apply_event(job, event)
    saved = store.get(job.id)
    assert saved.status == JobStatus.COMPLETED
    assert saved.destination == str(expected)
    assert saved.result_paths == [str(result)]
    assert any(event.get("type") == "destination" and event.get("destination") == str(expected) for event in emitted)
    assert not list(default.glob("fixture.*")) if expected != default else result.exists()


def test_custom_aliases_and_tv_metadata():
    database = copy.deepcopy(DEFAULT_DATABASE)
    database["tv"] = [{"name": "Personal Show", "aliases": ["Özel Dizi"], "ambiguous": False}]
    payload = DownloadJob(url="https://example.com", output_dir="C:/fallback").to_dict()
    choice = resolve_destination({"title": "Ozel Dizi sahne"}, payload, database)
    assert choice.category == "film"
    assert resolve_destination({"title": "Episode 2", "series": "Personal Show"}, payload, database).category == "film"
