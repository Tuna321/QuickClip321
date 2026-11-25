from quickclip321.models import DownloadJob
from quickclip321.models import JobStatus
from quickclip321.engine import DownloadEngine
from quickclip321.storage import JobStore


def test_auth_selection_never_enters_persisted_job_history(tmp_path):
    store = JobStore(tmp_path / "jobs.sqlite3")
    job = DownloadJob(
        url="https://example.org/video",
        auth_browser="firefox",
        auth_cookie_file=str(tmp_path / "private-cookies.txt"),
    )
    store.save(job)

    assert job.auth_browser == "firefox"  # In-memory job can still use this session.
    assert job.auth_cookie_file.endswith("private-cookies.txt")
    loaded = store.get(job.id)
    assert loaded.auth_browser == ""
    assert loaded.auth_cookie_file == ""
    raw = (tmp_path / "jobs.sqlite3").read_bytes()
    assert b"private-cookies.txt" not in raw
    assert b"firefox" not in raw


def test_retry_session_is_used_without_persisting_cookie_path(tmp_path):
    store = JobStore(tmp_path / "jobs.sqlite3")
    job = DownloadJob(url="https://example.org/video", status=JobStatus.FAILED)
    store.save(job)
    engine = DownloadEngine(store)
    cookie_path = str(tmp_path / "private-cookies.txt")

    assert engine.retry(job.id, auth_cookie_file=cookie_path)
    assert engine._auth_by_id[job.id] == ("", cookie_path)
    assert store.get(job.id).status == JobStatus.QUEUED
    assert store.get(job.id).auth_cookie_file == ""
    assert b"private-cookies.txt" not in store.path.read_bytes()
