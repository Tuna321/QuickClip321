from __future__ import annotations

from dataclasses import asdict, dataclass, field
from enum import StrEnum
from typing import Any
import time
import uuid


class JobStatus(StrEnum):
    QUEUED = "queued"
    ANALYZING = "analyzing"
    DOWNLOADING = "downloading"
    PROCESSING = "processing"
    VERIFYING = "verifying"
    RETRYING = "retrying"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"
    PAUSED = "paused"


ACTIVE_STATUSES = {
    JobStatus.ANALYZING,
    JobStatus.DOWNLOADING,
    JobStatus.PROCESSING,
    JobStatus.VERIFYING,
    JobStatus.RETRYING,
}


@dataclass(slots=True)
class DownloadJob:
    url: str
    output_format: str = "mp4"
    quality: str = "best"
    output_dir: str = ""
    playlist_mode: str = "single"
    profile: str = "Premiere Ready"
    organizer_enabled: bool = True
    manual_destination: bool = False
    selected_destination: str = ""
    destination: str = ""
    classification_reason: str = ""
    classification_confidence: float = 0.0
    result_paths: list[str] = field(default_factory=list)
    organizer_database: str = ""
    fragment_concurrency: int = 4
    partial_policy: str = "keep"
    clip_start_seconds: float | None = None
    clip_end_seconds: float | None = None
    subtitle_languages: list[str] = field(default_factory=list)
    subtitle_format: str = "srt"
    subtitle_auto: bool = False
    auth_browser: str = ""
    auth_cookie_file: str = ""
    batch_id: str = ""
    format_notice: str = ""
    subtitle_paths: list[str] = field(default_factory=list)
    subtitle_notice: str = ""
    id: str = field(default_factory=lambda: uuid.uuid4().hex)
    status: str = JobStatus.QUEUED
    title: str = ""
    progress: float = 0.0
    speed: str = ""
    eta: str = ""
    stage: str = "Queued"
    attempts: int = 0
    error_code: str = ""
    error_message: str = ""
    error_detail: str = ""
    result_path: str = ""
    created_at: float = field(default_factory=time.time)
    updated_at: float = field(default_factory=time.time)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "DownloadJob":
        allowed = cls.__dataclass_fields__.keys()
        return cls(**{key: value[key] for key in allowed if key in value})
