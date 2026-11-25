"""Capture isolated QuickClip321 windows for visual review on Windows."""

from __future__ import annotations

import ctypes
import os
import sys
from ctypes import wintypes
from pathlib import Path

from PIL import Image


def capture(app, path: Path) -> None:
    user = ctypes.WinDLL("user32")
    gdi = ctypes.WinDLL("gdi32")
    user.GetParent.argtypes = [wintypes.HWND]
    user.GetParent.restype = wintypes.HWND
    user.GetWindowDC.argtypes = [wintypes.HWND]
    user.GetWindowDC.restype = wintypes.HDC
    user.PrintWindow.argtypes = [wintypes.HWND, wintypes.HDC, wintypes.UINT]
    user.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
    gdi.CreateCompatibleDC.argtypes = [wintypes.HDC]
    gdi.CreateCompatibleDC.restype = wintypes.HDC
    gdi.CreateCompatibleBitmap.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int]
    gdi.CreateCompatibleBitmap.restype = wintypes.HBITMAP
    gdi.SelectObject.argtypes = [wintypes.HDC, wintypes.HGDIOBJ]
    gdi.SelectObject.restype = wintypes.HGDIOBJ
    gdi.GetBitmapBits.argtypes = [wintypes.HBITMAP, ctypes.c_long, ctypes.c_void_p]
    gdi.DeleteObject.argtypes = [wintypes.HGDIOBJ]
    gdi.DeleteDC.argtypes = [wintypes.HDC]
    hwnd = user.GetParent(app.winfo_id())
    rect = wintypes.RECT()
    user.GetWindowRect(hwnd, ctypes.byref(rect))
    width, height = rect.right - rect.left, rect.bottom - rect.top
    dc = user.GetWindowDC(hwnd)
    memory_dc = gdi.CreateCompatibleDC(dc)
    bitmap = gdi.CreateCompatibleBitmap(dc, width, height)
    old = gdi.SelectObject(memory_dc, bitmap)
    try:
        if not user.PrintWindow(hwnd, memory_dc, 2):
            raise OSError("PrintWindow failed")
        data = ctypes.create_string_buffer(width * height * 4)
        gdi.GetBitmapBits(bitmap, len(data), data)
        Image.frombuffer("RGB", (width, height), data.raw, "raw", "BGRX", 0, 1).save(path)
    finally:
        gdi.SelectObject(memory_dc, old)
        gdi.DeleteObject(bitmap)
        gdi.DeleteDC(memory_dc)
        user.ReleaseDC(hwnd, dc)


def main() -> None:
    theme = sys.argv[1] if len(sys.argv) > 1 else "Dark"
    if theme not in {"Light", "Dark", "System"}:
        raise ValueError("Theme must be Light, Dark, or System")
    root = Path(__file__).resolve().parents[1] / "test-results" / f"visual-review-{theme.lower()}"
    root.mkdir(parents=True, exist_ok=True)
    os.environ["QUICKCLIP321_DATA_DIR"] = str(root / "data")
    from quickclip321.settings import SettingsStore

    store = SettingsStore()
    settings = store.load()
    settings.clipboard_monitor = settings.notifications = False
    settings.theme_mode = theme
    store.save(settings)
    from quickclip321.ui import QuickClipApp

    app = QuickClipApp()
    try:
        app.show_window()
        app.update()
        capture(app, root / "quick.png")
        app.toggle_expanded()
        app.update()
        capture(app, root / "advanced.png")
        app._navigate("Downloads")
        app.update()
        capture(app, root / "downloads.png")
        app._navigate("History")
        app.update()
        capture(app, root / "history.png")
        app._navigate("Settings")
        app.update()
        capture(app, root / "settings.png")
        app._navigate("Diagnostics")
        app.update()
        capture(app, root / "diagnostics.png")
    finally:
        app._exit_on_ui()


if __name__ == "__main__":
    main()
