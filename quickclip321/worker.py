from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import time
from collections import deque
from pathlib import Path
from queue import Queue
from typing import Any, Callable

from .errors import classify_error, redact_sensitive
from .media_tools import install_media_tools, managed_tools
from .organizer import load_database, publish, resolve_destination
from .playlist import safety_options, single_video_url
from .validation import validate_clip_range


EventSink = Callable[[dict[str, Any]], None]


class CancelledError(Exception):
    pass


_BROWSERS = {"brave", "chrome", "chromium", "edge", "firefox", "opera", "safari", "vivaldi", "whale"}
_LANGUAGE = re.compile(r"^[a-zA-Z]{2,8}(?:[-_][a-zA-Z0-9]{2,8})*$")


def _auth_options(payload: dict[str, Any]) -> dict[str, Any]:
    browser = (payload.get("auth_browser") or "").strip().lower()
    cookie_file = (payload.get("auth_cookie_file") or "").strip()
    if browser and cookie_file:
        raise ValueError("Choose one browser session method.")
    if browser:
        if browser not in _BROWSERS:
            raise ValueError("Unsupported browser session type.")
        return {"cookiesfrombrowser": (browser,)}
    if cookie_file:
        path = Path(cookie_file).expanduser()
        if not path.is_file():
            raise ValueError("Cookie file was not found.")
        return {"cookiefile": str(path)}
    return {}


def _subtitle_options(payload: dict[str, Any]) -> dict[str, Any]:
    languages = payload.get("subtitle_languages") or []
    if isinstance(languages, str):
        languages = [part.strip() for part in languages.split(",") if part.strip()]
    if not isinstance(languages, (list, tuple)) or any(not isinstance(lang, str) or not _LANGUAGE.fullmatch(lang) for lang in languages):
        raise ValueError("Invalid subtitle language.")
    if not languages:
        return {}
    output_format = payload.get("subtitle_format") or "srt"
    if output_format not in {"srt", "vtt"}:
        raise ValueError("Subtitle format must be SRT or VTT.")
    return {
        "writesubtitles": True,
        "writeautomaticsub": bool(payload.get("subtitle_auto")),
        "subtitleslangs": list(dict.fromkeys(languages)),
        "subtitlesformat": "best",
        "convertsubtitles": output_format,
    }


def _format_notice(info: dict[str, Any], quality: str, output_format: str) -> str:
    if output_format in {"mp3", "m4a", "wav"}:
        return ""
    requested = int(quality) if quality in {"2160", "1440", "1080", "720", "480"} else 0
    actual = info.get("height")
    if requested and isinstance(actual, (int, float)) and actual < requested:
        return f"Requested {requested}p was unavailable; selected {int(actual)}p."
    if output_format == "mp4":
        formats = info.get("requested_formats") or [info]
        video = next((item for item in formats if item.get("vcodec") not in (None, "none")), {})
        audio = next((item for item in formats if item.get("acodec") not in (None, "none")), {})
        if video and not str(video.get("vcodec", "")).startswith("avc1") or audio and not str(audio.get("acodec", "")).startswith("mp4a"):
            return "H.264/AAC source format was unavailable; converting to a compatible MP4."
    return ""


