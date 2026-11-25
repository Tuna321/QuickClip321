"""Install the pinned Windows media tools on demand into the app data directory."""

from __future__ import annotations

import hashlib
import platform
import shutil
import time
from pathlib import Path
from urllib.request import Request, urlopen
from zipfile import ZipFile

from .instance import SingleInstance
from .settings import app_data_dir

BUILD = "ffmpeg-9.1-essentials_build"
URL = f"https://github.com/GyanD/codexffmpeg/releases/download/9.1/{BUILD}.zip"
SHA256 = "b9bbbb3acece000cbeec994f856649354e52c773807841e78908374b24d658fd"
ARCHIVE_SIZE = 119_467_219
TOOLS = ("ffmpeg.exe", "ffprobe.exe")


def managed_tools() -> tuple[str, str] | None:
    root = app_data_dir() / "media-tools" / BUILD
    paths = tuple(root / name for name in TOOLS)
    if all(path.is_file() for path in paths):
        return str(paths[0]), str(paths[1])
    return None


def install_media_tools(notify, cancelled) -> tuple[str, str]:
    if platform.system() != "Windows" or platform.machine().lower() not in {"amd64", "x86_64"}:
        raise RuntimeError("Automatic setup requires Windows x64. Install FFmpeg and ffprobe on PATH.")
    parent = app_data_dir() / "media-tools"
    parent.mkdir(parents=True, exist_ok=True)
    lock = None
    deadline = time.monotonic() + 300
    while lock is None:
        if cancelled():
            raise InterruptedError
        if installed := managed_tools():
            return installed
        try:
            lock = SingleInstance(parent / "install.lock")
        except RuntimeError:
            if time.monotonic() >= deadline:
                raise TimeoutError("Timed out waiting for another job to install FFmpeg.")
            notify("Waiting for FFmpeg setup…")
            time.sleep(0.5)

    # Only the lock owner creates or removes this app-owned staging directory.
    staging = parent / ".installing"
    try:
        if installed := managed_tools():
            return installed
        shutil.rmtree(staging, ignore_errors=True)
        staging.mkdir()
        archive = staging / "ffmpeg.zip"
        _download_archive(archive, notify, cancelled)
        notify("Installing FFmpeg…")
        extracted = staging / BUILD
        extracted.mkdir()
        with ZipFile(archive) as bundle:
            for name in (*TOOLS, "LICENSE"):
                if cancelled():
                    raise InterruptedError
                member = f"{BUILD}/{'bin/' if name in TOOLS else ''}{name}"
                with bundle.open(member) as source, (extracted / name).open("wb") as dest:
                    shutil.copyfileobj(source, dest)
        target = parent / BUILD
        if target.is_dir():
            shutil.rmtree(target)
        extracted.rename(target)
        return tuple(str(target / name) for name in TOOLS)
    finally:
        shutil.rmtree(staging, ignore_errors=True)
        lock.close()


def _download_archive(path: Path, notify, cancelled) -> None:
    received = 0
    digest = hashlib.sha256()
    notify("Downloading FFmpeg for first use…")
    request = Request(URL, headers={"User-Agent": "QuickClip321"})
    with urlopen(request, timeout=20) as response, path.open("wb") as output:
        while chunk := response.read(256 * 1024):
            if cancelled():
                raise InterruptedError
            received += len(chunk)
            if received > ARCHIVE_SIZE:
                raise ValueError("Unexpected FFmpeg archive size.")
            digest.update(chunk)
            output.write(chunk)
            notify(f"Downloading FFmpeg for first use… {received * 100 // ARCHIVE_SIZE}%")
    if received != ARCHIVE_SIZE or digest.hexdigest() != SHA256:
        raise ValueError("FFmpeg archive verification failed.")
