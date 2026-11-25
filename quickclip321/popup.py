"""Windows tray anchoring and popup styles; geometry math is independently testable."""

from __future__ import annotations

import ctypes
import os
from ctypes import wintypes
from dataclasses import dataclass


@dataclass(frozen=True)
class Rect:
    left: int
    top: int
    right: int
    bottom: int

    def contains(self, x, y):
        return self.left <= x < self.right and self.top <= y < self.bottom


def placement(anchor: Rect, monitor: Rect, work: Rect, size, dpi=96):
    """All coordinates are physical pixels, including monitors left of the primary."""
    gap = max(4, round(6 * dpi / 96))
    width, height = min(size[0], work.right - work.left), min(size[1], work.bottom - work.top)
    if work.top > monitor.top:
        edge = "top"
    elif work.bottom < monitor.bottom:
        edge = "bottom"
    elif work.left > monitor.left:
        edge = "left"
    elif work.right < monitor.right:
        edge = "right"
    else:
        cx, cy = (anchor.left + anchor.right) // 2, (anchor.top + anchor.bottom) // 2
        edge = min(
            {
                "left": abs(cx - monitor.left),
                "right": abs(cx - monitor.right),
                "top": abs(cy - monitor.top),
                "bottom": abs(cy - monitor.bottom),
            },
            key=lambda key: {
                "left": abs(cx - monitor.left),
                "right": abs(cx - monitor.right),
                "top": abs(cy - monitor.top),
                "bottom": abs(cy - monitor.bottom),
            }[key],
        )
    x = anchor.right - width
    y = anchor.top - height - gap
    if edge == "top":
        y = max(work.top, anchor.bottom) + gap
    elif edge == "left":
        x, y = max(work.left, anchor.right) + gap, anchor.bottom - height
    elif edge == "right":
        x, y = min(work.right, anchor.left) - width - gap, anchor.bottom - height
    x = max(work.left, min(x, work.right - width))
    y = max(work.top, min(y, work.bottom - height))
    return Rect(x, y, x + width, y + height)


def positioned(work: Rect, size, mode="Tray", anchor=None, monitor=None, dpi=96, ratios=(0.5, 0.5)):
    """Clamp a fixed popup to one monitor's work area."""
    width = min(max(1, size[0]), work.right - work.left)
    height = min(max(1, size[1]), work.bottom - work.top)
    if mode == "Tray" and anchor and monitor:
        return placement(anchor, monitor, work, (width, height), dpi)
    if mode == "Custom":
        x = work.left + round(max(0.0, min(1.0, ratios[0])) * (work.right - work.left - width))
        y = work.top + round(max(0.0, min(1.0, ratios[1])) * (work.bottom - work.top - height))
    else:
        x, y = work.right - width, work.bottom - height
    return Rect(x, y, x + width, y + height)


def _user():
    user = ctypes.WinDLL("user32", use_last_error=True)
    user.GetAncestor.argtypes = [wintypes.HWND, wintypes.UINT]
    user.GetAncestor.restype = wintypes.HWND
    user.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
    user.SetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int, wintypes.LONG]
    user.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user.SetWindowPos.argtypes = [
        wintypes.HWND,
        wintypes.HWND,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        wintypes.UINT,
    ]
    user.GetForegroundWindow.restype = wintypes.HWND
    user.WindowFromPoint.argtypes = [wintypes.POINT]
    user.WindowFromPoint.restype = wintypes.HWND
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.MonitorFromRect.argtypes = [ctypes.POINTER(wintypes.RECT), wintypes.DWORD]
    user.MonitorFromRect.restype = wintypes.HANDLE
    user.GetMonitorInfoW.argtypes = [wintypes.HANDLE, ctypes.c_void_p]
    user.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
    user.FindWindowW.restype = wintypes.HWND
    user.SetForegroundWindow.argtypes = [wintypes.HWND]
    user.GetAsyncKeyState.argtypes = [ctypes.c_int]
    user.GetAsyncKeyState.restype = ctypes.c_short
    return user


