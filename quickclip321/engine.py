from __future__ import annotations

import logging
import multiprocessing as mp
import os
import queue
import re
import shutil
import subprocess
import threading
import time
from pathlib import Path
from typing import Callable

from .models import ACTIVE_STATUSES, DownloadJob, JobStatus
from .storage import JobStore
from .worker import execute_job
from .process_owner import ProcessOwner
from .playlist import duplicate_key
from .errors import redact_sensitive


def terminate_tree(process) -> None:
    """Terminate worker and owned FFmpeg children, never unrelated processes."""
    if process.is_alive():
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                capture_output=True,
                timeout=10,
                creationflags=subprocess.CREATE_NO_WINDOW,
            )
        else:
            process.terminate()
        process.join(timeout=3)
        if process.is_alive():
            process.kill()
            process.join(timeout=2)


class DownloadEngine:
    def __init__(
        self, store: JobStore, max_retries=3, stall_timeout=90, concurrency=1, worker=execute_job, context=None
    ) -> None:
        self.store = store
        self.max_retries = max(1, min(5, max_retries))  # total attempts, not nested infinite retries
        self.stall_timeout = stall_timeout
        self.concurrency = max(1, min(3, concurrency))
        self.worker = worker
        self.context = context or mp.get_context("spawn")
        self._pending = queue.Queue()
        self._listeners: list[Callable] = []
        self._lock = threading.RLock()
        self._running = set()
        self._stopping = threading.Event()
        self._threads = []
        self._auth_by_id: dict[str, tuple[str, str]] = {}

    def start(self):
        self.store.recover_interrupted()
        for job in reversed(self.store.list()):
            if job.status == JobStatus.QUEUED:
                # Legacy persisted playlist-wide consent cannot authorize a new session.
                job.playlist_mode = "single"
                self.store.save(job)
                self._pending.put(job.id)
        for index in range(self.concurrency):
            thread = threading.Thread(target=self._run, name=f"QuickClipEngine-{index}", daemon=True)
            self._threads.append(thread)
            thread.start()

    def stop(self):
        self._stopping.set()
        for thread in self._threads:
            thread.join(timeout=15)
        self._auth_by_id.clear()

    def add_listener(self, listener):
        self._listeners.append(listener)

    def _emit(self, job):
        for listener in tuple(self._listeners):
            try:
                listener(DownloadJob.from_dict(job.to_dict()))
            except Exception:
                logging.exception("Job listener failed")

    def _save(self, job):
        self.store.save(job)
        self._emit(job)

    def submit(self, job):
        with self._lock:
            key = duplicate_key(job)
            for existing in self.store.list():
                if existing.status not in ACTIVE_STATUSES | {JobStatus.QUEUED, JobStatus.PAUSED}:
                    continue
                # History deliberately omits session authorization. Compare with
                # the live authorization instead of treating every private job as new.
                existing.auth_browser, existing.auth_cookie_file = self._auth_by_id.get(existing.id, ("", ""))
                if key == duplicate_key(existing):
                    return False
            if self.store.get(job.id):
                return False
            if job.auth_browser or job.auth_cookie_file:
                self._auth_by_id[job.id] = (job.auth_browser, job.auth_cookie_file)
            self._save(job)
            self._pending.put(job.id)
            return True

    def cancel(self, job_id):
        self._interrupt(job_id, JobStatus.CANCELLED, "Cancelled")
        self._auth_by_id.pop(job_id, None)

    def pause(self, job_id):
        self._interrupt(job_id, JobStatus.PAUSED, "Paused; partial files retained")

    def _interrupt(self, job_id, status, stage):
        with self._lock:
            job = self.store.get(job_id)
            if job and job.status in ACTIVE_STATUSES | {JobStatus.QUEUED, JobStatus.PAUSED}:
                job.status, job.stage = status, stage
                self._save(job)
                if status == JobStatus.CANCELLED and job_id not in self._running:
                    self._cleanup_partial(job)

    def retry(self, job_id, *, auth_browser=None, auth_cookie_file=None):
        with self._lock:
            job = self.store.get(job_id)
            if not job or job.id in self._running or job.status not in {JobStatus.FAILED, JobStatus.PAUSED}:
                return False
            if auth_browser is not None or auth_cookie_file is not None:
                self._auth_by_id[job_id] = (auth_browser or "", auth_cookie_file or "")
            job.status, job.stage = JobStatus.QUEUED, "Queued"
            job.attempts = 0
            job.error_code = job.error_message = job.error_detail = ""
            job.speed = job.eta = ""
            self._save(job)
            self._pending.put(job.id)
            return True

    def cancel_all(self):
        for job in self.store.list():
            self.cancel(job.id)

    def clear_pending(self):
        for job in self.store.list():
            if job.status == JobStatus.QUEUED:
                self.cancel(job.id)

    def remove(self, job_id):
        with self._lock:
            job = self.store.get(job_id)
            if (
                job
                and job_id not in self._running
                and job.status in {JobStatus.FAILED, JobStatus.COMPLETED, JobStatus.CANCELLED}
            ):
                self.store.delete(job_id)  # never remove downloaded media
                self._auth_by_id.pop(job_id, None)
                return True
            return False

    def _cleanup_partial(self, job):
        if job.partial_policy == "delete":
            if not re.fullmatch(r"[0-9a-f]{32}", job.id) or not job.output_dir:
                return
            base = Path(job.output_dir).expanduser().resolve()
            target = base / ".quickclip321" / job.id / "work"
            # Reject redirected staging directories, including symlinks and
            # junctions, before deleting anything.
            if target.exists() and target.resolve() == target and target.is_dir():
                shutil.rmtree(target)

    def _run(self):
        while not self._stopping.is_set():
            try:
                job_id = self._pending.get(timeout=0.2)
            except queue.Empty:
                continue
            try:
                with self._lock:
                    job = self.store.get(job_id)
                    if not job or job.status != JobStatus.QUEUED or job_id in self._running:
                        continue
                    self._running.add(job_id)
                self._execute(job)
            except Exception:
                logging.exception("Scheduler failed for %s", job_id)
                with self._lock:
                    job = self.store.get(job_id)
                    if job and job.status not in {JobStatus.CANCELLED, JobStatus.PAUSED}:
                        job.status, job.error_code = JobStatus.FAILED, "ENGINE_ERROR"
                        job.error_message = job.stage = "Engine error; check the log and retry."
                        self._save(job)
            finally:
                with self._lock:
                    self._running.discard(job_id)
                    current = self.store.get(job_id)
                    if current and current.status == JobStatus.QUEUED:
                        self._pending.put(job_id)
                    elif current and current.status in {JobStatus.COMPLETED, JobStatus.CANCELLED}:
                        self._auth_by_id.pop(job_id, None)
                self._pending.task_done()

    def _execute(self, job):
        events = self.context.Queue()
        cancelled = self.context.Event()
        worker_payload = job.to_dict()
        worker_payload["auth_browser"], worker_payload["auth_cookie_file"] = self._auth_by_id.get(job.id, ("", ""))
        process = self.context.Process(target=self.worker, args=(worker_payload, events, cancelled))
        terminal = retryable = False
        last_activity = time.monotonic()
        seen_activity = False
        owner = None
        try:
            with self._lock:
                current = self.store.get(job.id)
                if current.status != JobStatus.QUEUED:
                    return
                job.attempts += 1
                job.error_code = job.error_message = job.error_detail = ""
                job.speed = job.eta = ""
                job.status, job.stage = JobStatus.ANALYZING, "Starting engine"
                self._save(job)
            process.start()
            owner = ProcessOwner(process.pid)
            exited_at = None
            while not self._stopping.is_set():
                with self._lock:
                    current = self.store.get(job.id)
                    if current.status in {JobStatus.CANCELLED, JobStatus.PAUSED}:
                        cancelled.set()
                        job = current
                        break
                try:
                    event = events.get(timeout=0.2)
                    if event.get("type") != "log":
                        last_activity = time.monotonic()
                        seen_activity = True
                    with self._lock:
                        current = self.store.get(job.id)
                        if current.status in {JobStatus.CANCELLED, JobStatus.PAUSED}:
                            job = current
                            break
                        terminal, retryable = self._apply_event(job, event)
                    if terminal:
                        break
                except queue.Empty:
                    pass
                if not process.is_alive():
                    exited_at = exited_at or time.monotonic()
                    if time.monotonic() - exited_at > 3:  # Windows queue feeder may deliver the final event after exit
                        logging.error("worker[%s] exited without a terminal event; exitcode=%s", job.id, process.exitcode)
                        job.error_code, job.error_message = (
                            "WORKER_CRASH",
                            "Worker exited unexpectedly; you can retry.",
                        )
                        retryable = True
                        break
                # Processing has its own lifecycle; never apply a network timeout to FFmpeg.
                if (
                    job.status in {JobStatus.ANALYZING, JobStatus.DOWNLOADING}
                    and time.monotonic() - last_activity > (self.stall_timeout if seen_activity else max(20, self.stall_timeout))
                ):
                    job.error_code, job.error_message = "STALLED", "Network operation stopped responding; partial files retained."
                    retryable = True
                    break
        finally:
            cancelled.set()
            if owner:
                owner.close()
            if process.pid:
                process.join(timeout=0.5 if terminal else 0)
                terminate_tree(process)
                process.close()
            events.close()
            events.join_thread()
        with self._lock:
            current = self.store.get(job.id)
            if current.status in {JobStatus.CANCELLED, JobStatus.PAUSED}:
                if current.status == JobStatus.CANCELLED:
                    self._cleanup_partial(current)
                return
            if job.status == JobStatus.COMPLETED:
                return
            if self._stopping.is_set():
                job.status, job.stage = JobStatus.PAUSED, "App closed; you can resume"
                self._save(job)
                return
            if retryable and job.attempts < self.max_retries:
                delay = min(30, 2**job.attempts)
                job.status, job.stage = JobStatus.RETRYING, f"Retrying in {delay} seconds"
                self._save(job)
            else:
                job.status, job.stage = JobStatus.FAILED, job.error_message or "Download failed"
                self._save(job)
                return
        deadline = time.monotonic() + delay
        while not self._stopping.wait(0.1) and time.monotonic() < deadline:
            if self.store.get(job.id).status != JobStatus.RETRYING:
                return
        with self._lock:
            current = self.store.get(job.id)
            if current.status == JobStatus.RETRYING:
                current.status = JobStatus.PAUSED if self._stopping.is_set() else JobStatus.QUEUED
                self._save(current)

    def _apply_event(self, job, event):
        kind = event.get("type")
        if kind == "log":
            logging.info("worker[%s] %s", job.id, redact_sensitive(event.get("message", ""), (job.auth_cookie_file,)))
            return False, False
        if kind == "progress":
            job.status = event.get("status", job.status)
            job.stage = event.get("stage", job.stage)
            job.progress = max(0, min(1, float(event.get("progress", job.progress))))
            job.speed, job.eta = event.get("speed", ""), event.get("eta", "")
        elif kind == "metadata":
            job.title = event.get("title", job.title)
        elif kind == "format_notice":
            job.format_notice = redact_sensitive(event.get("message", ""))
        elif kind == "subtitle_notice":
            job.subtitle_notice = redact_sensitive(event.get("message", ""))
        elif kind == "destination":
            job.destination = event.get("destination", job.destination)
            job.classification_reason = event.get("reason", "")
            job.classification_confidence = float(event.get("confidence", 0))
        elif kind == "completed":
            path = event.get("path", "")
            if not path or not Path(path).exists():
                job.error_code, job.error_message = "MISSING_OUTPUT", "Completed file was not found."
                return True, False
            job.status, job.stage, job.progress = JobStatus.COMPLETED, "Completed", 1
            job.error_code = job.error_message = job.error_detail = ""
            job.speed = job.eta = ""
            job.result_path, job.title = path, event.get("title") or job.title
            job.destination = event.get("destination") or str(Path(path) if Path(path).is_dir() else Path(path).parent)
            job.result_paths = event.get("paths") or [path]
            job.subtitle_paths = event.get("subtitle_paths") or []
        elif kind == "error":
            job.error_code = event.get("code", "DOWNLOAD_ERROR")
            job.error_message = redact_sensitive(event.get("message", "Download failed"), (job.auth_cookie_file,))
            job.error_detail = redact_sensitive(event.get("detail", ""), (job.auth_cookie_file,))
            logging.error("job[%s] %s: %s", job.id, job.error_code, job.error_detail)
            return True, bool(event.get("retryable"))
        elif kind == "cancelled":
            job.status, job.stage = JobStatus.CANCELLED, "Cancelled"
        self._save(job)
        return kind in {"completed", "cancelled"}, False
