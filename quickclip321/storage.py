from __future__ import annotations

import json
import sqlite3
import threading
import time
from contextlib import contextmanager
from collections.abc import Iterator
from pathlib import Path

from .models import ACTIVE_STATUSES, DownloadJob, JobStatus


class JobStore:
    def __init__(self, path: Path) -> None:
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        self._lock = threading.RLock()
        self._initialize()

    @contextmanager
    def _connect(self) -> Iterator[sqlite3.Connection]:
        connection = sqlite3.connect(self.path, timeout=10)
        try:
            connection.execute("PRAGMA journal_mode=WAL")
            connection.execute("PRAGMA busy_timeout=10000")
            with connection:
                yield connection
        finally:
            connection.close()

    def _initialize(self) -> None:
        with self._connect() as db:
            db.execute(
                "CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, updated_at REAL NOT NULL, data TEXT NOT NULL)"
            )

    def save(self, job: DownloadJob) -> None:
        job.updated_at = time.time()
        stored = job.to_dict()
        # Browser selection and cookie file paths are per-session authorization,
        # not download history. The engine keeps them in memory for active retries.
        stored["auth_browser"] = ""
        stored["auth_cookie_file"] = ""
        payload = json.dumps(stored, ensure_ascii=False)
        with self._lock, self._connect() as db:
            db.execute(
                "INSERT INTO jobs(id, updated_at, data) VALUES(?, ?, ?) "
                "ON CONFLICT(id) DO UPDATE SET updated_at=excluded.updated_at, data=excluded.data",
                (job.id, job.updated_at, payload),
            )

    def get(self, job_id: str) -> DownloadJob | None:
        with self._lock, self._connect() as db:
            row = db.execute("SELECT data FROM jobs WHERE id=?", (job_id,)).fetchone()
        return DownloadJob.from_dict(json.loads(row[0])) if row else None

    def list(self, limit: int = -1) -> list[DownloadJob]:
        with self._lock, self._connect() as db:
            rows = db.execute("SELECT data FROM jobs ORDER BY updated_at DESC LIMIT ?", (limit,)).fetchall()
        return [DownloadJob.from_dict(json.loads(row[0])) for row in rows]

    def recover_interrupted(self) -> int:
        recovered = 0
        for job in self.list():
            if JobStatus(job.status) in ACTIVE_STATUSES:
                job.status = JobStatus.FAILED
                job.stage = "Interrupted; you can retry"
                job.error_code = "INTERRUPTED"
                job.error_message = "The app closed. Partial files were kept; try again."
                self.save(job)
                recovered += 1
        return recovered

    def delete(self, job_id: str) -> None:
        with self._lock, self._connect() as db:
            db.execute("DELETE FROM jobs WHERE id=?", (job_id,))