def _clip_media(source: Path, ffmpeg: str, ffprobe: str, start: float | None, end: float | None, cancel_event: Any, sink: EventSink) -> Path:
    if start is None and end is None:
        return source
    metadata = _probe_media(source, ffprobe)
    duration = float((metadata.get("format") or {}).get("duration") or 0)
    if duration <= 0 or (start or 0) >= duration:
        raise ValueError("Clip start is outside the media duration.")
    if end is not None and end > duration + 0.05:
        raise ValueError("Clip end is outside the media duration.")
    base_name = f"{source.stem} [clip {start or 0:g}-{end or duration:g}]"
    target = next((source.with_name(f"{base_name}{f' ({number})' if number else ''}{source.suffix}")
                   for number in range(1000)
                   if not source.with_name(f"{base_name}{f' ({number})' if number else ''}{source.suffix}").exists()), None)
    if target is None:
        raise RuntimeError("Could not create a unique clip filename")
    temporary = target.with_name(f".{target.stem}.{os.getpid()}.part{target.suffix}")
    streams = metadata.get("streams") or []
    video = any(stream.get("codec_type") == "video" for stream in streams)
    command = [ffmpeg, "-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i", str(source)]
    if start is not None:
        command += ["-ss", f"{start:.6f}"]
    if end is not None:
        command += ["-t", f"{end - (start or 0):.6f}"]
    if video:
        command += ["-map", "0:v:0", "-map", "0:a:0?", "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"]
    else:
        command += ["-map", "0:a:0", "-vn", "-c:a", {".mp3": "libmp3lame", ".m4a": "aac", ".wav": "pcm_s16le"}.get(source.suffix.lower(), "copy")]
    command += [str(temporary)]
    sink({"type": "progress", "status": "processing", "stage": "Trimming clip", "progress": 0.95})
    process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    try:
        while process.poll() is None:
            if cancel_event.is_set():
                process.terminate()
                raise CancelledError("Cancelled during clipping")
            time.sleep(0.1)
        detail = process.communicate(timeout=5)[1]
        if process.returncode:
            raise RuntimeError(f"FFmpeg clip failed: {detail[-1500:]}")
        _verify_media(temporary, ffprobe, video, sink)
        output_duration = float((_probe_media(temporary, ffprobe).get("format") or {}).get("duration") or 0)
        expected = (end or duration) - (start or 0)
        if abs(output_duration - expected) > max(0.25, expected * 0.02):
            raise RuntimeError("FFmpeg clip duration differs from requested range")
        os.link(temporary, target)
        temporary.unlink()
        return target
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
        temporary.unlink(missing_ok=True)


def _subtitle_sidecars(source: Path, output_format: str) -> list[Path]:
    return sorted(path for path in source.parent.iterdir()
                  if path.is_file() and path.name.startswith(source.stem + ".")
                  and path.suffix.lower() == "." + output_format)


def _trim_subtitle(path: Path, start: float | None, end: float | None) -> None:
    """Shift SRT/VTT cue times to match an exact post-download media clip."""
    if start is None and end is None:
        return
    subtitle_format = path.suffix.lower()
    if subtitle_format not in {".srt", ".vtt"}:
        return
    text = path.read_text(encoding="utf-8-sig")
    blocks = re.split(r"\r?\n\s*\r?\n", text.strip())

    def seconds(value: str) -> float:
        fields = value.replace(",", ".").split(":")
        if len(fields) == 2:
            fields.insert(0, "0")
        return int(fields[0]) * 3600 + int(fields[1]) * 60 + float(fields[2])

    def stamp(value: float) -> str:
        millis = round(max(0, value) * 1000)
        hours, remainder = divmod(millis, 3_600_000)
        minutes, remainder = divmod(remainder, 60_000)
        secs, ms = divmod(remainder, 1000)
        return f"{hours:02d}:{minutes:02d}:{secs:02d}{',' if subtitle_format == '.srt' else '.'}{ms:03d}"

    cues = []
    pattern = re.compile(r"((?:\d{2}:)?\d{2}:\d{2}[.,]\d{3})\s*-->\s*((?:\d{2}:)?\d{2}:\d{2}[.,]\d{3})(.*)")
    for block in blocks:
        lines = block.splitlines()
        found = next(((index, pattern.fullmatch(line)) for index, line in enumerate(lines) if pattern.fullmatch(line)), None)
        if found is None:
            continue
        index, match = found
        begin, finish = seconds(match[1]), seconds(match[2])
        if finish <= (start or 0) or end is not None and begin >= end:
            continue
        adjusted_begin = max(begin, start or 0) - (start or 0)
        adjusted_end = min(finish, end if end is not None else finish) - (start or 0)
        if adjusted_end <= adjusted_begin:
            continue
        cue = [f"{stamp(adjusted_begin)} --> {stamp(adjusted_end)}{match[3]}", *lines[index + 1:]]
        if subtitle_format == ".srt":
            cue.insert(0, str(len(cues) + 1))
        cues.append("\n".join(cue))
    output = ("WEBVTT\n\n" if subtitle_format == ".vtt" else "") + "\n\n".join(cues) + "\n"
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(output, encoding="utf-8")
    temporary.replace(path)


