"""Offline acceptance check: real yt-dlp + FFmpeg + filesystem destinations, local synthetic media only."""

from __future__ import annotations

import copy
import functools
import html
import http.server
import json
import subprocess
import threading
import time
from pathlib import Path

from .engine import DownloadEngine
from .models import DownloadJob, JobStatus
from .organizer import DEFAULT_DATABASE
from .storage import JobStore
from .worker import _find_binary


def destination_acceptance(directory):
    root = Path(directory).resolve()
    root.mkdir(parents=True, exist_ok=True)
    origin = root / "origin"
    origin.mkdir(exist_ok=True)
    fixture = origin / "synthetic.mp4"
    subprocess.run(
        [
            _find_binary("ffmpeg"),
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=purple:s=160x90:r=24:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            str(fixture),
        ],
        check=True,
        capture_output=True,
        timeout=30,
        creationflags=subprocess.CREATE_NO_WINDOW,
    )
    database = copy.deepcopy(DEFAULT_DATABASE)
    for category, destination in database["destinations"].items():
        relative = Path(destination).relative_to(Path.home() / "Downloads")
        database["destinations"][category] = str(root / "Downloads" / relative)
    db_path = root / "organizer.json"
    db_path.write_text(json.dumps(database), encoding="utf-8")
    cases = [
        ("instrumental", "Dark instrumental type beat", "mp3", "beats", False, True),
        ("song", "Regular song", "wav", "audio", False, True),
        ("movie", "Interstellar scene", "mp4", "film", False, True),
        ("tv", "Kurtlar Vadisi Pusu sahne", "mp4", "film", False, True),
        ("rap", "Ceza - Suspus (Official Music Video)", "mp4", "rap", False, True),
        ("unknown", "An unknown video", "mp4", "video", False, True),
        ("ambiguous", "Ceza hakkında röportaj", "mp4", "video", False, True),
        ("manual", "Dark instrumental type beat", "mp3", "manual", True, True),
        ("disabled", "Dark instrumental type beat", "mp3", "default", False, False),
    ]
    for name, title, *_ in cases:
        (origin / (name + ".html")).write_text(
            "<html><head><title>"
            + html.escape(title)
            + '</title></head><body><video controls src="synthetic.mp4"></video></body></html>',
            encoding="utf-8",
        )

    class QuietHandler(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *_):
            pass

    handler = functools.partial(QuietHandler, directory=str(origin))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    store = JobStore(root / "jobs.sqlite3")
    engine = DownloadEngine(store, max_retries=1)
    results = []
    try:
        engine.start()
        for name, title, output_format, category, manual, enabled in cases:
            default = root / "default"
            override = root / "manual"
            expected = override if manual else default if not enabled else Path(database["destinations"][category])
            job = DownloadJob(
                url=f"http://localhost:{server.server_port}/{name}.html",
                output_format=output_format,
                output_dir=str(default),
                manual_destination=manual,
                selected_destination=str(override) if manual else "",
                organizer_database=str(db_path),
                organizer_enabled=enabled,
            )
            engine.submit(job)
            deadline = time.monotonic() + 40
            while time.monotonic() < deadline:
                current = store.get(job.id)
                if current.status in {JobStatus.COMPLETED, JobStatus.FAILED}:
                    break
                time.sleep(0.05)
            current = store.get(job.id)
            if current.status != JobStatus.COMPLETED:
                raise AssertionError(f"{name}: {current.status} {current.error_detail}")
            final = Path(current.result_path)
            if not final.is_file() or final.parent != expected or final.stat().st_size < 1024:
                raise AssertionError(f"{name}: actual={final}; expected={expected}")
            if current.destination != str(expected):
                raise AssertionError(f"{name}: history destination differs from published file")
            results.append(
                {
                    "case": name,
                    "path": str(final),
                    "bytes": final.stat().st_size,
                    "destination": current.destination,
                    "reason": current.classification_reason,
                    "passed": True,
                }
            )
    finally:
        engine.stop()
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)
    (root / "results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    return results
