import os
import subprocess
import sys
import time
from pathlib import Path

import pytest

from quickclip321.engine import DownloadEngine
from quickclip321.models import DownloadJob, JobStatus
from quickclip321.storage import JobStore


def fake_worker(payload, events, cancelled):
    action = payload["quality"]
    if action == "crash":
        os._exit(7)
    if action == "child_crash":
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
        Path(payload["output_dir"], "child.pid").write_text(str(child.pid))
        os._exit(7)
    if action == "fail":
        events.put({"type": "error", "code": "NETWORK_TIMEOUT", "message": "network", "retryable": True})
        return
    if action == "processing":
        events.put({"type": "progress", "status": "processing"})
        time.sleep(1.2)
    elif action == "wait":
        events.put({"type": "progress", "status": "downloading"})
        time.sleep(20)
    output = Path(payload["output_dir"]) / (payload["id"] + ".mp4")
    output.write_bytes(b"ready")
    events.put({"type": "completed", "path": str(output)})


def wait_for(store, job_id, statuses, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        job = store.get(job_id)
        if job.status in statuses:
            return job
        time.sleep(0.03)
    pytest.fail(f"Timeout waiting for {statuses}: {store.get(job_id)}")


@pytest.fixture
def runner(tmp_path):
    store = JobStore(tmp_path / "jobs.sqlite3")
    engine = DownloadEngine(store, max_retries=2, stall_timeout=5, worker=fake_worker)
    engine.start()
    yield engine, store, tmp_path
    engine.stop()
    assert not any(t.is_alive() for t in engine._threads)


def submit(runner, quality):
    engine, store, directory = runner
    job = DownloadJob(url="https://example.com/video", output_dir=str(directory), quality=quality)
    assert engine.submit(job)
    return job


def test_fast_exit_completion_drained_and_duplicate_rejected(runner):
    engine, store, _ = runner
    job = submit(runner, "best")
    duplicate = DownloadJob.from_dict(job.to_dict())
    duplicate.id = "a" * 32
    assert not engine.submit(duplicate)
    result = wait_for(store, job.id, {JobStatus.COMPLETED})
    assert Path(result.result_path).exists()


def test_cancel_during_actual_work_never_retries(runner):
    engine, store, _ = runner
    job = submit(runner, "wait")
    wait_for(store, job.id, {JobStatus.DOWNLOADING})
    engine.cancel(job.id)
    time.sleep(0.5)
    assert store.get(job.id).status == JobStatus.CANCELLED
    assert store.get(job.id).attempts == 1
    assert not engine.retry(job.id)


def test_cancel_during_backoff(runner):
    engine, store, _ = runner
    job = submit(runner, "fail")
    wait_for(store, job.id, {JobStatus.RETRYING})
    engine.cancel(job.id)
    time.sleep(2.3)
    assert store.get(job.id).status == JobStatus.CANCELLED
    assert store.get(job.id).attempts == 1


def test_retry_limit(runner):
    _, store, _ = runner
    job = submit(runner, "fail")
    result = wait_for(store, job.id, {JobStatus.FAILED})
    assert result.attempts == 2 and result.error_code == "NETWORK_TIMEOUT"


def test_crash_classification(runner):
    engine, store, _ = runner
    engine.max_retries = 1
    job = submit(runner, "crash")
    result = wait_for(store, job.id, {JobStatus.FAILED})
    assert result.error_code == "WORKER_CRASH"
    assert engine.retry(job.id)
    assert not engine.retry(job.id)


def test_conversion_not_network_stall(runner):
    engine, store, _ = runner
    engine.stall_timeout = 0.6
    job = submit(runner, "processing")
    result = wait_for(store, job.id, {JobStatus.COMPLETED})
    assert result.attempts == 1


def test_pause_pending_and_resume(runner):
    engine, store, directory = runner
    first = submit(runner, "wait")
    wait_for(store, first.id, {JobStatus.DOWNLOADING})
    second = submit(runner, "best")
    engine.pause(second.id)
    assert store.get(second.id).status == JobStatus.PAUSED
    assert engine.retry(second.id)
    engine.cancel(first.id)
    wait_for(store, second.id, {JobStatus.COMPLETED})


def test_clear_entries_preserves_completed_media(runner):
    engine, store, _ = runner
    job = submit(runner, "best")
    result = wait_for(store, job.id, {JobStatus.COMPLETED})
    # Wait until scheduler releases ownership before removing record.
    time.sleep(0.3)
    assert engine.remove(job.id)
    assert store.get(job.id) is None
    assert Path(result.result_path).exists()


def test_partial_delete_only_owns_work_folder(runner):
    engine, store, directory = runner
    job = DownloadJob(url="https://example.com/v", output_dir=str(directory), partial_policy="delete")
    work = directory / ".quickclip321" / job.id / "work"
    work.mkdir(parents=True)
    (work / "partial.part").write_bytes(b"partial")
    source = work.parent / "source.mp4"
    source.write_bytes(b"source")
    store.save(job)
    engine.cancel(job.id)
    assert not work.exists()
    assert source.exists()
