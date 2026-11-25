import logging
import sys

import pytest

from quickclip321.main import RedactingFormatter, configure_logging, main
from test_launcher import DATA_DIRECTORY_NAME, select_data_directory


def test_portable_data_stays_beside_executable_when_writable(tmp_path, monkeypatch):
    monkeypatch.setenv("LOCALAPPDATA", str(tmp_path / "user"))
    data = select_data_directory(tmp_path / "portable")
    assert data == tmp_path / "portable" / DATA_DIRECTORY_NAME
    assert data.is_dir()
    assert not (tmp_path / "user").exists()


def test_portable_data_falls_back_to_isolated_user_directory(tmp_path, monkeypatch):
    import test_launcher

    portable = tmp_path / "portable" / DATA_DIRECTORY_NAME
    monkeypatch.setenv("LOCALAPPDATA", str(tmp_path / "user"))
    original = test_launcher._writable
    monkeypatch.setattr(test_launcher, "_writable", lambda path: False if path == portable else original(path))
    data = select_data_directory(tmp_path / "portable")
    assert data == tmp_path / "user" / "QuickClip321-Portable" / DATA_DIRECTORY_NAME
    assert data.is_dir()


def test_portable_data_reports_both_unwritable_locations(tmp_path, monkeypatch):
    import test_launcher

    monkeypatch.setenv("LOCALAPPDATA", str(tmp_path / "user"))
    monkeypatch.setattr(test_launcher, "_writable", lambda path: False)
    with pytest.raises(OSError, match="Neither portable data directory"):
        select_data_directory(tmp_path / "portable")


def test_log_file_is_rotating_and_redacts_credentials(tmp_path, monkeypatch):
    monkeypatch.setenv("QUICKCLIP321_DATA_DIR", str(tmp_path))
    root = logging.getLogger()
    previous_level = root.level
    previous_handlers = set(root.handlers)
    try:
        configure_logging()
        configure_logging()
        added = set(root.handlers) - previous_handlers
        assert len(added) == 1
        handler = added.pop()
        assert handler.maxBytes == 2_000_000
        assert handler.backupCount == 3
        root.error("Download failed: https://example.com/video?token=private-value&v=123")
        handler.flush()
        log = (tmp_path / "logs" / "quickclip321.log").read_text(encoding="utf-8")
        assert "private-value" not in log
        assert "v=123" in log
        assert "token=%5Bredacted%5D" in log
    finally:
        for handler in set(root.handlers) - previous_handlers:
            root.removeHandler(handler)
            handler.close()
        root.setLevel(previous_level)


def test_exception_traceback_is_redacted():
    formatter = RedactingFormatter("%(message)s")
    try:
        raise ValueError("Authorization: private-key")
    except ValueError:
        record = logging.LogRecord("test", logging.ERROR, __file__, 1, "Failed", (), sys.exc_info())
    assert "private-key" not in formatter.format(record)


def test_main_reports_log_startup_failure(monkeypatch, capsys):
    import quickclip321.main as entry

    def fail():
        raise OSError("read-only storage")

    monkeypatch.setattr(entry, "configure_logging", fail)
    assert main() == 1
    assert "read-only storage" in capsys.readouterr().err
