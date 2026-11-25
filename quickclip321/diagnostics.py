"""Offline packaged launch test; never extracts or downloads remote media."""

import json
import ssl
import time
from pathlib import Path


def smoke_worker(payload, events, cancelled):
    events.put({"type": "progress", "status": "downloading", "stage": "Smoke test", "progress": 0.5})
    time.sleep(0.5)
    path = Path(payload["output_dir"]) / "offline-smoke.fixture"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"QuickClip321 offline test fixture")
    events.put({"type": "completed", "path": str(path), "title": "Offline smoke test"})


def packaged_smoke(report_path):
    from .main import configure_logging
    from .models import DownloadJob, JobStatus
    from .settings import SettingsStore, app_data_dir
    from .ui import QuickClipApp
    from . import popup
    from .worker import _find_binary
    from yt_dlp.version import __version__

    configure_logging()
    store = SettingsStore()
    settings = store.load()
    settings.clipboard_monitor = settings.notifications = False
    settings.output_dir = str(app_data_dir() / "smoke-media")
    store.save(settings)
    started = time.monotonic()
    app = QuickClipApp(start_hidden=True)
    app.engine.worker = smoke_worker
    report = {
        "startup_seconds": time.monotonic() - started,
        "yt_dlp": __version__,
        "binaries": {name: _find_binary(name) for name in ("ffmpeg", "ffprobe", "node", "deno")},
    }
    try:
        context = ssl.create_default_context()
        report["ssl_context_ready"] = True
        report["certificate_store"] = context.cert_store_stats()
    except Exception as error:
        report["error"] = f"TLS initialization failed: {error}"
    job = DownloadJob(url="https://example.com/offline-fixture", output_dir=settings.output_dir)
    app.engine.submit(job)

    def check():
        current = app.job_store.get(job.id)
        if current.status == JobStatus.COMPLETED:
            report["hidden_download_completed"] = not app.visible
            report["tray_created"] = app.tray_icon is not None
            app.show_window()
            app.update_idletasks()
            hwnd = popup.hwnd_for(app)
            user = popup._user()
            report["native_titlebar_absent"] = not bool(user.GetWindowLongW(hwnd, -16) & 0x00C00000)
            exstyle = user.GetWindowLongW(hwnd, -20)
            report["native_toolwindow_no_taskbar_style"] = bool(exstyle & 0x80) and not bool(exstyle & 0x40000)
            report["tray_anchor_source"] = app._popup_context["source"]
            report["popup_rect"] = vars(popup.rect_for(hwnd))
            report["icon_rect"] = vars(app._popup_context["anchor"])
            if not all(
                report[key]
                for key in (
                    "hidden_download_completed",
                    "tray_created",
                    "native_titlebar_absent",
                    "native_toolwindow_no_taskbar_style",
                )
            ):
                report["error"] = "GUI regression check failed"
            app.toggle_expanded()
            for page in ["Downloads", "Queue", "History", "Settings", "Diagnostics"]:
                app._navigate(page)
            report["all_pages_render"] = True
            Path(report_path).write_text(json.dumps(report, indent=2), encoding="utf-8")
            app._exit_on_ui()
        elif time.monotonic() - started > 15:
            report["error"] = current.to_dict()
            Path(report_path).write_text(json.dumps(report, indent=2), encoding="utf-8")
            app._exit_on_ui()
        else:
            app.after(100, check)

    app.after(100, check)
    app.mainloop()
    if "error" not in report:
        try:
            from .acceptance import destination_acceptance

            report["real_destination_checks"] = destination_acceptance(app_data_dir() / "destination-acceptance")
        except Exception as error:
            report["error"] = str(error)
    Path(report_path).write_text(json.dumps(report, indent=2), encoding="utf-8")
    return int("error" in report)