def _human_bytes(value: float | int | None) -> str:
    if not value:
        return ""
    size = float(value)
    for unit in ("B/s", "KB/s", "MB/s", "GB/s"):
        if size < 1024 or unit == "GB/s":
            return f"{size:.1f} {unit}"
        size /= 1024
    return ""


def _human_eta(seconds: int | float | None) -> str:
    if seconds is None:
        return ""
    seconds = max(0, int(seconds))
    minutes, sec = divmod(seconds, 60)
    hours, minutes = divmod(minutes, 60)
    return f"{hours:d}:{minutes:02d}:{sec:02d}" if hours else f"{minutes:d}:{sec:02d}"


def _find_binary(name: str) -> str | None:
    app_dir = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent))
    candidates = [
        app_dir / "bin" / f"{name}.exe",
        app_dir / f"{name}.exe",
        Path(__file__).resolve().parents[1] / "bin" / f"{name}.exe",
    ]
    if getattr(sys, "frozen", False):
        candidates.append(Path(sys.executable).parent / "bin" / f"{name}.exe")
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    return shutil.which(name)


def _quality_selector(quality: str, output_format: str) -> str:
    if output_format in {"mp3", "m4a", "wav"}:
        return "bestaudio/best"
    heights = {"2160": 2160, "1440": 1440, "1080": 1080, "720": 720, "480": 480}
    if quality in heights:
        height = heights[quality]
        if output_format == "mp4":
            return (
                f"bestvideo*[height<={height}][vcodec^=avc1]+bestaudio[acodec^=mp4a]/"
                f"bestvideo*[height<={height}]+bestaudio/best[height<={height}]/best"
            )
        return f"bestvideo*[height<={height}]+bestaudio/best[height<={height}]/best"
    if output_format == "mp4":
        return "bestvideo*[vcodec^=avc1]+bestaudio[acodec^=mp4a]/bestvideo*+bestaudio/best"
    return "bestvideo*+bestaudio/best"


def _probe_media(path: Path, ffprobe: str) -> dict[str, Any]:
    completed = subprocess.run(
        [ffprobe, "-v", "error", "-show_entries", "format=duration", "-show_streams", "-of", "json", str(path)],
        capture_output=True,
        text=True,
        timeout=30,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )
    if completed.returncode:
        raise RuntimeError(f"ffprobe failed for {path.name}: {completed.stderr[-1000:]}")
    return json.loads(completed.stdout or "{}")


def _unique_mp4_path(source: Path) -> Path:
    desired = source.with_suffix(".mp4")
    if desired == source or not desired.exists():
        return desired
    for number in range(1, 1000):
        candidate = source.with_name(f"{source.stem} ({number}).mp4")
        if not candidate.exists():
            return candidate
    raise RuntimeError("A unique MP4 output name could not be created")