def enable_dpi_awareness():
    if os.name == "nt":
        user = _user()
        if hasattr(user, "SetProcessDpiAwarenessContext"):
            user.SetProcessDpiAwarenessContext.argtypes = [ctypes.c_void_p]
            # Per-monitor V2; an existing process awareness setting is left intact.
            user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))


def hwnd_for(window):
    return _user().GetAncestor(window.winfo_id(), 2)  # GA_ROOT, Tk wrapper rather than content HWND


def rect_for(hwnd):
    rect = wintypes.RECT()
    if not _user().GetWindowRect(hwnd, ctypes.byref(rect)):
        raise ctypes.WinError(ctypes.get_last_error())
    return Rect(rect.left, rect.top, rect.right, rect.bottom)


def popup_styles(window):
    if os.name != "nt":
        return
    user, hwnd = _user(), hwnd_for(window)
    style = user.GetWindowLongW(hwnd, -16)
    user.SetWindowLongW(hwnd, -16, style & ~0x00C00000 & ~0x00040000)  # caption, thick frame
    exstyle = user.GetWindowLongW(hwnd, -20)
    user.SetWindowLongW(hwnd, -20, (exstyle | 0x80) & ~0x40000)  # TOOLWINDOW, clear APPWINDOW
    user.SetWindowPos(hwnd, None, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0004 | 0x0020 | 0x0010)


def icon_rect(icon):
    if os.name != "nt" or not getattr(icon, "_hwnd", None):
        return None

    class GUID(ctypes.Structure):
        _fields_ = [
            ("Data1", wintypes.DWORD),
            ("Data2", wintypes.WORD),
            ("Data3", wintypes.WORD),
            ("Data4", ctypes.c_ubyte * 8),
        ]

    class Identifier(ctypes.Structure):
        _fields_ = [("cbSize", wintypes.DWORD), ("hWnd", wintypes.HWND), ("uID", wintypes.UINT), ("guidItem", GUID)]

    shell = ctypes.WinDLL("shell32", use_last_error=True)
    shell.Shell_NotifyIconGetRect.argtypes = [ctypes.POINTER(Identifier), ctypes.POINTER(wintypes.RECT)]
    shell.Shell_NotifyIconGetRect.restype = ctypes.c_long
    identifier = Identifier()
    identifier.cbSize, identifier.hWnd, identifier.uID = ctypes.sizeof(Identifier), icon._hwnd, id(icon) & 0xFFFFFFFF
    rect = wintypes.RECT()
    # pystray 0.19.5 sets hID instead of uID, so its actual registered uID is zero.
    # Also support versions which register the truncated Python object identifier.
    for uid in (0, id(icon) & 0xFFFFFFFF):
        identifier.uID = uid
        if shell.Shell_NotifyIconGetRect(ctypes.byref(identifier), ctypes.byref(rect)) == 0 and rect.right > rect.left:
            return Rect(rect.left, rect.top, rect.right, rect.bottom)
    return None


