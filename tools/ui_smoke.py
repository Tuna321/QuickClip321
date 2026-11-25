import json
import multiprocessing as mp
import os
import time
from pathlib import Path
from unittest.mock import patch


def smoke_worker(payload, events, cancelled):
    events.put({"type": "progress", "status": "downloading", "stage": "Test indiriliyor", "progress": 0.4})
    time.sleep(1.5)
    path = Path(payload["output_dir"]) / "smoke.mp4"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"test media fixture")
    events.put({"type": "completed", "path": str(path), "title": "Arka plan indirme testi"})


def run():
    from PIL import Image
    import ctypes
    from ctypes import wintypes
    from quickclip321.ui import QuickClipApp
    from quickclip321.models import JobStatus
    from quickclip321.settings import SettingsStore

    root = Path(__file__).resolve().parents[1] / "test-results"
    root.mkdir(parents=True, exist_ok=True)
    os.environ["QUICKCLIP321_DATA_DIR"] = str(root / "ui-smoke-data")
    store = SettingsStore()
    settings = store.load()
    settings.notifications = False
    settings.output_dir = str(root / "ui-smoke-media")
    store.save(settings)
    started = time.perf_counter()
    app = QuickClipApp()
    app._start_preview = lambda: None
    app.engine.worker = smoke_worker
    results = {"startup_seconds": time.perf_counter() - started}

    def tick(seconds=0.2):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            app.update()
            time.sleep(0.01)

    tick(0.5)
    from quickclip321 import popup

    user = popup._user()
    native_hwnd = popup.hwnd_for(app)
    style = user.GetWindowLongW(native_hwnd, -16)
    exstyle = user.GetWindowLongW(native_hwnd, -20)
    assert app.overrideredirect()
    assert style & 0x00C00000 == 0
    assert exstyle & 0x80 and not exstyle & 0x40000
    results["native_titlebar_absent"] = True
    results["native_toolwindow_no_taskbar_style"] = True
    context = app._popup_context
    bounds = popup.rect_for(native_hwnd)
    work = context["work"]
    assert work.left <= bounds.left < bounds.right <= work.right
    assert work.top <= bounds.top < bounds.bottom <= work.bottom
    results["tray_placement"] = {
        key: (vars(value) if hasattr(value, "__dataclass_fields__") else value) for key, value in context.items()
    }
    results["quick_progress_mapped"] = bool(app.quick_progress.winfo_ismapped())
    assert results["quick_progress_mapped"]
    assert context["source"] == "icon"
    results["required_panel_height"] = app.root_panel.winfo_reqheight()
    results["quick_height"] = app.winfo_height()
    # Exercise the Windows message used by pystray's real left-button handler.
    user.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    tray_hwnd = app.tray_icon._hwnd
    for _ in range(6):
        before = app.visible
        user.PostMessageW(tray_hwnd, 0x400 + 11, 0, 0x202)
        tick(0.3)
        assert app.visible != before
        assert popup.hwnd_for(app) == native_hwnd and app.tray_icon._hwnd == tray_hwnd
    results["native_tray_toggle_repeated_single_window"] = True
    user.PostMessageW(native_hwnd, 0x0010, 0, 0)
    tick(0.3)
    assert not app.visible and not app._closing
    assert any(thread.is_alive() for thread in app.engine._threads)
    app.show_window()
    tick(0.2)
    results["native_close_hides_without_stopping_engine"] = True
    app.tray_icon()
    tick(0.25)
    assert not app.visible
    app.tray_icon()
    tick(0.25)
    assert app.visible
    results["tray_default_action_toggles"] = True
    try:
        previous_clipboard = app.clipboard_get()
    except Exception:
        previous_clipboard = ""
    app.clipboard_clear()
    app.clipboard_append("https://youtube.com/watch?v=smoketest&list=PLtest")
    app.hide_window()
    app._read_clipboard()
    assert not app.visible
    assert app.url_var.get().endswith("list=PLtest")
    assert app.playlist_var.get() == "single"
    app.show_window()
    app.playlist_var.set("all")
    app.url_var.set("https://youtube.com/watch?v=smoketest")
    assert app.playlist_var.get() == "single"
    assert not app.playlist_frame.winfo_manager()
    from quickclip321.organizer import load_database

    database = load_database(app.database_path)
    app.format_var.set("mp3")
    # Deliver an asynchronous preview result through the normal UI consumer.
    import queue

    app._preview_url = app.url_var.get()
    app._preview_process = object()
    app._preview_queue = queue.Queue()
    app._preview_queue.put({"title": "A type beat", "metadata": {"title": "A type beat"}})
    with patch.object(app, "_stop_preview", side_effect=lambda: setattr(app, "_preview_process", None)):
        app._poll_preview()
    app._preview_queue = None
    assert app.destination_var.get() == database["destinations"]["beats"]
    results["preview_updates_destination"] = True
    manual = root / "manual-choice"
    with patch("quickclip321.ui.filedialog.askdirectory", return_value=str(manual)):
        app.choose_folder()
    assert app.destination_var.get() == str(manual)
    with patch.object(app.engine, "submit", return_value=True) as submission:
        app.start_download()
    assert submission.call_args.args[0].selected_destination == str(manual)
    assert submission.call_args.args[0].manual_destination
    results["ui_manual_destination_reaches_job"] = True
    app._reset_destination()
    app._save_setting("smart_organizer", False)
    assert app.destination_var.get() == app.settings.output_dir
    app._save_setting("smart_organizer", True)
    app.url_var.set("https://youtube.com/watch?v=background-smoke")
    app.start_download()
    tick(0.4)
    job = app.job_store.list()[0]
    app.hide_window()
    tick(2.5)
    assert app.job_store.get(job.id).status == JobStatus.COMPLETED
    assert not app.visible
    results["hide_download_continues"] = True
    results["clipboard_no_auto_open_or_download"] = True
    results["playlist_consent_reset"] = True
    app.show_window()
    tick(0.2)

    def screenshot(name):
        # Print only this app window; desktop overlays are excluded.
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
            assert user.PrintWindow(hwnd, memory_dc, 2)
            data = ctypes.create_string_buffer(width * height * 4)
            gdi.GetBitmapBits(bitmap, len(data), data)
            Image.frombuffer("RGB", (width, height), data.raw, "raw", "BGRX", 0, 1).save(root / name)
        finally:
            gdi.SelectObject(memory_dc, old)
            gdi.DeleteObject(bitmap)
            gdi.DeleteDC(memory_dc)
            user.ReleaseDC(hwnd, dc)

    app.url_var.set("https://youtube.com/watch?v=preview-fixture")
    app.format_var.set("mp3")
    app._preview_info = {"title": "Dark type beat instrumental"}
    app.preview_title.configure(text="Dark type beat instrumental")
    app._update_destination()
    app._resize_quick()
    tick(0.25)
    screenshot("QuickClip321-quick.png")
    app.toggle_expanded()
    for page in [
        "Aktif",
        "Kuyruk",
        "Geçmiş",
        "Medya araçları",
        "Profiller",
        "Düzenleyici",
        "Ayarlar",
        "Hata kayıtları",
    ]:
        app._navigate(page)
        tick(0.05)
    app._navigate("Geçmiş")
    tick(0.2)
    screenshot("QuickClip321-full.png")
    results["all_pages_render"] = True
    with patch.object(app, "clipboard_get", side_effect=RuntimeError("clipboard locked")):
        app._read_clipboard()
    results["clipboard_failure_safe"] = True
    app.show_window()
    app._mouse_pressed = False
    outside = {"pressed": True, "inside": False, "owned": False, "on_tray": False}
    with patch.object(popup, "pointer_state", return_value=outside):
        app._dialog_open = True
        app._monitor_outside_click()
        assert app.visible
        app._dialog_open = False
        app._mouse_pressed = False
        app._monitor_outside_click()
        assert not app.visible
    results["outside_dismiss_and_dialog_guard"] = True
    app.clipboard_clear()
    if previous_clipboard:
        app.clipboard_append(previous_clipboard)
    app.update()
    app._exit_on_ui()
    app.mainloop()
    results["clean_shutdown"] = True
    (root / "ui-smoke-results.json").write_text(json.dumps(results, indent=2))
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    mp.freeze_support()
    run()
