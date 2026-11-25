import ctypes
import os
from ctypes import wintypes

import pytest

from quickclip321.models import JobStatus
from test_engine import submit, wait_for, runner  # noqa: F401


@pytest.mark.skipif(os.name != "nt", reason="Windows Job Object integration")
def test_worker_crash_reaps_owned_child(runner):  # noqa: F811
    engine, store, directory = runner
    engine.max_retries = 1
    job = submit(runner, "child_crash")
    wait_for(store, job.id, {JobStatus.FAILED})
    pid = int((directory / "child.pid").read_text())
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = kernel.OpenProcess(0x00100000, False, pid)
    if handle:
        try:
            assert kernel.WaitForSingleObject(handle, 2000) == 0
        finally:
            kernel.CloseHandle(handle)
