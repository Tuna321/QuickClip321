import queue
import subprocess
import threading
from pathlib import Path
from unittest.mock import patch

import pytest

from quickclip321 import worker
from quickclip321.models import DownloadJob


@pytest.fixture(scope="module")
def media(tmp_path_factory):
    directory = tmp_path_factory.mktemp("media")
    ffmpeg = worker._find_binary("ffmpeg")
    ffprobe = worker._find_binary("ffprobe")
    if not ffmpeg or not ffprobe:
        pytest.skip("FFmpeg fixture requires ffmpeg/ffprobe on PATH")
    source = directory / "fixture.mkv"
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
            str(source),
        ],
        check=True,
        capture_output=True,
    )
    return source, ffmpeg, ffprobe


def test_real_premiere_conversion_and_verification(media, tmp_path):
    original, ffmpeg, ffprobe = media
    source = tmp_path / original.name
    source.write_bytes(original.read_bytes())
    output = worker._make_premiere_safe_mp4(source, ffmpeg, ffprobe, lambda _: None, threading.Event(), 1, 1)
    data = worker._probe_media(output, ffprobe)
    assert {s["codec_name"] for s in data["streams"]} == {"h264", "aac"}
    assert source.exists()  # source kept until final publication
    worker._verify_media(output, ffprobe, True, lambda _: None)


def test_failed_conversion_preserves_source(media, tmp_path):
    original, ffmpeg, ffprobe = media
    source = tmp_path / original.name
    source.write_bytes(original.read_bytes())
    before = source.read_bytes()

    class BrokenProcess:
        stdout = iter(["test codec failure\n"])

        def wait(self, timeout=None):
            return 1

        def poll(self):
            return 1

    with patch.object(worker.subprocess, "Popen", return_value=BrokenProcess()):
        # Preserve a real probe of the valid input, then fake the conversion process.
        with patch.object(
            worker, "_probe_media", return_value={"streams": [{"codec_type": "video", "codec_name": "mpeg4"}]}
        ):
            with pytest.raises(RuntimeError, match="FFmpeg"):
                worker._make_premiere_safe_mp4(source, ffmpeg, ffprobe, lambda _: None, threading.Event(), 1, 1)
    assert source.read_bytes() == before
    assert not list(tmp_path.glob("*.quickclip321.mp4"))


def test_already_compatible_source_skips_conversion(media, tmp_path):
    original, ffmpeg, ffprobe = media
    source = tmp_path / "compatible.mp4"
    source.write_bytes(b"fixture")
    streams = {
        "streams": [
            {"codec_type": "video", "codec_name": "h264", "pix_fmt": "yuv420p"},
            {"codec_type": "audio", "codec_name": "aac"},
        ]
    }
    with patch.object(worker, "_probe_media", return_value=streams), patch.object(worker.subprocess, "Popen") as popen:
        assert (
            worker._make_premiere_safe_mp4(source, ffmpeg, ffprobe, lambda _: None, threading.Event(), 1, 1) == source
        )
        popen.assert_not_called()


def test_execute_job_uses_exact_postprocessed_path_and_single_guard(tmp_path):
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

        def to_console_title(self, *args):
            pass

        def add_post_processor(self, pp, when):
            self.processors[when] = pp

        def extract_info(self, url, download):
            assert download
            assert "list=" not in url
            path = Path(seen["paths"]["home"]) / "actual.mp3"
            path.write_bytes(b"audio" * 400)
            info = {"filepath": str(path), "title": "Type beat", "id": "123"}
            self.processors["before_dl"].run(info)
            self.processors["after_move"].run(info)
            return info

    job = DownloadJob(
        url="https://youtube.com/watch?v=123&list=PL1",
        output_format="mp3",
        output_dir=str(tmp_path),
        organizer_enabled=False,
    )
    events = queue.Queue()
    with (
        patch.object(yt_dlp, "YoutubeDL", FakeYDL),
        patch.object(worker, "_find_binary", return_value="C:/bin/tool.exe"),
        patch.object(worker, "_verify_media"),
    ):
        worker.execute_job(job.to_dict(), events, threading.Event())
    emitted = list(events.queue)
    assert emitted[-1]["type"] == "completed", emitted
    assert Path(emitted[-1]["path"]).name == "actual.mp3"
    assert seen["noplaylist"] and seen["playlist_items"] == "1"
    assert seen["keepvideo"] and not seen["overwrites"]
    assert "trim_file_name" not in seen  # never truncate the absolute staging path
    assert not (tmp_path / ".quickclip321" / job.id).exists()


def test_unavailable_destination_fallback(tmp_path):
    # Exclusive publisher never removes source on a failed destination create.
    from quickclip321.organizer import publish

    file = tmp_path / "existing"
    file.write_bytes(b"not a directory")
    source = tmp_path / "source.mp3"
    source.write_bytes(b"media")
    with pytest.raises(OSError):
        publish(source, file / "nested")
    assert source.read_bytes() == b"media"
