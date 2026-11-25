import queue
import subprocess
import threading
import time
from pathlib import Path
from unittest.mock import patch

import pytest

from quickclip321.engine import DownloadEngine
from quickclip321.errors import redact_sensitive
from quickclip321.models import DownloadJob, JobStatus
from quickclip321.playlist import duplicate_key
from quickclip321.storage import JobStore
from quickclip321.validation import validate_clip_range
from quickclip321 import worker


def auth_worker(payload, events, cancelled):
    if payload["auth_browser"] != "firefox":
        events.put({"type": "error", "code": "MISSING_AUTH", "message": "Browser session missing", "retryable": False})
        return
    output = Path(payload["output_dir"]) / "authorized.mp4"
    output.write_bytes(b"fixture")
    events.put({"type": "completed", "path": str(output)})


def test_new_job_fields_survive_legacy_and_roundtrip():
    legacy = DownloadJob.from_dict({"url": "https://example.com/watch"})
    assert legacy.clip_start_seconds is None and legacy.subtitle_languages == []
    job = DownloadJob.from_dict({**legacy.to_dict(), "clip_start_seconds": 1.25, "clip_end_seconds": 2.5,
                                 "subtitle_languages": ["en"], "auth_browser": "firefox", "batch_id": "group"})
    assert DownloadJob.from_dict(job.to_dict()) == job
    assert duplicate_key(job) != duplicate_key(legacy)


def test_auth_reaches_worker_without_entering_history(tmp_path):
    store = JobStore(tmp_path / "jobs.sqlite3")
    engine = DownloadEngine(store, worker=auth_worker, max_retries=1)
    engine.start()
    try:
        job = DownloadJob(url="https://example.com/watch", output_dir=str(tmp_path), auth_browser="firefox")
        assert engine.submit(job)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline and store.get(job.id).status not in {JobStatus.COMPLETED, JobStatus.FAILED}:
            time.sleep(0.05)
        result = store.get(job.id)
        assert result.status == JobStatus.COMPLETED
        assert result.auth_browser == ""
        assert b"firefox" not in (tmp_path / "jobs.sqlite3").read_bytes()
    finally:
        engine.stop()


def test_completion_event_can_arrive_after_worker_exit(tmp_path):
    output = tmp_path / "delayed.mp4"
    output.write_bytes(b"ready")

    class DelayedQueue(queue.Queue):
        def close(self):
            pass

        def join_thread(self):
            pass

    class FakeProcess:
        pid = 123

        def __init__(self, target, args):
            self.events = args[1]

        def start(self):
            threading.Timer(0.8, lambda: self.events.put({"type": "completed", "path": str(output)})).start()

        def is_alive(self):
            return False

        def join(self, timeout=None):
            pass

        def close(self):
            pass

    class FakeContext:
        Queue = DelayedQueue
        Event = threading.Event
        Process = FakeProcess

    store = JobStore(tmp_path / "jobs.sqlite3")
    job = DownloadJob(url="https://example.com/watch", output_dir=str(tmp_path))
    store.save(job)
    engine = DownloadEngine(store, context=FakeContext(), max_retries=1)
    with patch("quickclip321.engine.ProcessOwner"):
        engine._execute(job)
    assert store.get(job.id).status == JobStatus.COMPLETED


@pytest.mark.parametrize("start,end", [(-1, 2), (2, 2), (3, 2), (float("nan"), None), (0, float("inf")), ("bad", 4)])
def test_invalid_clip_ranges(start, end):
    with pytest.raises(ValueError):
        validate_clip_range(start, end)


def test_precise_ffmpeg_clip_preserves_source(tmp_path):
    ffmpeg, ffprobe = worker._find_binary("ffmpeg"), worker._find_binary("ffprobe")
    if not ffmpeg or not ffprobe:
        pytest.skip("FFmpeg fixture unavailable")
    source = tmp_path / "source.mp4"
    subprocess.run([ffmpeg, "-v", "error", "-f", "lavfi", "-i", "color=c=blue:s=160x90:r=30:d=4",
                    "-f", "lavfi", "-i", "sine=frequency=440:duration=4", "-c:v", "libx264", "-c:a", "aac",
                    "-shortest", str(source)], check=True, capture_output=True)
    result = worker._clip_media(source, ffmpeg, ffprobe, 1.25, 2.75, threading.Event(), lambda _: None)
    duration = float(worker._probe_media(result, ffprobe)["format"]["duration"])
    assert abs(duration - 1.5) < 0.12
    assert source.exists() and result != source


