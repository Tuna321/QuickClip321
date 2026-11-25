from quickclip321.models import DownloadJob, JobStatus
from quickclip321.storage import JobStore


def test_job_round_trip(tmp_path):
    store = JobStore(tmp_path / "jobs.sqlite3")
    job = DownloadJob(url="https://example.com/video", output_dir=str(tmp_path))
    store.save(job)
    loaded = store.get(job.id)
    assert loaded is not None
    assert loaded.url == job.url
    assert loaded.playlist_mode == "single"


def test_interrupted_jobs_recover_for_manual_retry(tmp_path):
    store = JobStore(tmp_path / "jobs.sqlite3")
    job = DownloadJob(url="https://example.com/video", output_dir=str(tmp_path), status=JobStatus.DOWNLOADING)
    store.save(job)
    assert store.recover_interrupted() == 1
    assert store.get(job.id).status == JobStatus.FAILED
    assert store.get(job.id).error_code == "INTERRUPTED"


def test_connection_is_closed_after_transaction(tmp_path):
    import sqlite3
    import pytest

    store = JobStore(tmp_path / "connections.sqlite3")
    with store._connect() as connection:
        connection.execute("SELECT 1")
    with pytest.raises(sqlite3.ProgrammingError, match="closed"):
        connection.execute("SELECT 1")
