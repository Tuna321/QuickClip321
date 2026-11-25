"""Windows user-bound storage for an optional, user-supplied AI API key."""

from __future__ import annotations

import ctypes
import os
from ctypes import wintypes
from pathlib import Path

class CredentialError(RuntimeError):
    """Credential storage is unavailable for this Windows user."""


class _DataBlob(ctypes.Structure):
    _fields_ = [("cbData", wintypes.DWORD), ("pbData", ctypes.POINTER(ctypes.c_ubyte))]


def _crypt(data: bytes, *, protect: bool) -> bytes:
    if os.name != "nt":
        raise CredentialError("Secure API key storage requires Windows.")
    source_buffer = ctypes.create_string_buffer(data)
    source = _DataBlob(len(data), ctypes.cast(source_buffer, ctypes.POINTER(ctypes.c_ubyte)))
    output = _DataBlob()
    crypt32 = ctypes.windll.crypt32
    method = crypt32.CryptProtectData if protect else crypt32.CryptUnprotectData
    method.argtypes = [
        ctypes.POINTER(_DataBlob),
        ctypes.c_wchar_p if protect else ctypes.c_void_p,
        ctypes.POINTER(_DataBlob),
        ctypes.c_void_p,
        ctypes.c_void_p,
        wintypes.DWORD,
        ctypes.POINTER(_DataBlob),
    ]
    method.restype = wintypes.BOOL
    local_free = ctypes.windll.kernel32.LocalFree
    local_free.argtypes = [ctypes.c_void_p]
    local_free.restype = ctypes.c_void_p
    if protect:
        ok = method(ctypes.byref(source), ctypes.c_wchar_p("QuickClip321 AI key"), None, None, None, 0x01, ctypes.byref(output))
    else:
        ok = method(ctypes.byref(source), None, None, None, None, 0x01, ctypes.byref(output))
    if not ok:
        raise CredentialError("Windows could not protect or read the API key.")
    try:
        return ctypes.string_at(output.pbData, output.cbData)
    finally:
        local_free(output.pbData)


class CredentialStore:
    def __init__(self, path: Path | None = None) -> None:
        # API keys stay on this Windows account even when the app itself is portable.
        local_app_data = os.environ.get("LOCALAPPDATA")
        default = (Path(local_app_data) if local_app_data else Path.home() / "AppData" / "Local") / "QuickClip321" / "ai-key.dpapi"
        self.path = path or default

    def save(self, value: str) -> None:
        if not value or len(value) > 4096:
            raise ValueError("Enter a valid API key.")
        encrypted = _crypt(value.encode("utf-8"), protect=True)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix(".tmp")
        temporary.write_bytes(encrypted)
        temporary.replace(self.path)

    def load(self) -> str:
        try:
            encrypted = self.path.read_bytes()
        except FileNotFoundError:
            return ""
        try:
            return _crypt(encrypted, protect=False).decode("utf-8")
        except (UnicodeDecodeError, CredentialError):
            raise CredentialError("The saved API key cannot be read on this Windows account.") from None

    def clear(self) -> None:
        self.path.unlink(missing_ok=True)
