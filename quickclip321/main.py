from __future__ import annotations

import logging
import multiprocessing
import os
import sys
from logging.handlers import RotatingFileHandler

from .errors import redact_sensitive
from .instance import SingleInstance
from .settings import app_data_dir


class RedactingFormatter(logging.Formatter):
    """Sanitize complete log lines, including formatted exception tracebacks."""

    def format(self, record: logging.LogRecord) -> str:
        return redact_sensitive(super().format(record))


def configure_logging() -> None:
    log_dir = app_data_dir() / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    log_path = log_dir / "quickclip321.log"
    root = logging.getLogger()
    if any(isinstance(handler, RotatingFileHandler) and handler.baseFilename == str(log_path) for handler in root.handlers):
        return
    handler = RotatingFileHandler(log_path, maxBytes=2_000_000, backupCount=3, encoding="utf-8")
    handler.setFormatter(RedactingFormatter("%(asctime)s %(levelname)s %(name)s %(message)s"))
    root.addHandler(handler)
    root.setLevel(logging.INFO)


def report_startup_failure(stage: str, error: Exception) -> None:
    message = redact_sensitive(f"QuickClip321 could not {stage}: {error}")
    if sys.stderr is not None:
        print(message, file=sys.stderr)
    if os.name == "nt" and (getattr(sys, "frozen", False) or sys.stderr is None):
        try:
            import ctypes

            ctypes.windll.user32.MessageBoxW(None, message, "QuickClip321 startup error", 0x10)
        except (AttributeError, OSError):
            pass


def main() -> int:
    multiprocessing.freeze_support()
    try:
        configure_logging()
    except OSError as error:
        report_startup_failure("open the log directory", error)
        return 1
    try:
        instance = SingleInstance(app_data_dir() / "quickclip321.lock")
    except RuntimeError:
        return 0
    except OSError as error:
        report_startup_failure("open the application data directory", error)
        return 1
    pid_path = app_data_dir() / "quickclip321.pid"
    try:
        pid_path.write_text(str(os.getpid()), encoding="ascii")
        from .ui import QuickClipApp

        QuickClipApp(start_hidden="--start-hidden" in sys.argv[1:]).mainloop()
        return 0
    except Exception as error:
        logging.exception("QuickClip321 crashed")
        report_startup_failure("start", error)
        return 1
    finally:
        try:
            pid_path.unlink(missing_ok=True)
        except OSError:
            pass
        instance.close()


if __name__ == "__main__":
    sys.exit(main())