def _make_premiere_safe_mp4(
    source: Path,
    ffmpeg: str,
    ffprobe: str,
    sink: EventSink,
    cancel_event: Any,
    index: int,
    total_files: int,
) -> Path:
    metadata = _probe_media(source, ffprobe)
    streams = metadata.get("streams") or []
    video = next((stream for stream in streams if stream.get("codec_type") == "video"), None)
    audio = next((stream for stream in streams if stream.get("codec_type") == "audio"), None)
    if not video:
        raise RuntimeError(f"No video stream found in {source.name}")

    target = _unique_mp4_path(source)
    already_safe = (
        source.suffix.lower() == ".mp4"
        and video.get("codec_name") == "h264"
        and video.get("pix_fmt") == "yuv420p"
        and (audio is None or audio.get("codec_name") == "aac")
    )
    if already_safe:
        return source

    temporary = source.with_name(f".{source.stem}.{os.getpid()}.quickclip321.mp4")
    duration = float((metadata.get("format") or {}).get("duration") or 0)
    stage = f"Converting to H.264 + AAC ({index}/{total_files})" if total_files > 1 else "Converting to H.264 + AAC"
    command = [
        ffmpeg,
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        str(source),
        "-map",
        "0:v:0",
        "-map",
        "0:a:0?",
        "-map_metadata",
        "0",
        "-c:v",
        "libx264",
        "-preset",
        "fast",
        "-crf",
        "18",
        "-pix_fmt",
        "yuv420p",
        "-fps_mode",
        "cfr",
        "-c:a",
        "aac",
        "-profile:a",
        "aac_low",
        "-b:a",
        "192k",
        "-ar",
        "48000",
        "-movflags",
        "+faststart",
        "-avoid_negative_ts",
        "make_zero",
        "-progress",
        "pipe:1",
        "-nostats",
        str(temporary),
    ]
    # Compatible elementary streams can be remuxed individually without lossy re-encoding.
    if video.get("codec_name") == "h264" and video.get("pix_fmt") == "yuv420p":
        start = command.index("-c:v")
        end = command.index("-c:a")
        command[start:end] = ["-c:v", "copy"]
    if audio is None or audio.get("codec_name") == "aac":
        start = command.index("-c:a")
        end = command.index("-movflags")
        command[start:end] = ["-c:a", "copy"]
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )
    try:
        assert process.stdout is not None
        error_tail: deque[str] = deque(maxlen=40)
        for line in process.stdout:
            if cancel_event.is_set():
                process.terminate()
                raise CancelledError("Cancelled during conversion")
            key, _, value = line.strip().partition("=")
            if not key.startswith("out_time") and key not in {
                "progress",
                "frame",
                "fps",
                "bitrate",
                "total_size",
                "speed",
                "dup_frames",
                "drop_frames",
                "stream_0_0_q",
            }:
                error_tail.append(line.strip())
            if key in {"out_time_us", "out_time_ms"} and duration > 0:
                try:
                    file_fraction = min(1.0, int(value) / 1_000_000 / duration)
                    overall = ((index - 1) + file_fraction) / max(1, total_files)
                    sink(
                        {"type": "progress", "status": "processing", "stage": stage, "progress": 0.88 + overall * 0.09}
                    )
                except ValueError:
                    pass
        return_code = process.wait()
        if return_code:
            raise RuntimeError(f"FFmpeg H.264 conversion failed: {' | '.join(error_tail)[-3000:]}")
        verified = _probe_media(temporary, ffprobe)
        verified_streams = verified.get("streams") or []
        verified_video = next((stream for stream in verified_streams if stream.get("codec_type") == "video"), None)
        verified_audio = next((stream for stream in verified_streams if stream.get("codec_type") == "audio"), None)
        if (
            not verified_video
            or verified_video.get("codec_name") != "h264"
            or verified_video.get("pix_fmt") != "yuv420p"
        ):
            raise RuntimeError("Converted MP4 did not pass the H.264/yuv420p verification")
        if verified_audio and verified_audio.get("codec_name") != "aac":
            raise RuntimeError("Converted MP4 did not pass the AAC verification")
        if target == source:
            os.replace(temporary, source)
            return source
        os.replace(temporary, target)
        return target  # keep source until verified output has been published
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            pass


def _safe_result_path(info: dict[str, Any], prepared: str | None, output_dir: Path) -> Path | None:
    candidates: list[Path] = []
    if prepared:
        candidates.append(Path(prepared))
    for item in info.get("requested_downloads") or []:
        if item.get("filepath"):
            candidates.append(Path(item["filepath"]))
    if info.get("filepath"):
        candidates.append(Path(info["filepath"]))
    existing = [path for path in candidates if path.is_file()]
    if existing:
        return max(existing, key=lambda path: path.stat().st_mtime)
    return None