def test_subtitle_auth_and_format_notice(tmp_path):
    cookie_file = tmp_path / "cookies.txt"
    cookie_file.write_text("# Netscape HTTP Cookie File\n")
    assert worker._auth_options({"auth_cookie_file": str(cookie_file)}) == {"cookiefile": str(cookie_file)}
    assert worker._auth_options({"auth_browser": "Firefox"}) == {"cookiesfrombrowser": ("firefox",)}
    with pytest.raises(ValueError):
        worker._auth_options({"auth_browser": "firefox", "auth_cookie_file": str(cookie_file)})
    options = worker._subtitle_options({"subtitle_languages": ["en", "tr", "en"], "subtitle_format": "srt", "subtitle_auto": True})
    assert options["subtitleslangs"] == ["en", "tr"]
    assert options["writeautomaticsub"] and options["writesubtitles"]
    assert "720p" in worker._format_notice({"height": 720}, "1080", "mp4")


@pytest.mark.parametrize("extension,first_stamp", [("srt", "00:00:00,000"), ("vtt", "00:00:00.000")])
def test_clip_shifts_subtitle_cues(tmp_path, extension, first_stamp):
    subtitle = tmp_path / f"video.en.{extension}"
    separator = "," if extension == "srt" else "."
    header = "1\n" if extension == "srt" else "WEBVTT\n\n"
    subtitle.write_text(header + f"00:00:01{separator}000 --> 00:00:02{separator}000\nBefore\n\n"
                        + f"00:00:02{separator}500 --> 00:00:04{separator}000\nInside\n\n"
                        + f"00:00:05{separator}000 --> 00:00:06{separator}000\nAfter\n")
    worker._trim_subtitle(subtitle, 2.5, 4.5)
    result = subtitle.read_text()
    assert first_stamp in result and "Inside" in result
    assert "Before" not in result and "After" not in result


def test_sensitive_diagnostics_redacted_before_persistence(tmp_path, caplog):
    url = "https://alice:topsecret@example.com/watch?token=abc123&v=123#frag"
    text = redact_sensitive(f"fetch {url}; Cookie: secretcookie", ("C:/private/cookies.txt",))
    assert all(value not in text for value in ("alice", "topsecret", "abc123", "frag", "secretcookie"))
    class Store:
        def save(self, job):
            pass
    engine = DownloadEngine(Store())
    job = DownloadJob(url="https://example.com/watch", auth_cookie_file="C:/private/cookies.txt")
    engine._apply_event(job, {"type": "error", "code": "DOWNLOAD_ERROR", "message": url,
                              "detail": "C:/private/cookies.txt " + url, "retryable": False})
    assert all(value not in job.error_detail for value in ("abc123", "topsecret", "C:/private/cookies.txt"))
    assert "abc123" not in caplog.text


def test_worker_publishes_requested_subtitle_and_reports_fallback(tmp_path):
    import yt_dlp

    seen = {}
    class FakeYDL(yt_dlp.YoutubeDL):
        def __init__(self, options):
            seen.update(options)
            self.processors = {}
            super().__init__(options)

        def __enter__(self):
            return self

        def __exit__(self, *args):
            pass

        def add_post_processor(self, pp, when):
            self.processors.setdefault(when, []).append(pp)

        def extract_info(self, url, download):
            folder = Path(seen["paths"]["home"])
            media = folder / "video.mp4"
            media.write_bytes(b"media" * 400)
            (folder / "video.en.srt").write_text("1\n00:00:00,000 --> 00:00:01,000\nHi\n")
            info = {"filepath": str(media), "title": "Video", "id": "one", "height": 720,
                    "vcodec": "avc1.4d401f", "acodec": "mp4a.40.2"}
            for pp in self.processors["before_dl"]:
                if type(pp).__name__ == "MetadataPP":
                    pp.run(info)
            for pp in self.processors["after_move"]:
                pp.run(info)
            return info

    job = DownloadJob(url="https://example.com/watch", output_dir=str(tmp_path), quality="1080",
                      output_format="mp4", organizer_enabled=False, subtitle_languages=["en"])
    events = queue.Queue()
    with (patch.object(yt_dlp, "YoutubeDL", FakeYDL),
          patch.object(worker, "_find_binary", return_value="C:/bin/tool.exe"),
          patch.object(worker, "_make_premiere_safe_mp4", side_effect=lambda source, *args: source),
          patch.object(worker, "_verify_media")):
        worker.execute_job(job.to_dict(), events, threading.Event())
    emitted = list(events.queue)
    assert emitted[-1]["type"] == "completed", emitted
    assert [Path(path).name for path in emitted[-1]["subtitle_paths"]] == ["video.en.srt"]
    assert Path(emitted[-1]["subtitle_paths"][0]).exists()
    assert any(event.get("type") == "format_notice" and "720p" in event["message"] for event in emitted)
    assert seen["writesubtitles"] and seen["subtitleslangs"] == ["en"]
    assert any(item.get("key") == "FFmpegSubtitlesConvertor" for item in seen["postprocessors"])
