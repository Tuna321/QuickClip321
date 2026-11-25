from __future__ import annotations

import io
import json
import logging
import multiprocessing as mp
import os
import queue
import sys
import threading
import time
import uuid
from pathlib import Path
from tkinter import Misc, TclError, filedialog, messagebox
import webbrowser

import customtkinter as ctk
from PIL import Image
import pystray
from plyer import notification

from . import __version__
from .assistant import AssistantError, suggest_local, suggest_remote
from .branding import brand_tile
from .credentials import CredentialError, CredentialStore
from .engine import DownloadEngine, terminate_tree
from .models import ACTIVE_STATUSES, DownloadJob, JobStatus
from .organizer import DEFAULT_DATABASE, ensure_database, load_database, resolve_destination
from . import popup
from .playlist import is_playlist_url
from .preview import fetch_preview
from .profiles import load_profiles
from .settings import APP_NAME, AppSettings, SettingsStore, app_data_dir
from .storage import JobStore
from .support import redact_support_text
from .validation import validate_clip_range, validate_media_url

PURPLE = "#8b5cf6"
PURPLE_HOVER = "#a47afa"
BG = ("#e9edf3", "#0c0d12")
PANEL = ("#ffffff", "#15161e")
TEXT_MUTED = ("#526071", "#a9adbf")
TEXT_PRIMARY = ("#17202e", "#ede8ff")
CONTROL_BG = ("#edf0f5", "#282a38")
CONTROL_BUTTON = ("#dce2eb", "#3e354f")
CONTROL_TEXT = ("#1b2636", "#f6f3ff")
ERROR_TEXT = ("#b42345", "#ff8e9c")

THEMES = {
    "Purple": ("#8b5cf6", "#a47afa"),
    "Graphite": ("#596579", "#718096"),
    "Ocean": ("#0e89aa", "#18a6c6"),
}


def load_brand_image() -> Image.Image:
    """Load the supplied mark on a theme-independent dark badge."""
    root = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent.parent))
    return brand_tile(root / "resources" / "QuickClip321Logo.png")


def parse_urls(value):
    """One URL per line; report invalid input before submitting any jobs."""
    rows = [part.strip() for part in value.splitlines() if part.strip()]
    if not rows:
        return [], "Enter a video or audio URL first."
    urls = []
    for row in rows:
        valid, url, error = validate_media_url(row)
        if not valid:
            return [], error
        urls.append(url)
    return list(dict.fromkeys(urls)), ""


