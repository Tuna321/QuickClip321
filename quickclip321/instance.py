from __future__ import annotations

import os
from pathlib import Path


class SingleInstance:
    """Keep a process-owned lock so two tray copies cannot run together."""

    def __init__(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        self._handle = open(path, "a+b")
        try:
            if os.name == "nt":
                import msvcrt

                self._handle.seek(0)
                self._handle.write(b"0")
                self._handle.flush()
                self._handle.seek(0)
                msvcrt.locking(self._handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(self._handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as exc:
            self._handle.close()
            raise RuntimeError("QuickClip321 is already running.") from exc

    def close(self) -> None:
        if not self._handle.closed:
            self._handle.close()
