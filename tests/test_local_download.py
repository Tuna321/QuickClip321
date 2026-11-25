"""End-to-end yt-dlp/FFmpeg check using a local synthetic HTTP fixture only."""

import functools
import http.server
import subprocess
import threading
from pathlib import Path

import pytest

from quickclip321.engine import DownloadEngine
from quickclip321.models import DownloadJob, JobStatus
from quickclip321.storage import JobStore
from quickclip321.worker import _find_binary, _probe_media
from test_engine import wait_for


def test_real_engine_local_fixture(tmp_path):
    ffmpeg, ffprobe = _find_binary("ffmpeg"), _find_binary("ffprobe")
    if not ffmpeg or not ffprobe:
        pytest.skip("Requires local FFmpeg tools")
    origin = tmp_path / "origin"
    origin.mkdir()
    fixture = origin / "fixture.mkv"
    subprocess.run(
        [
            ffmpeg,
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=purple:s=160x90:r=24:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:v",
            "mpeg4",
            "-c:a",
            "pcm_s16le",
            "-shortest",
            str(fixture),
        ],
        check=True,
        capture_output=True,
    )
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(origin))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    store = JobStore(tmp_path / "jobs.sqlite3")
    engine = DownloadEngine(store, max_retries=1)
    job = DownloadJob(
        url=f"http://localhost:{server.server_port}/fixture.mkv",
        output_dir=str(tmp_path / "output"),
        organizer_enabled=False,
    )
    try:
        engine.start()
        engine.submit(job)
        result = wait_for(store, job.id, {JobStatus.COMPLETED, JobStatus.FAILED}, timeout=20)
        assert result.status == JobStatus.COMPLETED, result.error_detail
        output = Path(result.result_path)
        assert output.suffix == ".mp4"
        info = _probe_media(output, ffprobe)
        assert {stream["codec_name"] for stream in info["streams"]} == {"h264", "aac"}
        assert fixture.exists()
    finally:
        engine.stop()
        server.shutdown()
        server.server_close()
        server_thread.join(timeout=2)
