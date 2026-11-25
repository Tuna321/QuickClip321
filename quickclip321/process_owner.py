"""Windows Job Object: closing the supervisor handle also reaps orphaned children."""

import ctypes
import os
from ctypes import wintypes


class ProcessOwner:
    def __init__(self, pid):
        self.handle = None
        if os.name != "nt":
            return
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)

        class Basic(ctypes.Structure):
            _fields_ = [
                ("ProcessTime", ctypes.c_int64),
                ("JobTime", ctypes.c_int64),
                ("Flags", wintypes.DWORD),
                ("MinWorking", ctypes.c_size_t),
                ("MaxWorking", ctypes.c_size_t),
                ("Active", wintypes.DWORD),
                ("Affinity", ctypes.c_size_t),
                ("Priority", wintypes.DWORD),
                ("Scheduling", wintypes.DWORD),
            ]

        class IO(ctypes.Structure):
            _fields_ = [
                (name, ctypes.c_uint64)
                for name in ("ReadOps", "WriteOps", "OtherOps", "ReadBytes", "WriteBytes", "OtherBytes")
            ]

        class Extended(ctypes.Structure):
            _fields_ = [
                ("Basic", Basic),
                ("IO", IO),
                ("ProcessMemory", ctypes.c_size_t),
                ("JobMemory", ctypes.c_size_t),
                ("PeakProcess", ctypes.c_size_t),
                ("PeakJob", ctypes.c_size_t),
            ]

        kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
        kernel.CreateJobObjectW.restype = wintypes.HANDLE
        kernel.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
        kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        self.kernel = kernel
        handle = kernel.CreateJobObjectW(None, None)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        info = Extended()
        info.Basic.Flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        child = kernel.OpenProcess(0x0100 | 0x0001, False, pid)
        try:
            if not child or not kernel.SetInformationJobObject(handle, 9, ctypes.byref(info), ctypes.sizeof(info)):
                raise ctypes.WinError(ctypes.get_last_error())
            if not kernel.AssignProcessToJobObject(handle, child):
                raise ctypes.WinError(ctypes.get_last_error())
            self.handle = handle
        finally:
            if child:
                kernel.CloseHandle(child)
            if self.handle is None:
                kernel.CloseHandle(handle)

    def close(self):
        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None