def tray_context(icon):
    user = _user()
    anchor = icon_rect(icon)
    source = "icon"
    if anchor is None:
        source = "taskbar-fallback"
        taskbar = user.FindWindowW("Shell_TrayWnd", None)
        if taskbar:
            bar = rect_for(taskbar)
            anchor = Rect(bar.right - 32, bar.top, bar.right - 8, bar.bottom)
        else:
            point = wintypes.POINT()
            user.GetCursorPos(ctypes.byref(point))
            anchor = Rect(point.x, point.y, point.x + 1, point.y + 1)
    native = wintypes.RECT(anchor.left, anchor.top, anchor.right, anchor.bottom)
    monitor_handle = user.MonitorFromRect(ctypes.byref(native), 2)

    class MonitorInfo(ctypes.Structure):
        _fields_ = [
            ("cbSize", wintypes.DWORD),
            ("rcMonitor", wintypes.RECT),
            ("rcWork", wintypes.RECT),
            ("dwFlags", wintypes.DWORD),
        ]

    info = MonitorInfo()
    info.cbSize = ctypes.sizeof(info)
    if not user.GetMonitorInfoW(monitor_handle, ctypes.byref(info)):
        raise ctypes.WinError(ctypes.get_last_error())
    monitor = Rect(info.rcMonitor.left, info.rcMonitor.top, info.rcMonitor.right, info.rcMonitor.bottom)
    work = Rect(info.rcWork.left, info.rcWork.top, info.rcWork.right, info.rcWork.bottom)
    dpi = wintypes.UINT(96)
    try:
        shcore = ctypes.WinDLL("shcore")
        shcore.GetDpiForMonitor.argtypes = [
            wintypes.HANDLE,
            ctypes.c_int,
            ctypes.POINTER(wintypes.UINT),
            ctypes.POINTER(wintypes.UINT),
        ]
        y_dpi = wintypes.UINT()
        shcore.GetDpiForMonitor(monitor_handle, 0, ctypes.byref(dpi), ctypes.byref(y_dpi))
    except (OSError, AttributeError):
        pass
    return {"anchor": anchor, "monitor": monitor, "work": work, "dpi": dpi.value, "source": source}


def _clip_popup_corners(user, hwnd, width, height, dpi):
    gdi = ctypes.windll.gdi32
    gdi.CreateRoundRectRgn.argtypes = [ctypes.c_int] * 6
    gdi.CreateRoundRectRgn.restype = wintypes.HANDLE
    gdi.DeleteObject.argtypes = [wintypes.HANDLE]
    user.SetWindowRgn.argtypes = [wintypes.HWND, wintypes.HANDLE, wintypes.BOOL]
    diameter = max(2, round(32 * dpi / 96))
    region = gdi.CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter)
    # Windows owns the region after a successful SetWindowRgn.
    if region and not user.SetWindowRgn(hwnd, region, True):
        gdi.DeleteObject(region)


def position_popup(window, icon, activate=False, mode="Tray", ratios=(0.5, 0.5)):
    context = tray_context(icon)
    hwnd = hwnd_for(window)
    current = rect_for(hwnd)
    target = positioned(
        context["work"],
        (current.right - current.left, current.bottom - current.top),
        mode,
        context["anchor"],
        context["monitor"],
        context["dpi"],
        ratios,
    )
    user = _user()
    user.SetWindowPos(
        hwnd, wintypes.HWND(-1), target.left, target.top, target.right - target.left, target.bottom - target.top, 0x0010
    )
    _clip_popup_corners(user, hwnd, target.right - target.left, target.bottom - target.top, context["dpi"])
    if activate:
        user.SetForegroundWindow(hwnd)
    return {**context, "popup": target}


def pointer_state(window, icon):
    user = _user()
    point = wintypes.POINT()
    user.GetCursorPos(ctypes.byref(point))
    under_pointer = user.WindowFromPoint(point)
    pid = wintypes.DWORD()
    user.GetWindowThreadProcessId(under_pointer, ctypes.byref(pid))
    bounds, tray = rect_for(hwnd_for(window)), icon_rect(icon)
    return {
        "pressed": bool(user.GetAsyncKeyState(1) & 0x8001),
        "inside": bounds.contains(point.x, point.y),
        "owned": pid.value == os.getpid(),
        "on_tray": bool(tray and tray.contains(point.x, point.y)),
    }


def foreground_owned():
    user = _user()
    pid = wintypes.DWORD()
    user.GetWindowThreadProcessId(user.GetForegroundWindow(), ctypes.byref(pid))
    return pid.value == os.getpid()


def should_dismiss(state, was_pressed, dialog_open=False):
    return (
        not dialog_open
        and state["pressed"]
        and not was_pressed
        and not state["inside"]
        and not state["owned"]
        and not state["on_tray"]
    )
