"""Separate portable test build: never reads or writes the installed app's state."""

import multiprocessing
import os
import sys
import tempfile
from pathlib import Path


DATA_DIRECTORY_NAME = "QuickClip321-2.1.0-rc.1-data"


def _writable(directory: Path) -> bool:
    try:
        directory.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryFile(dir=directory):
            pass
        return True
    except OSError:
        return False


def select_data_directory(base: Path) -> Path:
    """Keep portable state beside the executable when writable; isolate fallback state."""
    portable = base / DATA_DIRECTORY_NAME
    if _writable(portable):
        return portable
    user_base = os.environ.get("LOCALAPPDATA") or os.environ.get("XDG_DATA_HOME")
    fallback = (Path(user_base) if user_base else Path.home() / ".local" / "share") / "QuickClip321-Portable" / DATA_DIRECTORY_NAME
    if _writable(fallback):
        return fallback
    raise OSError(f"Neither portable data directory ({portable}) nor user data directory ({fallback}) is writable")


if __name__ == "__main__":
    multiprocessing.freeze_support()
    base = Path(sys.executable).parent if getattr(sys, "frozen", False) else Path(__file__).resolve().parent
    if "QUICKCLIP321_DATA_DIR" not in os.environ:
        try:
            os.environ["QUICKCLIP321_DATA_DIR"] = str(select_data_directory(base))
        except OSError as error:
            from quickclip321.main import report_startup_failure

            report_startup_failure("select a writable data directory", error)
            raise SystemExit(1)
    if "--smoke-test" in sys.argv:
        from quickclip321.diagnostics import packaged_smoke

        raise SystemExit(packaged_smoke(sys.argv[sys.argv.index("--smoke-test") + 1]))
    from quickclip321.main import main

    raise SystemExit(main())