def _verify_media(path: Path, ffprobe: str | None, expect_video: bool, sink: EventSink) -> None:
    sink({"type": "progress", "status": "verifying", "stage": "Verifying file", "progress": 0.98})
    if not path.is_file() or path.stat().st_size < 1024:
        raise RuntimeError("Output file is missing or empty")
    if not ffprobe:
        return
    data = _probe_media(path, ffprobe)
    streams = data.get("streams") or []
    if expect_video and not any(stream.get("codec_type") == "video" for stream in streams):
        raise RuntimeError("Output has no video stream")
    if not expect_video and not any(stream.get("codec_type") == "audio" for stream in streams):
        raise RuntimeError("Output has no audio stream")
    duration = float((data.get("format") or {}).get("duration") or 0)
    if duration <= 0:
        raise RuntimeError("Output duration is invalid")


def execute_job(payload: dict[str, Any], event_queue: Queue, cancel_event: Any) -> None:
    last_progress = [0.0]
    last_phase = [None]

    def sink(event):
        if event.get("type") == "progress":
            now = time.monotonic()
            phase = (event.get("status"), event.get("stage"))
            if phase == last_phase[0] and now - last_progress[0] < 0.25:
                return
            last_progress[0] = now
            last_phase[0] = phase
        for key in ("message", "detail"):
            if key in event:
                event[key] = redact_sensitive(event[key], (payload.get("auth_cookie_file", ""),))
        event["job_id"] = payload["id"]
        event_queue.put(event)

    try:
        from yt_dlp import YoutubeDL
        from yt_dlp.postprocessor.common import PostProcessor
        from yt_dlp.version import __version__

        if not re.fullmatch(r"[0-9a-f]{32}", payload["id"]):
            raise ValueError("Invalid job identifier")

        root = Path(payload["output_dir"]).expanduser().resolve() / ".quickclip321" / payload["id"]
        work = root / "work"
        work.mkdir(parents=True, exist_ok=True)
        manifest_path = root / "published.json"
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            if not isinstance(manifest, dict):
                manifest = {}
        except (OSError, ValueError):
            manifest = {}
        ffmpeg, ffprobe = _find_binary("ffmpeg"), _find_binary("ffprobe")
        if not (ffmpeg and ffprobe) or Path(ffmpeg).parent != Path(ffprobe).parent:
            try:
                ffmpeg, ffprobe = managed_tools() or install_media_tools(
                    lambda stage: sink({"type": "progress", "status": "analyzing", "stage": stage}),
                    cancel_event.is_set,
                )
            except InterruptedError:
                raise CancelledError() from None
            except Exception as exc:
                raise RuntimeError(f"FFmpeg setup failed: {exc}") from exc
        output_format = payload["output_format"]
        clip_start, clip_end = validate_clip_range(payload.get("clip_start_seconds"), payload.get("clip_end_seconds"))
        subtitle_options = _subtitle_options(payload)
        auth_options = _auth_options(payload)
        db = load_database(payload["organizer_database"]) if payload.get("organizer_database") else None

        def destination_event(classification):
            sink(
                {
                    "type": "destination",
                    "destination": str(classification.destination),
                    "reason": classification.reason,
                    "confidence": classification.confidence,
                }
            )

        sink({"type": "log", "message": f"yt-dlp={__version__}; ffmpeg={ffmpeg}; profile={payload.get('profile')}"})
        collected = []

        class CapturePP(PostProcessor):
            def run(self, info):
                if cancel_event.is_set():
                    raise CancelledError()
                path = Path(info.get("filepath") or info.get("_filename") or "")
                if not path.is_file() or not path.resolve().is_relative_to(work.resolve()):
                    raise RuntimeError("Output outside job staging folder or missing")
                collected.append((path, dict(info)))
                last_progress[0] = 0
                sink({"type": "progress", "status": "analyzing", "stage": "Preparing download results"})
                return [], info

        class MetadataPP(PostProcessor):
            def run(self, info):
                if cancel_event.is_set():
                    raise CancelledError()
                sink({"type": "metadata", "title": info.get("title", "")})
                notice = _format_notice(info, payload.get("quality", "best"), output_format)
                if notice:
                    sink({"type": "format_notice", "message": notice})
                destination_event(resolve_destination(info, payload, db))
                last_progress[0] = 0
                sink({"type": "progress", "status": "analyzing", "stage": "Preparing download"})
                return [], info

        class Logger:
            def debug(self, message):
                if message.startswith("[debug]"):
                    sink({"type": "log", "message": message})

            def warning(self, message):
                sink({"type": "log", "message": message})

            def error(self, message):
                sink({"type": "log", "message": message})

        phase = [""]

        def hook(data):
            if cancel_event.is_set():
                raise CancelledError()
            if data.get("status") == "downloading":
                total = data.get("total_bytes") or data.get("total_bytes_estimate")
                sink(
                    {
                        "type": "progress",
                        "status": "downloading",
                        "stage": "Downloading",
                        "progress": min(0.88, (data.get("downloaded_bytes", 0) / total * 0.88)) if total else 0,
                        "speed": _human_bytes(data.get("speed")),
                        "eta": _human_eta(data.get("eta")),
                    }
                )
            elif data.get("status") == "finished":
                last_progress[0] = 0
                sink({"type": "progress", "status": "processing", "stage": "Processing media", "progress": 0.89})

        def pp_hook(data):
            if cancel_event.is_set():
                raise CancelledError()
            phase[0] = data.get("postprocessor", "FFmpeg")
            if phase[0] in {"Metadata", "Capture"}:
                return
            last_progress[0] = 0
            sink({"type": "progress", "status": "processing", "stage": f"Processing: {phase[0]}", "progress": 0.9})

        processors = []
        if output_format in {"mp3", "m4a", "wav"}:
            processors.append({"key": "FFmpegExtractAudio", "preferredcodec": output_format, "preferredquality": "192"})
        if subtitle_options:
            processors.insert(0, {"key": "FFmpegSubtitlesConvertor", "format": payload.get("subtitle_format") or "srt", "when": "before_dl"})
        mode = payload.get("playlist_mode", "single")
        url = payload["url"] if mode == "all" else single_video_url(payload["url"])
        options = {
            "format": _quality_selector(payload["quality"], output_format),
            "outtmpl": str(work / "%(title).100B [%(id)s].%(ext)s"),
            "paths": {"home": str(work), "temp": str(work)},
            **safety_options(url, mode),
            **subtitle_options,
            **auth_options,
            "continuedl": True,
            "overwrites": False,
            "keepvideo": True,
            "retries": 2,
            "fragment_retries": 2,
            "file_access_retries": 2,
            "extractor_retries": 1,
            "retry_sleep_functions": {"http": lambda n: min(8, 2**n), "fragment": lambda n: min(8, 2**n)},
            "socket_timeout": 20,
            "concurrent_fragment_downloads": max(1, min(8, payload.get("fragment_concurrency", 4))),
            "progress_hooks": [hook],
            "postprocessor_hooks": [pp_hook],
            "logger": Logger(),
            "postprocessors": processors,
            "windowsfilenames": True,
            # Bound only the title in outtmpl. trim_file_name truncates absolute paths too.
            "quiet": True,
            "ffmpeg_location": str(Path(ffmpeg).parent),
            "ignoreerrors": False,
            "js_runtimes": {},
        }
        for name in ("deno", "node"):
            binary = _find_binary(name)
            if binary:
                options["js_runtimes"][name] = {"path": binary}
        if output_format == "mp4":
            options["merge_output_format"] = "mp4"
        archive = payload.get("profile") == "Archive" or payload.get("archive_metadata", False)
        if archive:
            options.update({"writeinfojson": True, "writedescription": True})
        sink({"type": "progress", "status": "analyzing", "stage": "Analyzing URL", "progress": 0.01})
        with YoutubeDL(options) as ydl:
            ydl.add_post_processor(MetadataPP(ydl), when="before_dl")
            ydl.add_post_processor(CapturePP(ydl), when="after_move")
            result_info = ydl.extract_info(url, download=True)
        if not collected:
            raise RuntimeError("Downloaded output file could not be located")
        published = []
        # All outputs are verified before publishing; conversion failures retain every source.
        ready = []
        seen_items = set()
        for index, (source, info) in enumerate(collected, 1):
            if cancel_event.is_set():
                raise CancelledError()
            item_key = str(info.get("extractor_key", "")) + ":" + str(info.get("id") or source.name)
            if item_key in seen_items:
                continue
            seen_items.add(item_key)
            previous = manifest.get(item_key)
            if previous and isinstance(previous, str) and Path(previous).is_file():
                published.append(Path(previous))
                sink({"type": "log", "message": f"Previously published item retained: {previous}"})
                continue
            last_progress[0] = 0
            sink({"type": "progress", "status": "processing", "stage": "Checking compatibility", "progress": 0.9})
            target = (
                _make_premiere_safe_mp4(source, ffmpeg, ffprobe, sink, cancel_event, index, len(collected))
                if output_format == "mp4"
                else source
            )
            _verify_media(
                target, ffprobe, output_format not in {"mp3", "m4a", "wav"} and info.get("vcodec") != "none", sink
            )
            # _clip_media verifies its own output before exposing it. When
            # trimming is disabled it returns the already-verified input.
            target = _clip_media(target, ffmpeg, ffprobe, clip_start, clip_end, cancel_event, sink)
            classification = resolve_destination(info, payload, db)
            destination_event(classification)
            sink(
                {
                    "type": "log",
                    "message": f"classification={classification.category} confidence={classification.confidence} reason={classification.reason} destination={classification.destination}",
                }
            )
            subtitles = _subtitle_sidecars(source, payload.get("subtitle_format") or "srt") if subtitle_options else []
            if subtitle_options and not subtitles:
                sink({"type": "subtitle_notice", "message": "No subtitle was available in the selected language for this item."})
            for subtitle in subtitles:
                _trim_subtitle(subtitle, clip_start, clip_end)
            ready.append((target, classification.destination, item_key, source.stem, subtitles))
        published_subtitles = []
        for target, destination, item_key, source_stem, subtitles in ready:
            if cancel_event.is_set():
                raise CancelledError()
            try:
                final = publish(target, destination)
            except OSError:
                # Unavailable organizer destination must not discard a valid download.
                if payload.get("manual_destination") or destination == Path(payload["output_dir"]):
                    raise
                sink({"type": "log", "message": "Organizer destination unavailable; using default"})
                final = publish(target, Path(payload["output_dir"]))
            published.append(final)
            manifest[item_key] = str(final)
            temporary_manifest = manifest_path.with_suffix(".tmp")
            temporary_manifest.write_text(json.dumps(manifest), encoding="utf-8")
            temporary_manifest.replace(manifest_path)
            sink({"type": "log", "message": f"Published output: {final}"})
            for subtitle in subtitles:
                if cancel_event.is_set():
                    raise CancelledError()
                # Retain the language suffix while matching any collision-safe media name.
                renamed = subtitle.with_name(final.stem + subtitle.name[len(source_stem):])
                if renamed != subtitle:
                    if renamed.exists():
                        raise FileExistsError(f"Staged subtitle already exists: {renamed.name}")
                    subtitle.rename(renamed)
                published_subtitles.append(publish(renamed, final.parent))
            if archive:
                for suffix in (".info.json", ".description"):
                    sidecar = target.with_suffix(suffix)
                    if sidecar.exists():
                        publish(sidecar, final.parent)
        shutil.rmtree(root, ignore_errors=True)
        sink(
            {
                "type": "completed",
                "path": str(published[-1].parent if mode == "all" and len(published) > 1 else published[-1]),
                "title": (result_info or {}).get("title") or published[-1].stem,
                "destination": str(published[-1].parent),
                "paths": [str(path) for path in published],
                "subtitle_paths": [str(path) for path in published_subtitles],
            }
        )
    except CancelledError:
        sink({"type": "cancelled"})
    except Exception as exc:
        detail = str(exc) or repr(exc)
        code, message, retryable = classify_error(detail)
        sink({"type": "error", "code": code, "message": message, "detail": detail[-4000:], "retryable": retryable})
    finally:
        # The worker exits immediately after its terminal event. Flush the
        # multiprocessing queue feeder before Windows tears down the process.
        if hasattr(event_queue, "close") and hasattr(event_queue, "join_thread"):
            event_queue.close()
            event_queue.join_thread()
