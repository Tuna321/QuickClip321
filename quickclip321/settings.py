from __future__ import annotations

import json
import os
from dataclasses import asdict, dataclass
from pathlib import Path


APP_NAME = "QuickClip321"


def app_data_dir() -> Path:
    isolated = os.environ.get("QUICKCLIP321_DATA_DIR")
    if isolated:
        return Path(isolated)
    base = os.environ.get("LOCALAPPDATA") or os.environ.get("XDG_DATA_HOME")
    if base:
        return Path(base) / APP_NAME
    return Path.home() / ".local" / "share" / APP_NAME


def default_download_dir() -> Path:
    return Path.home() / "Downloads" / "QuickClip321"


@dataclass(slots=True)
class AppSettings:
    output_dir: str = str(default_download_dir())
    default_format: str = "mp4"
    default_quality: str = "best"
    max_retries: int = 3
    stall_timeout_seconds: int = 90
    clipboard_monitor: bool = True
    open_on_clipboard_url: bool = False
    notifications: bool = True
    concurrent_downloads: int = 1
    playlist_mode: str = "single"
    smart_organizer: bool = True
    fragment_concurrency: int = 4
    partial_policy: str = "keep"
    default_profile: str = "Premiere Ready"
    startup_mode: str = "quick"
    remembered_mode: str = "quick"
    theme_mode: str = "System"
    color_theme: str = "Purple"
    popup_position: str = "Tray"
    popup_monitor: str = ""
    popup_x_ratio: float = 0.5
    popup_y_ratio: float = 0.5
    ai_provider: str = "Off"
    ai_model: str = ""
    schema_version: int = 3


class SettingsStore:
    def __init__(self) -> None:
        self.directory = app_data_dir()
        self.directory.mkdir(parents=True, exist_ok=True)
        self.path = self.directory / "settings.json"

    def load(self) -> AppSettings:
        try:
            data = json.loads(self.path.read_text(encoding="utf-8"))
            if not isinstance(data, dict):
                return AppSettings()
            allowed = AppSettings.__dataclass_fields__.keys()
            settings = AppSettings(**{key: data[key] for key in allowed if key in data})
            settings.playlist_mode = "single"  # consent is never persisted
            settings.open_on_clipboard_url = False
            settings.max_retries = max(1, min(5, int(settings.max_retries)))
            settings.concurrent_downloads = max(1, min(3, int(settings.concurrent_downloads)))
            settings.fragment_concurrency = max(1, min(8, int(settings.fragment_concurrency)))
            settings.stall_timeout_seconds = max(30, min(600, int(settings.stall_timeout_seconds)))
            settings.partial_policy = "delete" if settings.partial_policy == "delete" else "keep"
            settings.schema_version = 3
            if not isinstance(settings.output_dir, str) or not settings.output_dir.strip():
                settings.output_dir = str(default_download_dir())
            if settings.default_format not in {"mp4", "mp3", "m4a", "wav", "original"}:
                settings.default_format = "mp4"
            if settings.default_quality not in {"best", "2160", "1440", "1080", "720", "480"}:
                settings.default_quality = "best"
            if settings.startup_mode not in {"quick", "advanced", "remember"}:
                settings.startup_mode = "quick"
            if settings.remembered_mode not in {"quick", "advanced"}:
                settings.remembered_mode = "quick"
            if settings.theme_mode not in {"System", "Light", "Dark"}:
                settings.theme_mode = "System"
            if settings.color_theme not in {"Purple", "Graphite", "Ocean"}:
                settings.color_theme = "Purple"
            if settings.popup_position not in {"Tray", "Corner", "Custom"}:
                settings.popup_position = "Tray"
            if settings.ai_provider not in {"Off", "openai", "ollama"}:
                settings.ai_provider = "Off"
            if not isinstance(settings.ai_model, str):
                settings.ai_model = ""
            if not isinstance(settings.popup_monitor, str):
                settings.popup_monitor = ""
            for name in ("popup_x_ratio", "popup_y_ratio"):
                try:
                    setattr(settings, name, max(0.0, min(1.0, float(getattr(settings, name)))))
                except (TypeError, ValueError):
                    setattr(settings, name, 0.5)
            return settings
        except (OSError, ValueError, TypeError):
            return AppSettings()

    def save(self, settings: AppSettings) -> None:
        temp = self.path.with_suffix(".tmp")
        temp.write_text(json.dumps(asdict(settings), indent=2, ensure_ascii=False), encoding="utf-8")
        temp.replace(self.path)