class QuickClipApp(ctk.CTk):
    def __init__(self, *, start_hidden=False):
        popup.enable_dpi_awareness()
        super().__init__()
        self.withdraw()
        self.overrideredirect(True)
        self.attributes("-topmost", True)
        self.settings_store = SettingsStore()
        self.settings: AppSettings = self.settings_store.load()
        self.credentials = CredentialStore()
        ctk.set_appearance_mode(self.settings.theme_mode.lower())
        self.accent, self.accent_hover = THEMES[self.settings.color_theme]
        self.database_path = ensure_database(app_data_dir() / "organizer.json")
        self.profiles_path = app_data_dir() / "profiles.json"
        self.profiles = load_profiles(self.profiles_path)
        self.job_store = JobStore(app_data_dir() / "jobs.sqlite3")
        self.engine = DownloadEngine(
            self.job_store,
            self.settings.max_retries,
            self.settings.stall_timeout_seconds,
            self.settings.concurrent_downloads,
        )
        self.events = queue.Queue()
        self.engine.add_listener(lambda job: self.events.put(("job", job)))
        self.visible = False
        self.expanded = (self.settings.remembered_mode if self.settings.startup_mode == "remember" else self.settings.startup_mode) == "advanced"
        self.tray_icon = None
        self._last_clipboard = ""
        self._notified_jobs = set()
        self._job_widgets = {}
        self._render_cache = {}
        self._preview_process = self._preview_queue = None
        self._preview_after = None
        self._preview_url = ""
        self._thumbnail = None
        self._manual_destination = False
        self._manual_destination_path = ""
        self._preview_info = {}
        self._preview_result = {}
        self._preview_playlist_count = None
        self._dialog_open = False
        self._mouse_pressed = False
        self._popup_context = None
        self._closing = False
        self.logo_bitmap = load_brand_image()
        self.current_page = "Queue"
        self.title(f"{APP_NAME} {__version__}")
        self.geometry("600x365")
        self.resizable(False, False)
        self.configure(fg_color=BG)
        self.protocol("WM_DELETE_WINDOW", self.hide_window)
        self._build_ui()
        if self.expanded:
            self.full_container.pack(fill="both", expand=True, padx=10, pady=(0, 10))
            self.expand_btn.configure(text="Quick view")
            self._navigate(self.current_page)
            self._set_window_size(930, 720)
        self.bind("<Escape>", lambda _event: self.hide_window())
        self.bind("<Map>", self._on_map, add="+")
        self._last_popup_size = None
        self._position_after = None
        self.bind("<Configure>", self._on_popup_configure, add="+")
        self.bind("<FocusOut>", lambda _event: self.after(80, self._dismiss_on_focus_out), add="+")
        self._create_tray()
        if not start_hidden:
            self.after(150, self.show_window)
        self.after(120, self._monitor_outside_click)
        self.after(100, self._drain_events)
        self.after(300, self._monitor_clipboard)
        self.engine.start()

    def button(self, parent, text, command, **kwargs):
        return ctk.CTkButton(
            parent,
            text=text,
            command=command,
            height=30,
            fg_color=CONTROL_BG,
            hover_color=CONTROL_BUTTON,
            text_color=CONTROL_TEXT,
            corner_radius=8,
            **kwargs,
        )

    def _build_ui(self):
        self.root_panel = ctk.CTkFrame(self, fg_color=PANEL, corner_radius=16, border_width=1,
                                       border_color=CONTROL_BUTTON)
        self.root_panel.pack(fill="both", expand=True)
        header = ctk.CTkFrame(self.root_panel, fg_color="transparent")
        header.pack(fill="x", padx=20, pady=(14, 8))
        self.brand_image = ctk.CTkImage(
            light_image=self.logo_bitmap, dark_image=self.logo_bitmap, size=(36, 36)
        )
        ctk.CTkLabel(header, text="", image=self.brand_image, width=36, height=36).pack(side="left", padx=(0, 9))
        ctk.CTkLabel(header, text="QuickClip321", font=("Segoe UI Semibold", 23), text_color=TEXT_PRIMARY).pack(side="left")
        self.button(header, "Hide", self.hide_window, width=54).pack(side="right")
        self.expand_btn = self.button(header, "Advanced", self.toggle_expanded, width=106)
        self.expand_btn.pack(side="right", padx=8)
        ctk.CTkLabel(self.root_panel, text="Media link", anchor="w", text_color=TEXT_PRIMARY,
                     font=("Segoe UI Semibold", 12)).pack(fill="x", padx=20, pady=(0, 2))
        row = ctk.CTkFrame(self.root_panel, fg_color="transparent")
        row.pack(fill="x", padx=20, pady=(0, 8))
        self.url_var = ctk.StringVar()
        self.url_entry = ctk.CTkEntry(
            row,
            textvariable=self.url_var,
            height=42,
            placeholder_text="Paste a video or audio URL",
            border_color=("#b7c1d0", "#424458"),
            corner_radius=10,
        )
        self.url_entry.pack(side="left", fill="x", expand=True, padx=(0, 8))
        self.url_entry.bind("<Return>", lambda _: self.start_download())
        self.download_btn = ctk.CTkButton(
            row,
            text="Download",
            command=self.start_download,
            width=100,
            height=42,
            fg_color=self.accent,
            hover_color=self.accent_hover,
            font=("Segoe UI Semibold", 15),
            corner_radius=10,
        )
        self.download_btn.pack(side="right")
        self.button(row, "Paste", self._paste_media_url, width=58).pack(side="right", padx=(0, 8))
        self._help(self.url_entry, "Paste one URL here. Use Advanced for a batch of URLs.")
        controls = ctk.CTkFrame(self.root_panel, fg_color="transparent")
        controls.pack(fill="x", padx=20)
        self.profile_var = ctk.StringVar(value=self.settings.default_profile)
        self.format_var = ctk.StringVar(value=self.settings.default_format)
        self.quality_var = ctk.StringVar(value="best")
        def option_group(label):
            group = ctk.CTkFrame(controls, fg_color="transparent")
            group.pack(side="left", padx=(0, 26))
            ctk.CTkLabel(group, text=label, text_color=TEXT_MUTED, anchor="w",
                         font=("Segoe UI", 11)).pack(fill="x")
            return group

        self.profile_menu = ctk.CTkOptionMenu(
            option_group("Preset"),
            values=list(self.profiles),
            variable=self.profile_var,
            command=self._select_profile,
            width=150,
            fg_color=CONTROL_BG,
            button_color=CONTROL_BUTTON,
            text_color=CONTROL_TEXT,
        )
        self.profile_menu.pack(side="left", padx=(0, 22))
        self._help(self.profile_menu, "Choose a saved combination of format and quality.")
        self.quality_menu = None
        self.available_qualities = ["best"]
        for values, var, width in [
            (["mp4", "mp3", "wav", "m4a", "original"], self.format_var, 90),
            (["best"], self.quality_var, 90),
        ]:
            menu = ctk.CTkOptionMenu(
                option_group("Format" if var is self.format_var else "Resolution"),
                values=values, variable=var, width=width, fg_color=CONTROL_BG,
                button_color=CONTROL_BUTTON, text_color=CONTROL_TEXT
            )
            menu.pack(side="left", padx=(0, 22))
            self._help(menu, "Output format." if var is self.format_var else "Available resolutions from the current preview. Best uses the highest available quality.")
            if var is self.quality_var:
                self.quality_menu = menu

        self.preview_row = ctk.CTkFrame(self.root_panel, fg_color=BG, corner_radius=8)
        self.preview_row.pack(fill="x", padx=20, pady=(10, 0))
        self.thumbnail_label = ctk.CTkLabel(self.preview_row, text="▶", width=72, height=45, text_color=PURPLE)
        self.thumbnail_label.pack(side="left", padx=(0, 10))
        self.preview_title = ctk.CTkLabel(
            self.preview_row,
            text="A preview appears after you add a link.",
            anchor="w",
            wraplength=450,
            font=("Segoe UI", 12),
            text_color=TEXT_MUTED,
        )
        self.preview_title.pack(side="left", fill="x", expand=True)
        self.destination_row = ctk.CTkFrame(self.root_panel, fg_color="transparent")
        self.destination_row.pack(fill="x", padx=20, pady=(7, 0))
        ctk.CTkLabel(self.destination_row, text="Destination:", font=("Segoe UI", 11)).pack(side="left", padx=(0, 5))
        self.destination_var = ctk.StringVar()
        self.destination_entry = ctk.CTkEntry(
            self.destination_row, textvariable=self.destination_var, height=27, font=("Segoe UI", 10), state="readonly"
        )
        self.destination_entry.pack(side="left", fill="x", expand=True)
        self._help(self.destination_entry, "Predicted folder. Choose Change to override it for the next job.")
        self.button(self.destination_row, "Change", self.choose_folder, width=62).pack(side="left", padx=(6, 0))
        self.button(self.destination_row, "Auto", self._reset_destination, width=42).pack(side="left", padx=(4, 0))
        self.playlist_frame = ctk.CTkFrame(self.root_panel, fg_color="transparent")
        self.playlist_var = ctk.StringVar(value="single")
        self.batch_urls = ""
        self.clip_start_var = ctk.StringVar()
        self.clip_end_var = ctk.StringVar()
        self.subtitle_languages_var = ctk.StringVar()
        self.subtitle_format_var = ctk.StringVar(value="srt")
        self.subtitle_auto_var = ctk.BooleanVar(value=False)
        self.organizer_job_var = ctk.BooleanVar(value=self.settings.smart_organizer)
        self.auth_browser_var = ctk.StringVar(value="None")
        self.auth_cookie_file = ""
        ctk.CTkRadioButton(
            self.playlist_frame, text="Single video", variable=self.playlist_var, value="single", fg_color=self.accent
        ).pack(side="left", padx=(0, 20))
        ctk.CTkRadioButton(
            self.playlist_frame, text="Entire playlist", variable=self.playlist_var, value="all", fg_color=self.accent
        ).pack(side="left")
        self.status = ctk.CTkLabel(
            self.root_panel, text="Ready", anchor="w", text_color=TEXT_MUTED, font=("Segoe UI", 11)
        )
        self.status.pack(fill="x", padx=20, pady=(8, 3))
        self.quick_progress = ctk.CTkProgressBar(self.root_panel, height=4, progress_color=self.accent,
                                                fg_color=("#d7deea", "#282a38"))
        self.quick_progress.pack(fill="x", padx=20, pady=(0, 12))
        self.quick_progress.set(0)
        self.full_container = ctk.CTkFrame(self.root_panel, fg_color=BG)
        self.sidebar = ctk.CTkFrame(self.full_container, fg_color="transparent", width=130)
        self.sidebar.pack(side="left", fill="y", padx=10, pady=12)
        ctk.CTkLabel(self.sidebar, text="WORKSPACE", font=("Segoe UI Semibold", 10),
                     text_color=TEXT_MUTED, anchor="w").pack(fill="x", padx=8, pady=(1, 10))
        self.nav_buttons = {}
        for label in [
            "Downloads",
            "Queue",
            "History",
            "Settings",
            "Diagnostics",
        ]:
            button = ctk.CTkButton(self.sidebar, text=label, command=lambda page=label: self._navigate(page),
                                   width=126, height=35, anchor="w", corner_radius=8,
                                   fg_color=self.accent if label == self.current_page else "transparent",
                                   hover_color=("#dfe5ee", "#282a38"),
                                   text_color="#ffffff" if label == self.current_page else TEXT_MUTED)
            button.pack(fill="x", pady=2)
            self.nav_buttons[label] = button
        self.page_container = ctk.CTkFrame(self.full_container, fg_color="transparent")
        self.page_container.pack(side="left", fill="both", expand=True, padx=(0, 12), pady=12)
        self.url_var.trace_add("write", self._url_changed)
        self.format_var.trace_add("write", lambda *_: self._update_destination())
        self._update_destination()

    def _help(self, widget, explanation):
        """Show a tip only after the pointer rests on the info mark."""
        icon = ctk.CTkLabel(widget.master, text="ⓘ", width=20, height=20,
                            text_color=self.accent, cursor="hand2", font=("Segoe UI", 14))

        def position_icon():
            if not widget.winfo_exists() or not icon.winfo_exists():
                return
            inside = isinstance(widget, (ctk.CTkEntry, ctk.CTkTextbox))
            x = widget.winfo_x() + widget.winfo_width() - (22 if inside else -4)
            y = widget.winfo_y() + max(0, (widget.winfo_height() - 20) // 2)
            icon.place(x=x, y=y)

        widget.after_idle(position_icon)
        Misc.bind(widget, "<Configure>", lambda _event: position_icon(), add="+")
        Misc.bind(widget.master, "<Configure>", lambda _event: widget.after_idle(position_icon), add="+")
        icon.bind("<Enter>", lambda _event: self._schedule_help(icon, explanation), add="+")
        icon.bind("<Leave>", lambda _event: self._hide_help(), add="+")
        return icon

    def _schedule_help(self, icon, explanation):
        self._hide_help()
        self._help_after = self.after(500, lambda: self._show_help(icon, explanation))

    def _show_help(self, icon, explanation):
        self._help_after = None
        if not icon.winfo_exists() or not icon.winfo_ismapped():
            return
        self._hide_help()
        tip = ctk.CTkToplevel(self)
        tip.withdraw()
        tip.overrideredirect(True)
        tip.attributes("-topmost", True)
        ctk.CTkLabel(tip, text=explanation, wraplength=300, justify="left",
                     fg_color=PANEL, corner_radius=8).pack(padx=10, pady=8)
        tip.update_idletasks()
        x = min(icon.winfo_rootx(), max(0, self.winfo_screenwidth() - tip.winfo_reqwidth() - 8))
        y = icon.winfo_rooty() + icon.winfo_height() + 4
        if y + tip.winfo_reqheight() > self.winfo_screenheight():
            y = icon.winfo_rooty() - tip.winfo_reqheight() - 4
        tip.geometry(f"+{x}+{max(0, y)}")
        tip.deiconify()
        self._help_window = tip

    def _hide_help(self):
        pending = getattr(self, "_help_after", None)
        if pending is not None:
            self.after_cancel(pending)
            self._help_after = None
        tip = getattr(self, "_help_window", None)
        if tip and tip.winfo_exists():
            tip.destroy()
        self._help_window = None

    def _set_window_size(self, width, height):
        self.update_idletasks()
        screen_width, screen_height = self.winfo_screenwidth(), self.winfo_screenheight()
        self.geometry(f"{min(width, max(420, screen_width - 24))}x{min(height, max(300, screen_height - 80))}")

    def _on_map(self, event):
        if event.widget is self:
            self.after_idle(self._hide_from_taskbar)

    def _on_popup_configure(self, event):
        if event.widget is self and self.visible and not self._closing:
            size = (event.width, event.height)
            if size != self._last_popup_size:
                self._last_popup_size = size
                if self._position_after:
                    self.after_cancel(self._position_after)
                self._position_after = self.after(100, self._place_bottom_right)

    def _hide_from_taskbar(self):
        if not self._closing:
            popup.popup_styles(self)

    def _place_bottom_right(self, activate=False):
        self.update_idletasks()
        if os.name == "nt":
            try:
                self._popup_context = popup.position_popup(
                    self, self.tray_icon, activate, self.settings.popup_position,
                    (self.settings.popup_x_ratio, self.settings.popup_y_ratio),
                )
                return
            except OSError:
                logging.exception("Native tray anchoring unavailable")
        self.geometry(
            f"+{max(0, self.winfo_screenwidth() - self.winfo_width() - 22)}+{max(0, self.winfo_screenheight() - self.winfo_height() - 76)}"
        )

    def _monitor_outside_click(self):
        if self._closing:
            return
        if self.visible and os.name == "nt":
            try:
                state = popup.pointer_state(self, self.tray_icon)
                if popup.should_dismiss(state, self._mouse_pressed, self._dialog_open):
                    self.hide_window()
                self._mouse_pressed = state["pressed"]
            except OSError:
                logging.debug("Pointer state unavailable", exc_info=True)
        else:
            self._mouse_pressed = False
        self.after(100, self._monitor_outside_click)

    def _dismiss_on_focus_out(self):
        if self.visible and not self._closing and not self._dialog_open and os.name == "nt":
            try:
                if not popup.foreground_owned():
                    state = popup.pointer_state(self, self.tray_icon)
                    if not (state["inside"] or state["owned"] or state["on_tray"]):
                        self.hide_window()
            except OSError:
                logging.debug("Foreground state unavailable", exc_info=True)

    def _resize_quick(self):
        if not self.expanded:
            self._set_window_size(600, 395 if self.playlist_frame.winfo_manager() else 365)
            if self.visible:
                self.after_idle(self._place_bottom_right)

    def _destination_payload(self):
        return {
            "output_format": self.format_var.get(),
            "output_dir": self.settings.output_dir,
            "organizer_enabled": self.settings.smart_organizer,
            "manual_destination": self._manual_destination,
            "selected_destination": self._manual_destination_path,
        }

    def _update_destination(self):
        choice = resolve_destination(self._preview_info, self._destination_payload(), load_database(self.database_path))
        self._selected_destination = str(choice.destination)
        self.destination_var.set(self._selected_destination)
        self.destination_entry.xview_moveto(1)
        return choice

    def _url_changed(self, *_):
        self._preview_info = {}
        self._preview_result = {}
        self._preview_playlist_count = None
        self.available_qualities = ["best"]
        self.quality_menu.configure(values=self.available_qualities)
        self.quality_var.set("best")
        self._update_destination()
        self.playlist_var.set("single")
        self.preview_title.configure(
            text="Loading preview…" if self.url_var.get().strip() else "A preview appears after you add a link."
        )
        self.thumbnail_label.configure(image=None, text="▶")
        self._thumbnail = None
        self._show_playlist(is_playlist_url(self.url_var.get()))
        if self._preview_after:
            self.after_cancel(self._preview_after)
        self._preview_after = self.after(800, self._start_preview)

    def _show_playlist(self, visible):
        if visible:
            self.playlist_frame.pack(fill="x", padx=20, pady=(8, 0), before=self.status)
        else:
            self.playlist_frame.pack_forget()
        self._resize_quick()

    def _start_preview(self):
        self._preview_after = None
        url = self.url_var.get().strip()
        if not self.visible or not validate_media_url(url)[0]:
            return
        self._stop_preview()
        context = mp.get_context("spawn")
        self._preview_queue = context.Queue()
        self._preview_url = url
        self._preview_process = context.Process(target=fetch_preview, args=(url, self._preview_queue))
        self._preview_process.start()
        self._preview_started = time.monotonic()

    def _stop_preview(self):
        if self._preview_process:
            terminate_tree(self._preview_process)
            self._preview_process.join(timeout=1)
            self._preview_process.close()
            self._preview_process = None
        if self._preview_queue:
            self._preview_queue.close()
            self._preview_queue = None

    def _poll_preview(self):
        if not self._preview_process:
            return
        try:
            result = self._preview_queue.get_nowait()
        except queue.Empty:
            if time.monotonic() - self._preview_started > 25:
                self._stop_preview()
                if self.url_var.get().strip() == self._preview_url:
                    self.preview_title.configure(text="Preview unavailable. You can still download with Best quality.")
            return
        url = self._preview_url
        self._stop_preview()
        if self.url_var.get().strip() != url:
            return
        if result.get("error"):
            self.preview_title.configure(text="Preview unavailable. You can still download with Best quality.")
            return
        title = result.get("title") or "Untitled media"
        qualities = [str(q) for q in result.get("qualities", []) if str(q) in {"2160", "1440", "1080", "720", "480"}]
        self.available_qualities = ["best", *dict.fromkeys(qualities)]
        self.quality_menu.configure(values=self.available_qualities)
        if self.quality_var.get() not in {"best", *qualities}:
            self.quality_var.set("best")
        if result.get("playlist") or is_playlist_url(url):
            self._show_playlist(True)
            self._preview_playlist_count = result.get("count")
            title += f"  •  {result.get('count') or 'count unknown'} items"
        self.preview_title.configure(text=title[:180])
        self._preview_info = result.get("metadata") or {"title": result.get("title", "")}
        self._preview_result = {**result, "metadata": self._preview_info}
        self._update_destination()
        if result.get("image"):
            try:
                image = Image.open(io.BytesIO(result["image"]))
                image.thumbnail((240, 140))
                self._thumbnail = ctk.CTkImage(light_image=image.copy(), dark_image=image.copy(), size=(72, 45))
                self.thumbnail_label.configure(image=self._thumbnail, text="")
            except Exception:
                pass

    def _select_profile(self, name):
        profile = self.profiles[name]
        self.format_var.set(profile["format"])
        self.quality_var.set(profile["quality"] if profile["quality"] in self.available_qualities else "best")

    def _paste_media_url(self):
        try:
            raw = self.clipboard_get()
        except TclError:
            raw = ""
        valid, url, _ = validate_media_url(raw.strip())
        if valid:
            self.url_var.set(url)
            self.status.configure(text="Link pasted.", text_color=TEXT_MUTED)
        else:
            self.status.configure(text="The clipboard does not contain a media URL.", text_color=ERROR_TEXT)

    def start_download(self, *, use_batch=False):
        value = self.batch_entry.get("1.0", "end").strip() if use_batch else self.url_var.get()
        urls, error = parse_urls(value)
        if error:
            self.status.configure(text=error, text_color=ERROR_TEXT)
            return
        if self.auth_cookie_file and self.auth_browser_var.get() != "None":
            self.status.configure(text="Choose either a browser session or a cookie file.", text_color=ERROR_TEXT)
            return
        try:
            clip_start, clip_end = validate_clip_range(self.clip_start_var.get(), self.clip_end_var.get())
        except ValueError as exc:
            self.status.configure(text=str(exc), text_color=ERROR_TEXT)
            return
        playlists = [url for url in urls if is_playlist_url(url)]
        if playlists and self.playlist_var.get() == "all":
            count = self._preview_playlist_count if len(urls) == 1 else None
            detail = f" ({count} items)" if count else " (item count unavailable)"
            if not messagebox.askyesno("Download playlist", f"Add {len(playlists)} entire playlist(s){detail} to the queue?", parent=self):
                return
        self._update_destination()
        batch_id = uuid.uuid4().hex if len(urls) > 1 else ""
        submitted = 0
        for url in urls:
            job = DownloadJob(
                url=url, output_format=self.format_var.get(), quality=self.quality_var.get(),
                output_dir=self.settings.output_dir,
                playlist_mode=self.playlist_var.get() if is_playlist_url(url) else "single",
                profile=self.profile_var.get(), organizer_enabled=self.organizer_job_var.get(),
                manual_destination=self._manual_destination, selected_destination=self._selected_destination,
                destination=self._selected_destination, organizer_database=str(self.database_path),
                fragment_concurrency=self.settings.fragment_concurrency, partial_policy=self.settings.partial_policy,
                clip_start_seconds=clip_start, clip_end_seconds=clip_end,
                subtitle_languages=[language.strip() for language in self.subtitle_languages_var.get().split(",") if language.strip()],
                subtitle_format=self.subtitle_format_var.get(), subtitle_auto=self.subtitle_auto_var.get(),
                auth_browser="" if self.auth_browser_var.get() == "None" else self.auth_browser_var.get().lower(),
                auth_cookie_file=self.auth_cookie_file, batch_id=batch_id,
            )
            submitted += bool(self.engine.submit(job))
        if not submitted:
            self.status.configure(text="These downloads are already in the queue.", text_color=TEXT_MUTED)
            return
        self.settings.default_format, self.settings.default_quality = self.format_var.get(), self.quality_var.get()
        self.settings.default_profile = self.profile_var.get()
        self.settings_store.save(self.settings)
        if use_batch:
            self.batch_urls = ""
            self.batch_entry.delete("1.0", "end")
        else:
            self.url_var.set("")
        self._manual_destination = False
        self._manual_destination_path = ""
        self.organizer_job_var.set(self.settings.smart_organizer)
        self._update_destination()
        self.status.configure(text=f"Added {submitted} download(s) to the queue.", text_color=TEXT_MUTED)

    def _drain_events(self):
        if self._closing:
            return
        latest = {}
        for _ in range(500):
            try:
                kind, value = self.events.get_nowait()
            except queue.Empty:
                break
            if kind == "job":
                latest[value.id] = value
            elif kind == "toggle":
                self.hide_window() if self.visible else self.show_window()
            elif kind == "exit":
                self._exit_on_ui()
                return
            elif kind == "folder":
                self.open_folder()
            elif kind == "tool":
                self.status.configure(text=value[:130])
            elif kind == "assistant":
                self._show_suggestion(value)
        for job in latest.values():
            self._handle_job_event(job)
        self._poll_preview()
        self.after(200, self._drain_events)

    def _handle_job_event(self, job):
        self.status.configure(text=(job.stage + (f"   {job.speed}" if job.speed else ""))[:120])
        self.quick_progress.set(job.progress)
        if job.destination and not self.url_var.get().strip():
            self.destination_var.set(job.destination)
            self.destination_entry.xview_moveto(1)
        notification_key = (job.id, job.status, job.updated_at)
        if self.settings.notifications and notification_key not in self._notified_jobs:
            if job.status in {JobStatus.COMPLETED, JobStatus.FAILED}:
                self._notify(
                    "Download completed" if job.status == JobStatus.COMPLETED else "Download failed",
                    job.title if job.status == JobStatus.COMPLETED else job.error_message,
                )
                self._notified_jobs.add(notification_key)
        if self.visible and self.expanded and self.current_page in {"Queue", "History"}:
            in_history = job.status in {JobStatus.COMPLETED, JobStatus.FAILED, JobStatus.CANCELLED}
            if (self.current_page == "History") != in_history:
                self._navigate(self.current_page)
            else:
                self._render_job(job)

    def _notify(self, title, message):
        def send():
            try:
                notification.notify(
                    title=title, message=(message or "QuickClip321")[:200], app_name=APP_NAME, timeout=4
                )
            except Exception:
                logging.exception("Notification failed")

        threading.Thread(target=send, daemon=True).start()

    def toggle_expanded(self):
        self.expanded = not self.expanded
        if self.expanded:
            self._set_window_size(930, 720)
            self.full_container.pack(fill="both", expand=True, padx=10, pady=(0, 10))
            self.expand_btn.configure(text="Quick view")
            self._navigate(self.current_page)
        else:
            self.full_container.pack_forget()
            self._resize_quick()
            self.expand_btn.configure(text="Advanced")
        self.settings.remembered_mode = "advanced" if self.expanded else "quick"
        self.settings_store.save(self.settings)
        self._place_bottom_right()

    def _navigate(self, page):
        self._hide_help()
        if getattr(self, "batch_entry", None) and self.batch_entry.winfo_exists():
            self.batch_urls = self.batch_entry.get("1.0", "end").strip()
        self.batch_entry = None
        self.current_page = page
        for label, button in self.nav_buttons.items():
            button.configure(fg_color=self.accent if label == page else "transparent",
                             text_color="#ffffff" if label == page else TEXT_MUTED)
        for child in self.page_container.winfo_children():
            child.destroy()
        self._job_widgets.clear()
        self._render_cache.clear()
        ctk.CTkLabel(self.page_container, text=page, font=("Segoe UI Semibold", 20), anchor="w").pack(
            fill="x", pady=(0, 10)
        )
        if page == "Downloads":
            self._build_download_options()
        elif page in {"Queue", "History"}:
            toolbar = ctk.CTkFrame(self.page_container, fg_color="transparent")
            toolbar.pack(fill="x", pady=(0, 8))
            if page == "Queue":
                self.button(toolbar, "Cancel all", self.engine.cancel_all, width=112).pack(side="left", padx=(0, 6))
                self.button(toolbar, "Clear pending", self.engine.clear_pending, width=142).pack(side="left")
            else:
                self.button(toolbar, "Clear completed", self._clear_completed, width=162).pack(side="left")
            self.queue_container = ctk.CTkScrollableFrame(self.page_container, fg_color="transparent")
            self.queue_container.pack(fill="both", expand=True)
            shown = 0
            for job in reversed(self.job_store.list(300)):
                in_history = job.status in {JobStatus.COMPLETED, JobStatus.FAILED, JobStatus.CANCELLED}
                if (page == "History") != in_history:
                    continue
                self._render_job(job)
                shown += 1
            if not shown:
                ctk.CTkLabel(self.queue_container,
                             text="Your queue is clear" if page == "Queue" else "No downloads in history yet",
                             font=("Segoe UI Semibold", 18), text_color=TEXT_PRIMARY).pack(pady=(105, 5))
                ctk.CTkLabel(self.queue_container,
                             text="Paste a link above to get started." if page == "Queue" else "Completed and failed downloads appear here.",
                             font=("Segoe UI", 12), text_color=TEXT_MUTED).pack()
        elif page == "Settings":
            self._build_settings()
        elif page == "Diagnostics":
            toolbar = ctk.CTkFrame(self.page_container, fg_color="transparent")
            toolbar.pack(fill="x", pady=(0, 8))
            for label, command, explanation in (
                ("Open log folder", lambda: self._open_path(app_data_dir() / "logs"), "Open the local folder containing complete application logs."),
                ("Copy safe report", self._copy_support_report, "Copy diagnostics with URLs and local paths removed."),
                ("Export safe report", self._export_support_report, "Save a redacted text report where you choose."),
            ):
                button = self.button(toolbar, label, command, width=135)
                button.pack(side="left", padx=(0, 20))
                self._help(button, explanation)
            ctk.CTkLabel(self.page_container, text="Copy and export remove URLs and local paths. Logs are read only when this page opens.",
                         text_color=TEXT_MUTED, anchor="w", wraplength=650).pack(fill="x", pady=(0, 6))
            text = ctk.CTkTextbox(self.page_container, wrap="word", font=("Consolas", 11))
            text.pack(fill="both", expand=True)
            log_path = app_data_dir() / "logs" / "quickclip321.log"
            try:
                self._diagnostics_raw = log_path.read_text(encoding="utf-8", errors="replace")[-100_000:]
            except FileNotFoundError:
                self._diagnostics_raw = "No log entries yet."
            except OSError as error:
                self._diagnostics_raw = f"Log unavailable: {type(error).__name__}"
            self._diagnostics_raw += "\n\nRecent failed jobs:\n"
            for job in self.job_store.list(100):
                if job.error_code:
                    self._diagnostics_raw += f"\n{job.title or job.url}\n{job.error_code}: {job.error_message}\n{job.error_detail}\n"
            text.insert("end", self._diagnostics_raw)
            text.configure(state="disabled")
            inspect = self.button(self.page_container, "Inspect local media", self._inspect_media)
            inspect.pack(anchor="w", pady=10)
            self._help(inspect, "Check an existing file with the bundled FFprobe without uploading it.")

    def _copy_support_report(self):
        self.clipboard_clear()
        self.clipboard_append(redact_support_text(getattr(self, "_diagnostics_raw", "")))
        self.status.configure(text="Safe diagnostic report copied.", text_color=TEXT_MUTED)

    def _export_support_report(self):
        self._dialog_open = True
        try:
            path = filedialog.asksaveasfilename(parent=self, title="Export safe diagnostic report",
                                                defaultextension=".txt", filetypes=[("Text files", "*.txt")])
        finally:
            self._dialog_open = False
        if not path:
            return
        try:
            Path(path).write_text(redact_support_text(getattr(self, "_diagnostics_raw", "")), encoding="utf-8")
        except OSError as error:
            self.status.configure(text=f"Could not export diagnostics: {error}", text_color=ERROR_TEXT)
            return
        self.status.configure(text="Safe diagnostic report exported.", text_color=TEXT_MUTED)

    def _build_download_options(self):
        scroll = ctk.CTkScrollableFrame(self.page_container, fg_color="transparent")
        scroll.pack(fill="both", expand=True)
        ctk.CTkLabel(scroll, text="Batch URLs", font=("Segoe UI Semibold", 13)).pack(anchor="w")
        ctk.CTkLabel(scroll, text="Enter one link per line.",
                     text_color=TEXT_MUTED).pack(anchor="w", pady=(0, 3))
        self.batch_entry = ctk.CTkTextbox(scroll, height=90)
        self.batch_entry.pack(fill="x", pady=(2, 8))
        self.batch_entry.insert("1.0", self.batch_urls)
        self._help(self.batch_entry, "Paste multiple URLs, one per line. Duplicates are ignored.")
        self.button(scroll, "Add batch to queue", lambda: self.start_download(use_batch=True),
                    width=145).pack(anchor="w", pady=(0, 6))
        row = ctk.CTkFrame(scroll, fg_color="transparent")
        row.pack(fill="x", pady=5)
        for label, var in (("Clip start (seconds)", self.clip_start_var), ("Clip end (seconds)", self.clip_end_var)):
            group = ctk.CTkFrame(row, fg_color="transparent")
            group.pack(side="left", padx=(0, 12))
            ctk.CTkLabel(group, text=label, text_color=TEXT_MUTED).pack(anchor="w")
            field = ctk.CTkEntry(group, textvariable=var, placeholder_text="Optional", width=180)
            field.pack(anchor="w")
            self._help(field, "Leave empty for the full video. End must be after start.")
        row = ctk.CTkFrame(scroll, fg_color="transparent")
        row.pack(fill="x", pady=5)
        language_group = ctk.CTkFrame(row, fg_color="transparent")
        language_group.pack(side="left", padx=(0, 12))
        ctk.CTkLabel(language_group, text="Subtitle languages", text_color=TEXT_MUTED).pack(anchor="w")
        languages = ctk.CTkEntry(language_group, textvariable=self.subtitle_languages_var,
                                 placeholder_text="e.g. en, es", width=270)
        languages.pack(anchor="w")
        self._help(languages, "Comma separated language codes. Leave empty to skip subtitles.")
        format_group = ctk.CTkFrame(row, fg_color="transparent")
        format_group.pack(side="left")
        ctk.CTkLabel(format_group, text="Subtitle file", text_color=TEXT_MUTED).pack(anchor="w")
        formats = ctk.CTkOptionMenu(format_group, values=["srt", "vtt"], variable=self.subtitle_format_var)
        formats.pack(anchor="w")
        self._help(formats, "Subtitle file format.")
        auto = ctk.CTkSwitch(scroll, text="Allow automatic subtitles", variable=self.subtitle_auto_var, progress_color=self.accent)
        auto.pack(anchor="w", pady=5)
        self._help(auto, "Use automatically generated subtitles when manually authored ones are unavailable.")
        organizer = ctk.CTkSwitch(scroll, text="Organize this job automatically", variable=self.organizer_job_var, progress_color=self.accent)
        organizer.pack(anchor="w", pady=5)
        self._help(organizer, "Override the default organizer setting for the next job only.")
        ctk.CTkLabel(scroll, text="Authenticated download (explicit opt-in)").pack(anchor="w", pady=(8, 2))
        browser = ctk.CTkOptionMenu(scroll, values=["None", "Chrome", "Edge", "Firefox", "Brave"], variable=self.auth_browser_var)
        browser.pack(anchor="w")
        self._help(browser, "Only the browser you select will be used for this download. No browser session is read by default.")
        cookie = self.button(scroll, "Choose cookies.txt file", self._choose_cookie_file)
        cookie.pack(anchor="w", pady=6)
        self._help(cookie, "Use a cookies.txt file you explicitly select. Browser and file cannot both be selected.")
        self.cookie_label = ctk.CTkLabel(scroll, text=Path(self.auth_cookie_file).name if self.auth_cookie_file else "No cookie file selected")
        self.cookie_label.pack(anchor="w")
        self.button(scroll, "Save current format as profile", self._prompt_save_profile).pack(anchor="w", pady=8)
        self.button(scroll, "Local suggestion", self._local_suggestion).pack(anchor="w", pady=4)
        ai = self.button(scroll, "Ask AI", self._ask_ai)
        ai.pack(anchor="w", pady=4)
        self._help(ai, "Sends only displayed title, duration, channel and available formats to your selected provider when clicked.")
        self.button(scroll, "Add to queue", self.start_download).pack(anchor="w", pady=8)

    def _build_settings(self):
        scroll = ctk.CTkScrollableFrame(self.page_container, fg_color="transparent")
        scroll.pack(fill="both", expand=True)
        for attr, label, explanation in (
            ("clipboard_monitor", "Monitor clipboard", "Check the clipboard on show and every two seconds in the background."),
            ("notifications", "Desktop notifications", "Notify when a download completes or fails."),
            ("smart_organizer", "Automatic organizer", "Choose Video, Audio or Unsorted folders using local rules."),
        ):
            var = ctk.BooleanVar(value=getattr(self.settings, attr))
            control = ctk.CTkSwitch(scroll, text=label, variable=var, progress_color=self.accent,
                                    command=lambda a=attr, v=var: self._save_setting(a, v.get()))
            control.pack(anchor="w", pady=5)
            self._help(control, explanation)
        for label, attr, values, help_text in (
            ("Startup view", "startup_mode", ["quick", "advanced", "remember"], "Remember opens the last used view."),
            ("Appearance", "theme_mode", ["System", "Light", "Dark"], "Follow the system or choose a fixed light/dark mode."),
            ("Accent", "color_theme", ["Purple", "Graphite", "Ocean"], "Changes the accent color. Reopen the app to update every control."),
            ("Popup position", "popup_position", ["Tray", "Corner", "Custom"], "Tray follows its icon; Corner uses the tray monitor; Custom uses saved relative coordinates."),
            ("Concurrent downloads (restart required)", "concurrent_downloads", ["1", "2", "3"], "Maximum simultaneous downloads after restart."),
            ("Parallel fragments", "fragment_concurrency", ["1", "2", "4", "8"], "Parallel fragments per download; more may increase network load."),
            ("Partial files after cancel", "partial_policy", ["keep", "delete"], "Keep allows a later retry; delete removes incomplete output."),
        ):
            self._setting_menu(label, attr, values, scroll, help_text)
        location = ctk.CTkFrame(scroll, fg_color="transparent")
        location.pack(fill="x", pady=6)
        ctk.CTkLabel(location, text="Custom X % / Y %").pack(side="left")
        self.popup_x_entry = ctk.CTkEntry(location, width=58)
        self.popup_x_entry.insert(0, str(round(self.settings.popup_x_ratio * 100)))
        self.popup_x_entry.pack(side="left", padx=6)
        self.popup_y_entry = ctk.CTkEntry(location, width=58)
        self.popup_y_entry.insert(0, str(round(self.settings.popup_y_ratio * 100)))
        self.popup_y_entry.pack(side="left", padx=6)
        self.button(location, "Save position", self._save_custom_position, width=105).pack(side="left")
        self._help(self.popup_x_entry, "Horizontal position within the tray monitor's usable area, 0 to 100 percent.")
        self._help(self.popup_y_entry, "Vertical position within the tray monitor's usable area, 0 to 100 percent.")
        self.button(scroll, "Choose default download folder", lambda: self.choose_folder(for_default=True)).pack(anchor="w", pady=5)
        ctk.CTkLabel(scroll, text=self.settings.output_dir, wraplength=570).pack(anchor="w")
        self._build_organizer_editor(scroll)
        ctk.CTkLabel(scroll, text="Optional AI suggestions", font=("Segoe UI Semibold", 15)).pack(anchor="w", pady=(12, 5))
        self._setting_menu("Provider", "ai_provider", ["Off", "openai", "ollama"], scroll,
                           "Off by default. OpenAI uses your API key; Ollama runs on this computer.")
        ctk.CTkLabel(scroll, text="Model name").pack(anchor="w", pady=(5, 2))
        self.ai_model_entry = ctk.CTkEntry(scroll, width=280)
        self.ai_model_entry.insert(0, self.settings.ai_model)
        self.ai_model_entry.pack(anchor="w")
        self._help(self.ai_model_entry, "Enter the exact model name for your selected provider. No suggestion runs automatically.")
        self.button(scroll, "Save model", self._save_ai_model).pack(anchor="w", pady=4)
        self.button(scroll, "Save OpenAI API key…", self._save_ai_key).pack(anchor="w", pady=4)
        self.button(scroll, "Clear saved API key", self._clear_ai_key).pack(anchor="w", pady=4)
        self.button(scroll, "Reset all settings and organizer rules", self._reset_defaults).pack(anchor="w", pady=10)

    def _save_ai_model(self):
        self.settings.ai_model = self.ai_model_entry.get().strip()[:100]
        self.settings_store.save(self.settings)
        self.status.configure(text="AI model saved.")

    def _save_ai_key(self):
        dialog = ctk.CTkToplevel(self)
        dialog.title("Save OpenAI API key")
        dialog.geometry("390x150")
        dialog.resizable(False, False)
        dialog.transient(self)
        dialog.grab_set()
        ctk.CTkLabel(dialog, text="OpenAI API key").pack(anchor="w", padx=20, pady=(16, 6))
        field = ctk.CTkEntry(dialog, width=350, show="*")
        field.pack(padx=20)
        field.focus_set()

        def save():
            value = field.get().strip()
            if not value:
                dialog.destroy()
                return
            try:
                self.credentials.save(value)
                self.status.configure(text="API key saved for this Windows user.")
                dialog.destroy()
            except (CredentialError, ValueError) as exc:
                self.status.configure(text=str(exc), text_color=ERROR_TEXT)
        self.button(dialog, "Save", save, width=75).pack(side="left", padx=20, pady=12)
        self.button(dialog, "Cancel", dialog.destroy, width=75).pack(side="left", pady=12)
        field.bind("<Return>", lambda _event: save())

    def _clear_ai_key(self):
        self.credentials.clear()
        self.status.configure(text="Saved API key removed.")

    def _local_suggestion(self):
        suggestion = suggest_local(self._preview_result, self.format_var.get(), self.quality_var.get(),
                                   self.settings.output_dir, load_database(self.database_path), self.organizer_job_var.get())
        self._show_suggestion(suggestion)

    def _ask_ai(self):
        if self.settings.ai_provider == "Off":
            self.status.configure(text="Choose an AI provider in Settings first.")
            return
        metadata = dict(self._preview_result)
        shown = f"Title: {metadata.get('title') or '(none)'}\nDuration: {metadata.get('duration') or '(unknown)'}\nChannel: {(metadata.get('metadata') or {}).get('channel') or '(unknown)'}\nAvailable formats: {', '.join(map(str, metadata.get('qualities') or [])) or '(unknown)'}"
        if not messagebox.askyesno("Ask AI", "Send only these media details to the selected provider?\n\n" + shown, parent=self):
            return
        try:
            key = self.credentials.load() if self.settings.ai_provider == "openai" else ""
        except CredentialError as exc:
            self.status.configure(text=str(exc), text_color=ERROR_TEXT)
            return
        provider, model = self.settings.ai_provider, self.settings.ai_model
        def run():
            try:
                self.events.put(("assistant", suggest_remote(provider, metadata, model, api_key=key)))
            except AssistantError as exc:
                self.events.put(("tool", str(exc)))
        threading.Thread(target=run, daemon=True).start()
        self.status.configure(text="Waiting for AI suggestion…")

    def _show_suggestion(self, suggestion):
        if suggestion:
            accepted = messagebox.askyesno("Download suggestion",
                f"Format: {suggestion.get('format')}\nQuality: {suggestion.get('quality')}\n"
                f"Name: {suggestion.get('filename')}\nCategory: {suggestion.get('category')}\n"
                f"Reason: {suggestion.get('reason')}\n\nApply the suggested format and quality?",
                parent=self)
            if accepted:
                self.format_var.set(suggestion.get("format", self.format_var.get()))
                quality = suggestion.get("quality", "best")
                self.quality_var.set(quality if quality in self.available_qualities else "best")

    def _build_organizer_editor(self, parent):
        ctk.CTkLabel(parent, text="Organizer folders and title rules", font=("Segoe UI Semibold", 15)).pack(anchor="w", pady=(12, 5))
        ctk.CTkLabel(parent, text="Rules run before automatic media classification. Existing files are never scanned.", wraplength=570).pack(anchor="w")
        database = load_database(self.database_path)
        for category, label in (("video", "Video"), ("audio", "Audio"), ("unsorted", "Unsorted")):
            row = ctk.CTkFrame(parent, fg_color="transparent")
            row.pack(fill="x", pady=3)
            ctk.CTkLabel(row, text=label, width=75).pack(side="left")
            entry = ctk.CTkEntry(row, width=390)
            entry.insert(0, database.get("destinations", {}).get(category, ""))
            entry.pack(side="left", padx=5)
            self.button(row, "Save", lambda c=category, e=entry: self._save_organizer_path(c, e.get()), width=55).pack(side="left")
            self._help(entry, f"Destination for {label} files. Use an absolute folder path.")
        row = ctk.CTkFrame(parent, fg_color="transparent")
        row.pack(fill="x", pady=5)
        self.rule_keyword = ctk.CTkEntry(row, placeholder_text="Title contains…", width=230)
        self.rule_keyword.pack(side="left", padx=(0, 6))
        self.rule_category = ctk.StringVar(value="video")
        category_menu = ctk.CTkOptionMenu(row, values=["video", "audio", "unsorted"], variable=self.rule_category, width=115)
        category_menu.pack(side="left", padx=(0, 6))
        self.button(row, "Add rule", self._add_organizer_rule, width=80).pack(side="left")
        self._help(self.rule_keyword, "Case-insensitive title phrase to route into the selected category.")
        self._help(category_menu, "Folder used when the title contains the phrase.")
        for index, rule in enumerate(database.get("rules", [])):
            row = ctk.CTkFrame(parent, fg_color="transparent")
            row.pack(fill="x")
            ctk.CTkLabel(row, text=f"{rule['contains']} → {rule['category']}", anchor="w", width=340).pack(side="left")
            self.button(row, "Remove", lambda i=index: self._remove_organizer_rule(i), width=70).pack(side="left")

    def _write_database(self, database):
        temp = self.database_path.with_suffix(".tmp")
        temp.write_text(json.dumps(database, indent=2, ensure_ascii=False), encoding="utf-8")
        temp.replace(self.database_path)
        self._update_destination()

    def _save_organizer_path(self, category, raw):
        path = Path(raw).expanduser()
        if not path.is_absolute():
            self.status.configure(text="Enter an absolute folder path.", text_color=ERROR_TEXT)
            return
        database = load_database(self.database_path)
        database.setdefault("destinations", {})[category] = str(path)
        aliases = {"video": ("film", "rap"), "audio": ("beats",)}
        for alias in aliases.get(category, ()):
            database["destinations"][alias] = str(path)
        self._write_database(database)
        self.status.configure(text=f"Saved {category} folder.")

    def _add_organizer_rule(self):
        term = self.rule_keyword.get().strip()
        if not term:
            self.status.configure(text="Enter a title phrase for the rule.")
            return
        database = load_database(self.database_path)
        database.setdefault("rules", []).append({"contains": term[:100], "category": self.rule_category.get()})
        self._write_database(database)
        self._navigate("Settings")

    def _remove_organizer_rule(self, index):
        database = load_database(self.database_path)
        if index < len(database.get("rules", [])):
            del database["rules"][index]
            self._write_database(database)
            self._navigate("Settings")

    def _save_custom_position(self):
        try:
            x, y = float(self.popup_x_entry.get()), float(self.popup_y_entry.get())
            if not (0 <= x <= 100 and 0 <= y <= 100):
                raise ValueError
        except ValueError:
            self.status.configure(text="Custom coordinates must be between 0 and 100.", text_color=ERROR_TEXT)
            return
        self.settings.popup_x_ratio, self.settings.popup_y_ratio = x / 100, y / 100
        self.settings.popup_position = "Custom"
        self.settings_store.save(self.settings)
        self._place_bottom_right()

    def _reset_defaults(self):
        if not messagebox.askyesno("Reset defaults", "Reset settings, organizer folders and rules?", parent=self):
            return
        self.settings = AppSettings()
        self.settings_store.save(self.settings)
        self.credentials.clear()
        self._write_database(json.loads(json.dumps(DEFAULT_DATABASE)))
        ctk.set_appearance_mode(self.settings.theme_mode.lower())
        self.accent, self.accent_hover = THEMES[self.settings.color_theme]
        self.format_var.set(self.settings.default_format)
        self.quality_var.set("best")
        self.organizer_job_var.set(self.settings.smart_organizer)
        self._navigate("Settings")

    def _choose_cookie_file(self):
        self._dialog_open = True
        try:
            path = filedialog.askopenfilename(parent=self, title="Select a cookies.txt file", filetypes=[("Text files", "*.txt"), ("All files", "*.*")])
        finally:
            self._dialog_open = False
        if path:
            self.auth_cookie_file = path
            self.auth_browser_var.set("None")
            self.cookie_label.configure(text=Path(path).name)

    def _prompt_save_profile(self):
        name = ctk.CTkInputDialog(text="Profile name", title="Save profile").get_input()
        if name:
            self._save_profile(name)

    def _setting_menu(self, label, attribute, values, parent, explanation):
        ctk.CTkLabel(parent, text=label).pack(anchor="w", pady=(8, 2))
        control = ctk.CTkOptionMenu(
            parent,
            values=values,
            variable=ctk.StringVar(value=str(getattr(self.settings, attribute))),
            command=lambda v: self._save_setting(attribute, int(v) if v.isdigit() else v),
            fg_color=CONTROL_BG,
            text_color=CONTROL_TEXT,
        )
        control.pack(anchor="w")
        self._help(control, explanation)

    def _save_setting(self, attribute, value):
        setattr(self.settings, attribute, value)
        self.settings_store.save(self.settings)
        if attribute == "theme_mode":
            ctk.set_appearance_mode(value.lower())
        if attribute == "color_theme":
            self.accent, self.accent_hover = THEMES[value]
        if attribute == "smart_organizer":
            self.organizer_job_var.set(value)
        if attribute == "popup_position":
            self._place_bottom_right()
        self._update_destination()

    def _reset_destination(self):
        self._manual_destination = False
        self._manual_destination_path = ""
        self._update_destination()
        self.status.configure(text="Automatic destination restored.")

    def _save_profile(self, requested_name=None):
        name = (requested_name or "").strip()[:60]
        if not name:
            return
        self.profiles[name] = {
            "format": self.format_var.get(),
            "quality": self.quality_var.get(),
            "premiere": self.format_var.get() == "mp4",
            "metadata": False,
        }
        temp = self.profiles_path.with_suffix(".tmp")
        temp.write_text(json.dumps(self.profiles, indent=2, ensure_ascii=False), encoding="utf-8")
        temp.replace(self.profiles_path)
        self.profile_menu.configure(values=list(self.profiles))
        self.profile_var.set(name)
        self.status.configure(text="Profile saved.")

    def _inspect_media(self):
        self._dialog_open = True
        try:
            path = filedialog.askopenfilename(parent=self)
        finally:
            self._dialog_open = False
        if not path:
            return

        def inspect():
            from .worker import _find_binary, _probe_media

            try:
                result = _probe_media(Path(path), _find_binary("ffprobe"))
                logging.info("Media inspection %s: %s", path, json.dumps(result))
                streams = ", ".join(s.get("codec_name", "?") for s in result.get("streams", []))
                self.events.put(("tool", f"Streams: {streams}. Details are in the log."))
            except Exception as error:
                self.events.put(("tool", str(error)))

        threading.Thread(target=inspect, daemon=True).start()

    def _matches_page(self, job):
        if self.current_page == "Queue":
            return job.status in ACTIVE_STATUSES | {JobStatus.QUEUED, JobStatus.PAUSED}
        return self.current_page == "History" and job.status in {JobStatus.COMPLETED, JobStatus.FAILED, JobStatus.CANCELLED}

    def _render_job(self, job):
        if not self._matches_page(job):
            old = self._job_widgets.pop(job.id, None)
            if old:
                old.destroy()
            self._render_cache.pop(job.id, None)
            return
        signature = (job.status, job.title, job.stage, round(job.progress, 2), job.speed, job.eta, job.destination,
                     job.format_notice, job.subtitle_notice)
        if self._render_cache.get(job.id) == signature:
            return
        cached = self._job_widgets.get(job.id)
        previous = self._render_cache.get(job.id)
        if cached and previous and previous[0] == job.status and previous[7:] == signature[7:]:
            cached.qc_title.configure(text=(job.title or job.url)[:100])
            cached.qc_detail.configure(text=f"{job.stage}   {int(job.progress * 100)}%   {job.speed}   {job.eta}")
            cached.qc_bar.set(job.progress)
            cached.qc_destination.configure(
                text="Folder: " + (job.destination or job.selected_destination or job.output_dir)
            )
            self._render_cache[job.id] = signature
            return
        self._render_cache[job.id] = signature
        old = self._job_widgets.pop(job.id, None)
        if old:
            old.destroy()
        card = ctk.CTkFrame(self.queue_container, fg_color=PANEL, corner_radius=10)
        card.pack(fill="x", pady=5)
        self._job_widgets[job.id] = card
        card.qc_title = ctk.CTkLabel(
            card, text=(job.title or job.url)[:100], anchor="w", font=("Segoe UI Semibold", 12), wraplength=560
        )
        card.qc_title.pack(fill="x", padx=12, pady=(10, 2))
        card.qc_detail = ctk.CTkLabel(
            card,
            text=f"{job.stage}   {int(job.progress * 100)}%   {job.speed}   {job.eta}",
            anchor="w",
            wraplength=560,
            text_color=ERROR_TEXT if job.status == JobStatus.FAILED else TEXT_MUTED,
        )
        card.qc_detail.pack(fill="x", padx=12)
        card.qc_destination = ctk.CTkLabel(
            card,
            text="Folder: " + (job.destination or job.selected_destination or job.output_dir),
            anchor="w",
            wraplength=560,
            font=("Segoe UI", 10),
            text_color=TEXT_MUTED,
        )
        card.qc_destination.pack(fill="x", padx=12)
        bar = ctk.CTkProgressBar(card, progress_color=PURPLE, height=4)
        bar.pack(fill="x", padx=12, pady=8)
        bar.set(job.progress)
        card.qc_bar = bar
        actions = ctk.CTkFrame(card, fg_color="transparent")
        actions.pack(fill="x", padx=12, pady=(0, 10))
        if job.status in ACTIVE_STATUSES | {JobStatus.QUEUED}:
            self.button(actions, "Cancel", lambda: self.engine.cancel(job.id), width=65).pack(side="right", padx=3)
            self.button(actions, "Pause", lambda: self.engine.pause(job.id), width=78).pack(side="right", padx=3)
        if job.status in {JobStatus.FAILED, JobStatus.PAUSED}:
            self.button(
                actions,
                "Partial files",
                lambda: self._open_path(Path(job.output_dir) / ".quickclip321" / job.id / "work"),
                width=112,
            ).pack(side="left", padx=3)
            self.button(
                actions,
                "Resume" if job.status == JobStatus.PAUSED else "Retry",
                lambda: self.engine.retry(job.id),
                width=100,
            ).pack(side="right", padx=3)
        if job.status == JobStatus.FAILED and job.error_code in {"ACCESS_DENIED", "LOGIN_REQUIRED"}:
            self.button(actions, "Use browser session…", lambda j=job: self._retry_with_browser(j), width=146).pack(side="left", padx=3)
            self.button(actions, "Use cookies.txt…", lambda j=job: self._retry_with_cookie_file(j), width=128).pack(side="left", padx=3)
        if job.status == JobStatus.PAUSED:
            self.button(actions, "Cancel", lambda: self.engine.cancel(job.id), width=65).pack(side="right", padx=3)
        if job.status in {JobStatus.COMPLETED, JobStatus.FAILED, JobStatus.CANCELLED}:
            self.button(actions, "Remove", lambda: self._remove_job(job.id), width=65).pack(side="right", padx=3)
        if job.status == JobStatus.COMPLETED:
            self.button(actions, "Open", lambda: self._open_path(job.result_path), width=50).pack(side="left", padx=3)
            self.button(actions, "Show in folder", lambda: self.reveal_file(job.result_path), width=116).pack(
                side="left", padx=3
            )
        if job.error_code:
            ctk.CTkLabel(card, text=job.error_code, text_color=TEXT_MUTED).pack(anchor="w", padx=12)
        for notice in (job.format_notice, job.subtitle_notice):
            if notice:
                ctk.CTkLabel(card, text=notice, text_color=TEXT_MUTED, anchor="w", wraplength=560).pack(fill="x", padx=12)

    def _retry_with_browser(self, job):
        dialog = ctk.CTkInputDialog(text="Browser name (Chrome, Edge, Firefox or Brave)", title="Use browser session")
        browser = (dialog.get_input() or "").strip().lower()
        if browser not in {"chrome", "edge", "firefox", "brave"}:
            return
        updated = self.job_store.get(job.id)
        if updated:
            self.engine.retry(job.id, auth_browser=browser)

    def _retry_with_cookie_file(self, job):
        self._dialog_open = True
        try:
            path = filedialog.askopenfilename(parent=self, title="Select a cookies.txt file", filetypes=[("Text files", "*.txt"), ("All files", "*.*")])
        finally:
            self._dialog_open = False
        if path:
            updated = self.job_store.get(job.id)
            if updated:
                self.engine.retry(job.id, auth_cookie_file=path)

    def _remove_job(self, job_id):
        if self.engine.remove(job_id):
            self._navigate(self.current_page)

    def _clear_completed(self):
        for job in self.job_store.list():
            if job.status == JobStatus.COMPLETED:
                self.engine.remove(job.id)
        self._navigate(self.current_page)

    def _create_tray(self):
        menu = pystray.Menu(
            pystray.MenuItem("Show/Hide", lambda *_: self.events.put(("toggle", None)), default=True),
            pystray.MenuItem("Download folder", lambda *_: self.events.put(("folder", None))),
            pystray.MenuItem("Exit", lambda *_: self.events.put(("exit", None))),
        )
        image = self.logo_bitmap.resize((64, 64), Image.Resampling.LANCZOS)
        self.tray_icon = pystray.Icon(APP_NAME, image, f"{APP_NAME} {__version__}", menu)
        threading.Thread(target=self.tray_icon.run, name="QuickClipTray", daemon=True).start()

    def toggle_window(self, *_):
        self.events.put(("toggle", None))

    def hide_window(self):
        self._hide_help()
        self.withdraw()
        self.visible = False

    def show_window(self):
        self._read_clipboard()  # populate immediately, without waiting for polling interval
        self.deiconify()
        self.overrideredirect(True)
        self._hide_from_taskbar()
        self.lift()
        self.focus_force()
        self.visible = True
        if self.expanded:
            self._navigate(self.current_page)
        self._place_bottom_right(activate=True)
        self.after(200, self._place_bottom_right)
        if self.url_var.get().strip() and self.url_var.get().strip() != self._preview_url:
            self._start_preview()

    def _read_clipboard(self):
        if not self.settings.clipboard_monitor:
            return
        try:
            raw = self.clipboard_get().strip()
            if raw == self._last_clipboard:
                return
            self._last_clipboard = raw
            valid, url, _ = validate_media_url(raw)
            if valid:
                self.url_var.set(url)
                self.status.configure(text="Clipboard URL detected. Ready to download.")
        except Exception:
            pass

    def _monitor_clipboard(self):
        if self._closing:
            return
        self._read_clipboard()
        self.after(2000, self._monitor_clipboard)

    def choose_folder(self, for_default=False):
        self._dialog_open = True
        try:
            selected = filedialog.askdirectory(parent=self, initialdir=self._selected_destination)
        finally:
            self._dialog_open = False
        if selected:
            if for_default:
                self.settings.output_dir = selected
                self.settings_store.save(self.settings)
            else:
                self._manual_destination = True
                self._manual_destination_path = selected
            self._update_destination()
            self.status.configure(text=f"{'Default' if for_default else 'Manual'} folder: {selected}")

    @staticmethod
    def _open_path(path):
        try:
            target = Path(path).resolve()
            if target.exists():
                os.startfile(str(target))
        except (AttributeError, OSError):
            logging.exception("Cannot open path %s", path)

    def open_folder(self, *_):
        target = Path(self.settings.output_dir)
        target.mkdir(parents=True, exist_ok=True)
        self._open_path(target)

    def reveal_file(self, path):
        if os.name == "nt":
            import subprocess

            target = Path(path)
            subprocess.Popen(["explorer", str(target)] if target.is_dir() else ["explorer", "/select,", str(target)])
        else:
            webbrowser.open(Path(path).parent.as_uri())

    def exit_app(self, *_):
        self.events.put(("exit", None))

    def _exit_on_ui(self):
        self._closing = True
        self.withdraw()
        self._stop_preview()
        if self.tray_icon:
            self.tray_icon.stop()

        # Cleanup may wait for Windows process termination; keep Tk responsive.
        def stop():
            self.engine.stop()
            self.events.put(("stopped", None))

        threading.Thread(target=stop, daemon=True).start()
        self._wait_shutdown()

    def _wait_shutdown(self):
        try:
            kind, _ = self.events.get_nowait()
            if kind == "stopped":
                self.destroy()
                return
        except queue.Empty:
            pass
        self.after(100, self._wait_shutdown)
