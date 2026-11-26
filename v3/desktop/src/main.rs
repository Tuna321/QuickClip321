#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use quickclip_core::{Engine, Request, Settings, Snapshot};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri_plugin_clipboard_manager::ClipboardExt;
#[derive(Default)]
struct ClipboardInbox(Mutex<Option<String>>);
#[derive(Default)]
struct DuplicateInbox(Mutex<Vec<Request>>);
#[tauri::command]
fn take_duplicate_requests(state: State<'_, DuplicateInbox>) -> Vec<Request> {
    std::mem::take(&mut *state.0.lock().unwrap())
}
#[derive(Default)]
struct ExitPending(std::sync::atomic::AtomicBool);
#[derive(Default)]
struct ClipboardConsumption {
    sequence: Mutex<Option<u32>>,
    submitted: Mutex<Vec<(String, String)>>,
}
#[cfg(windows)]
fn matching_clipboard_sequence(app: &tauri::AppHandle, url: &str) -> Option<u32> {
    let seq = clipboard_sequence();
    let text = app.clipboard().read_text().ok()?;
    (clipboard_sequence() == seq
        && quickclip_core::validate_url(text.trim()).ok().as_deref() == Some(url))
    .then_some(seq)
}
#[cfg(not(windows))]
fn matching_clipboard_sequence(_: &tauri::AppHandle, _: &str) -> Option<u32> {
    None
}
fn consume_link(app: &tauri::AppHandle, url: &str, sequence: Option<u32>, job: Option<String>) {
    let state = app.state::<ClipboardConsumption>();
    let mut consumed = state.sequence.lock().unwrap();
    #[cfg(windows)]
    if sequence.is_some_and(|seq| seq == clipboard_sequence()) {
        *consumed = sequence;
    }
    let inbox = app.state::<ClipboardInbox>();
    let mut pending = inbox.0.lock().unwrap();
    if pending.as_deref() == Some(url) {
        *pending = None;
    }
    if let Some(id) = job {
        let mut events = state.submitted.lock().unwrap();
        events.push((id, url.to_owned()));
        if events.len() > 64 {
            events.remove(0);
        }
    }
}
#[tauri::command]
fn take_submitted_links(state: State<'_, ClipboardConsumption>) -> Vec<(String, String)> {
    std::mem::take(&mut *state.submitted.lock().unwrap())
}
#[derive(Clone, Default, serde::Serialize)]
struct ShortcutState {
    status: String,
    hits: u64,
}
#[derive(Default)]
struct HotkeyStatus(Mutex<std::collections::BTreeMap<String, ShortcutState>>);
#[derive(Default)]
struct CaptureUntil(Mutex<Option<std::time::Instant>>);
#[derive(Default)]
struct TestUntil(Mutex<Option<std::time::Instant>>);
#[tauri::command]
fn shortcut_test(test: State<'_, TestUntil>, active: bool) {
    *test.0.lock().unwrap() = if active {
        Some(std::time::Instant::now() + std::time::Duration::from_secs(30))
    } else {
        None
    };
}
#[tauri::command]
fn shortcut_capture(capture: State<'_, CaptureUntil>, active: bool) {
    *capture.0.lock().unwrap() = if active {
        Some(std::time::Instant::now() + std::time::Duration::from_secs(15))
    } else {
        None
    };
}
#[tauri::command]
fn hotkey_status(
    status: State<'_, HotkeyStatus>,
) -> std::collections::BTreeMap<String, ShortcutState> {
    status.0.lock().unwrap().clone()
}
#[tauri::command]
fn shortcut_hit(status: State<'_, HotkeyStatus>, action: String) {
    if matches!(action.as_str(), "submit" | "link" | "hide") {
        status.0.lock().unwrap().entry(action).or_default().hits += 1;
    }
}
#[tauri::command]
fn remember_download(
    engine: State<'_, Engine>,
    preset: String,
    quality: String,
    destination: PathBuf,
    organize: bool,
) -> Result<(), String> {
    quickclip_core::selector(&preset, &quality)?;
    if !destination.is_absolute() {
        return Err("Choose an absolute destination folder.".into());
    }
    let mut s = engine.snapshot().settings;
    if s.prefs.default_format == preset
        && s.prefs.default_quality == quality
        && s.prefs.last_destination.as_ref() == Some(&destination)
        && s.prefs.last_organize == organize
    {
        return Ok(());
    }
    s.prefs.default_preset = "custom".into();
    s.prefs.default_format = preset;
    if matches!(s.prefs.default_format.as_str(), "editor" | "original") {
        s.prefs.last_video_quality = quality.clone();
    }
    s.prefs.default_quality = quality;
    s.prefs.last_destination = Some(destination);
    s.prefs.last_organize = organize;
    engine.settings(s)
}
#[cfg(windows)]
mod native_notice;
fn show_shortcut_toast(app: &tauri::AppHandle, message: &str) {
    #[cfg(windows)]
    if let Some(w) = app.get_webview_window("main") {
        if let Ok(Some(m)) = w.primary_monitor() {
            let area = m.work_area();
            let dpi = m.scale_factor();
            let width = (340. * dpi).round() as i32;
            let height = (48. * dpi).round() as i32;
            native_notice::show(
                message,
                area.position.x + (area.size.width as i32 - width) / 2,
                area.position.y + (24. * dpi) as i32,
                width,
                height,
                dpi,
            );
        }
    }
}
fn run_shortcut(app: &tauri::AppHandle, action: &str) {
    let engine = app.state::<Engine>();
    let s = engine.snapshot().settings;
    let tr = s.prefs.language == "tr";
    if (isolated_run() && std::env::var_os("QC_SHORTCUT_SMOKE").is_some())
        || app
            .state::<TestUntil>()
            .0
            .lock()
            .unwrap()
            .is_some_and(|until| std::time::Instant::now() < until)
    {
        show_shortcut_toast(
            app,
            if tr {
                "K\u{0131}sayol alg\u{0131}land\u{0131}"
            } else {
                "Shortcut recognized"
            },
        );
        return;
    }
    if app
        .state::<ExitPending>()
        .0
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return;
    }
    let message = match action {
        "open" => {
            if let Some(w) = app.get_webview_window("main") {
                toggle_popup(&w, None);
            }
            return;
        }
        "folder" => {
            let path = s.prefs.last_destination.as_ref().unwrap_or(&s.destination);
            if std::fs::create_dir_all(path).is_ok() && reveal_folder(path).is_ok() {
                return;
            }
            if tr {
                "Klas\u{00f6}r a\u{00e7}\u{0131}lamad\u{0131}"
            } else {
                "Could not open folder"
            }
        }
        "download" | "mp3" | "mp4" => {
            if !configured_tools_ready(&s) {
                if tr {
                    "\u{00d6}nce kurulumu tamamla"
                } else {
                    "Finish setup first"
                }
            } else if let Some(url) = if isolated_run() {
                // Only a loopback fixture is allowed; do not read the real clipboard.
                std::env::var("QC_SHORTCUT_DOWNLOAD_FIXTURE")
                    .ok()
                    .filter(|v| v.starts_with("http://127.0.0.1:"))
                    .and_then(|v| quickclip_core::validate_url(v.trim()).ok())
            } else {
                app.clipboard()
                    .read_text()
                    .ok()
                    .and_then(|v| quickclip_core::validate_url(v.trim()).ok())
            } {
                let preset = match action {
                    "mp3" => "mp3".into(),
                    "mp4" => "editor".into(),
                    _ => s.prefs.default_format.clone(),
                };
                let quality = if matches!(preset.as_str(), "mp3" | "m4a" | "wav") {
                    "best".into()
                } else {
                    if action == "mp4" {
                        s.prefs.last_video_quality.clone()
                    } else {
                        s.prefs.default_quality.clone()
                    }
                };
                let destination = s.prefs.last_destination.clone().unwrap_or(s.destination);
                let organize = if s.prefs.last_destination.is_some() {
                    s.prefs.last_organize
                } else {
                    s.prefs.smart_organizer
                };
                let seq = if isolated_run() {
                    None
                } else {
                    matching_clipboard_sequence(app, &url)
                };
                let request = Request {
                    url: url.clone(),
                    preset,
                    quality,
                    playlist: false,
                    organize,
                    options: Default::default(),
                    destination,
                };
                match engine.enqueue_checked(request.clone(), false) {
                    Ok(id) => {
                        consume_link(app, &url, seq, Some(id.clone()));
                        let running = engine
                            .snapshot()
                            .jobs
                            .iter()
                            .any(|j| j.id == id && j.status == quickclip_core::Status::Running);
                        match (tr, running) {
                            (true, true) => "\u{0130}ndirme ba\u{015f}lat\u{0131}ld\u{0131}",
                            (true, false) => "S\u{0131}raya eklendi",
                            (false, true) => "Download started",
                            _ => "Added to queue",
                        }
                    }
                    Err(e) if e.starts_with("Duplicate completed job: ") => {
                        let inbox = app.state::<DuplicateInbox>();
                        let mut pending = inbox.0.lock().unwrap();
                        if pending.len() < 8 {
                            pending.push(request);
                        }
                        drop(pending);
                        if let Some(w) = app.get_webview_window("main") {
                            show_popup(&w, None);
                        }
                        return;
                    }
                    Err(_) => {
                        if tr {
                            "\u{0130}ndirme eklenemedi"
                        } else {
                            "Download not added"
                        }
                    }
                }
            } else if tr {
                "Ge\u{00e7}erli bir ba\u{011f}lant\u{0131} kopyala"
            } else {
                "Copy a valid media link"
            }
        }
        _ => return,
    };
    show_shortcut_toast(app, message);
}
#[cfg(windows)]
fn watch_hotkey(app: tauri::AppHandle) {
    use windows_sys::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};
    std::thread::spawn(move || {
        let mut current = std::collections::BTreeMap::new();
        let mut suspended = false;
        let mut registered = std::collections::HashMap::<i32, String>::new();
        loop {
            let wanted = app.state::<Engine>().shortcut_config();
            let recording = app
                .state::<CaptureUntil>()
                .0
                .lock()
                .unwrap()
                .is_some_and(|until| std::time::Instant::now() < until);
            if current != wanted || recording != suspended {
                for id in registered.keys() {
                    unsafe {
                        UnregisterHotKey(std::ptr::null_mut(), *id);
                    }
                }
                registered.clear();
                current = wanted;
                suspended = recording;
                let status = app.state::<HotkeyStatus>();
                let mut status = status.0.lock().unwrap();
                let mut used = std::collections::HashSet::new();
                for (i, (action, _, global)) in
                    quickclip_core::preferences::SHORTCUTS.iter().enumerate()
                {
                    let key = &current[*action];
                    let id = 321 + i as i32;
                    let code = quickclip_core::preferences::parse_shortcut(key, *global);
                    let value = if recording {
                        "Recording"
                    } else if key == "Off" {
                        "Off"
                    } else if let Some((mods, vk)) = code {
                        if !used.insert((mods, vk)) {
                            "Duplicate shortcut"
                        } else if !global {
                            "In app"
                        } else if unsafe {
                            RegisterHotKey(std::ptr::null_mut(), id, mods | MOD_NOREPEAT, vk)
                        } != 0
                        {
                            registered.insert(id, action.to_string());
                            "Active"
                        } else {
                            "Unavailable (shortcut in use)"
                        }
                    } else {
                        "Invalid shortcut"
                    };
                    status.entry(action.to_string()).or_default().status = value.into();
                }
            }
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(
                    &mut msg,
                    std::ptr::null_mut(),
                    WM_HOTKEY,
                    WM_HOTKEY,
                    PM_REMOVE,
                ) != 0
                {
                    if let Some(action) = registered.get(&(msg.wParam as i32)).cloned() {
                        app.state::<HotkeyStatus>()
                            .0
                            .lock()
                            .unwrap()
                            .entry(action.clone())
                            .or_default()
                            .hits += 1;
                        if let Some(dir) = std::env::var_os("QC_V3_SMOKE_DIR") {
                            if std::env::var_os("QC_SHORTCUT_SMOKE").is_some() {
                                let _ = std::fs::write(
                                    PathBuf::from(dir).join("hotkey-fired.txt"),
                                    &action,
                                );
                            }
                        }
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || run_shortcut(&handle, &action));
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(70));
        }
    });
}
#[tauri::command]
fn visual_page() -> String {
    std::env::var("QC_V3_VISUAL").unwrap_or_default()
}
#[tauri::command]
fn smoke_active() -> bool {
    std::env::var_os("QC_V3_SMOKE_DIR").is_some()
}
#[tauri::command]
fn take_clipboard_link(
    engine: State<'_, Engine>,
    inbox: State<'_, ClipboardInbox>,
) -> Option<String> {
    if !engine.clipboard_enabled() {
        inbox.0.lock().ok()?.take();
        return None;
    }
    inbox.0.lock().ok()?.take()
}
#[cfg(windows)]
fn clipboard_sequence() -> u32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
    }
    unsafe { GetClipboardSequenceNumber() }
}
#[cfg(windows)]
fn watch_clipboard(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            if !app.state::<Engine>().clipboard_enabled() {
                if let Ok(mut pending) = app.state::<ClipboardInbox>().0.lock() {
                    *pending = None;
                }
                last = None;
            } else {
                let seq = clipboard_sequence();
                if last != Some(seq) {
                    if let Ok(text) = app.clipboard().read_text() {
                        last = Some(seq);
                        let link = quickclip_core::validate_url(text.trim()).ok();
                        if let Some(link) = link {
                            let consumption = app.state::<ClipboardConsumption>();
                            let consumed = consumption.sequence.lock().unwrap();
                            if *consumed != Some(seq) && clipboard_sequence() == seq {
                                if let Ok(mut pending) = app.state::<ClipboardInbox>().0.lock() {
                                    *pending = Some(link);
                                }
                            }
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(350));
        }
    });
}
#[cfg(not(windows))]
fn watch_clipboard(_: tauri::AppHandle) {}
#[cfg(windows)]
fn round_window(window: &tauri::WebviewWindow) -> bool {
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateRoundRectRgn(l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> isize;
        fn DeleteObject(h: isize) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowRgn(hwnd: isize, rgn: isize, redraw: i32) -> i32;
    }
    let (Ok(hwnd), Ok(size)) = (window.hwnd(), window.outer_size()) else {
        return false;
    };
    let diameter = (32.0 * window.scale_factor().unwrap_or(1.)).round() as i32;
    // SetWindowRgn can emit window-position events. Avoid reapplying an unchanged region.
    static LAST: Mutex<Option<(isize, u32, u32, i32)>> = Mutex::new(None);
    let key = (hwnd.0 as isize, size.width, size.height, diameter);
    if let Ok(mut last) = LAST.lock() {
        if *last == Some(key) {
            return true;
        }
        *last = Some(key);
    }
    unsafe {
        let region = CreateRoundRectRgn(
            0,
            0,
            size.width as i32 + 1,
            size.height as i32 + 1,
            diameter,
            diameter,
        );
        if region == 0 {
            if let Ok(mut last) = LAST.lock() {
                *last = None;
            }
            return false;
        }
        if SetWindowRgn(hwnd.0 as isize, region, 1) == 0 {
            DeleteObject(region);
            if let Ok(mut last) = LAST.lock() {
                *last = None;
            }
            return false;
        }
    }
    true
}
#[cfg(not(windows))]
fn round_window(_: &tauri::WebviewWindow) -> bool {
    true
}
#[cfg(windows)]
fn has_round_region(window: &tauri::WebviewWindow) -> bool {
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateRectRgn(l: i32, t: i32, r: i32, b: i32) -> isize;
        fn DeleteObject(h: isize) -> i32;
        fn PtInRegion(h: isize, x: i32, y: i32) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetWindowRgn(hwnd: isize, rgn: isize) -> i32;
    }
    let (Ok(hwnd), Ok(size)) = (window.hwnd(), window.outer_size()) else {
        return false;
    };
    unsafe {
        let r = CreateRectRgn(0, 0, 0, 0);
        if r == 0 {
            return false;
        }
        let ok = GetWindowRgn(hwnd.0 as isize, r) > 0
            && PtInRegion(r, 0, 0) == 0
            && PtInRegion(r, size.width as i32 / 2, size.height as i32 / 2) != 0;
        DeleteObject(r);
        ok
    }
}
#[cfg(not(windows))]
fn has_round_region(_: &tauri::WebviewWindow) -> bool {
    true
}
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};
#[tauri::command]
async fn media_preview(
    app: tauri::AppHandle,
    url: String,
) -> Result<quickclip_core::Preview, String> {
    let tool = app.state::<Engine>().snapshot().settings.yt_dlp;
    tauri::async_runtime::spawn_blocking(move || quickclip_core::preview(&url, &tool))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn live_preview_frame(app: tauri::AppHandle, id: String, seconds: f64) -> Option<String> {
    let s = app.state::<Engine>().snapshot();
    if !s
        .jobs
        .iter()
        .any(|j| j.id == id && j.status == quickclip_core::Status::Running)
    {
        return None;
    }
    tauri::async_runtime::spawn_blocking(move || {
        quickclip_core::live_frame(&id, &s.settings.ffmpeg, seconds)
    })
    .await
    .ok()
    .flatten()
}
#[tauri::command]
fn validate_links(urls: Vec<String>) -> Result<Vec<String>, String> {
    if urls.len() > 100 {
        return Err("Add at most 100 links at once.".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut result = vec![];
    for url in urls {
        let u = quickclip_core::validate_url(url.trim())?;
        if seen.insert(u.clone()) {
            result.push(u);
        }
    }
    if result.is_empty() {
        return Err("Paste at least one link.".into());
    }
    Ok(result)
}
#[tauri::command]
fn queue_action(engine: State<'_, Engine>, action: String) -> Result<(), String> {
    engine.bulk_action(&action)
}
#[tauri::command]
fn snapshot(engine: State<'_, Engine>) -> Snapshot {
    engine.snapshot()
}
#[tauri::command]
fn enqueue(
    app: tauri::AppHandle,
    engine: State<'_, Engine>,
    request: Request,
    allow_duplicate: Option<bool>,
) -> Result<String, String> {
    if app
        .state::<ExitPending>()
        .0
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("Saving queue before exit".into());
    }
    if !configured_tools_ready(&engine.snapshot().settings) {
        return Err("QuickClip321 is not ready yet. Complete media setup first.".into());
    }
    let url = quickclip_core::validate_url(request.url.trim())?;
    let seq = if engine.clipboard_enabled() && !isolated_run() {
        matching_clipboard_sequence(&app, &url)
    } else {
        None
    };
    let id = engine.enqueue_checked(request, allow_duplicate.unwrap_or(false))?;
    consume_link(&app, &url, seq, None);
    Ok(id)
}
#[tauri::command]
fn cancel(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    engine.cancel(&id)
}
#[tauri::command]
fn pause_waiting(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    engine.hold(&id, true)
}
#[tauri::command]
fn resume_waiting(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    engine.hold(&id, false)
}
#[tauri::command]
async fn setup_managed_tools(engine: State<'_, Engine>) -> Result<Settings, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let settings = engine.setup_managed_tools()?;
        if !isolated_run() {
            let _ = engine.resume_saved_queue();
        }
        Ok(settings)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn setup_progress(engine: State<'_, Engine>) -> quickclip_core::SetupProgress {
    engine.setup_progress()
}
fn configured_tools_ready(settings: &Settings) -> bool {
    let managed_node = quickclip_core::managed_node_path(&settings.yt_dlp).is_some();
    settings.yt_dlp.is_absolute()
        && settings.yt_dlp.is_file()
        && settings.ffmpeg.is_absolute()
        && settings.ffmpeg.is_file()
        && settings.ffmpeg.with_file_name("ffprobe.exe").is_file()
        && (managed_node || find_tool("node.exe").is_file())
}
#[tauri::command]
fn tools_ready(engine: State<'_, Engine>) -> bool {
    configured_tools_ready(&engine.snapshot().settings)
}
#[tauri::command]
fn detect_tools() -> [String; 2] {
    [
        find_tool("yt-dlp.exe").to_string_lossy().into_owned(),
        find_tool("ffmpeg.exe").to_string_lossy().into_owned(),
    ]
}
#[tauri::command]
fn organizer_counts() -> [usize; 8] {
    quickclip_core::organizer::counts()
}
#[tauri::command]
fn diagnostics(engine: State<'_, Engine>) -> String {
    let s = engine.snapshot();
    let mut out = format!("QuickClip321 {}\n", env!("CARGO_PKG_VERSION"));
    for (label, path) in [
        ("yt-dlp", &s.settings.yt_dlp),
        ("FFmpeg", &s.settings.ffmpeg),
    ] {
        out.push_str(&format!(
            "{label}: {}\n",
            if path.is_file() {
                "Detected"
            } else {
                "Missing"
            }
        ));
    }
    let probe = s.settings.ffmpeg.with_file_name("ffprobe.exe");
    out.push_str(&format!(
        "FFprobe: {}\n",
        if probe.is_file() {
            "Detected"
        } else {
            "Missing"
        }
    ));
    for (name, status) in [
        ("Waiting", quickclip_core::Status::Queued),
        ("Paused", quickclip_core::Status::Paused),
        ("Running", quickclip_core::Status::Running),
        ("Completed", quickclip_core::Status::Completed),
        ("Failed", quickclip_core::Status::Failed),
    ] {
        out.push_str(&format!(
            "{name}: {}\n",
            s.jobs.iter().filter(|j| j.status == status).count()
        ));
    }
    out.push_str("Privacy: report excludes URLs, file paths, clipboard and error logs.\n");
    for (label, needle) in [
        ("Removed source", "removed by the source"),
        ("Private video", "video is private"),
        ("Network", "Network error"),
        ("Format", "Selected format"),
        ("Storage", "folder permissions"),
        ("Media tools", "Media engine"),
        ("Interrupted", "Interrupted"),
    ] {
        let n = s
            .jobs
            .iter()
            .filter(|j| j.status == quickclip_core::Status::Failed && j.message.contains(needle))
            .count();
        if n > 0 {
            out.push_str(&format!("{label}: {n} failed jobs\n"));
        }
    }

    out
}
#[tauri::command]
fn retry(engine: State<'_, Engine>, id: String) -> Result<String, String> {
    engine.retry(&id)
}
#[tauri::command]
fn remove(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    engine.remove(&id)
}
fn isolated_run() -> bool {
    std::env::var_os("QC_V3_SMOKE_DIR").is_some() || std::env::var_os("QC_V3_BENCH_DIR").is_some()
}
#[cfg(windows)]
fn sync_startup(enabled: bool) -> Result<(), String> {
    use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};
    if isolated_run() {
        return Ok(());
    }
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let name = wide("QuickClip321V3");
    let mut h = std::ptr::null_mut();
    unsafe {
        let result = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut h,
            std::ptr::null_mut(),
        );
        if result != ERROR_SUCCESS {
            return Err(format!("Windows startup settings unavailable ({result})."));
        }
        let result = if enabled {
            let exe = std::env::current_exe().map_err(|e| {
                RegCloseKey(h);
                e.to_string()
            })?;
            let value = wide(&format!("\"{}\"", exe.display()));
            RegSetValueExW(
                h,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(h, name.as_ptr())
        };
        RegCloseKey(h);
        if result == ERROR_SUCCESS || (!enabled && result == 2) {
            Ok(())
        } else {
            Err(format!("Could not update Windows startup ({result})."))
        }
    }
}
#[cfg(not(windows))]
fn sync_startup(_: bool) -> Result<(), String> {
    Ok(())
}
#[tauri::command]
fn save_settings(
    window: tauri::WebviewWindow,
    engine: State<'_, Engine>,
    settings: Settings,
) -> Result<(), String> {
    let before = engine.snapshot().settings.prefs;
    let top = settings.prefs.always_on_top;
    let startup_changed = before.run_on_startup != settings.prefs.run_on_startup;
    let position_changed = before.popup_mode != settings.prefs.popup_mode
        || before.popup_x != settings.prefs.popup_x
        || before.popup_y != settings.prefs.popup_y;
    if startup_changed {
        sync_startup(settings.prefs.run_on_startup)?;
    }
    if let Err(e) = engine.settings(settings) {
        if startup_changed {
            let _ = sync_startup(before.run_on_startup);
        }
        return Err(e);
    }
    let _ = apply_topmost(&window, top);
    if position_changed {
        position_popup(&window, None);
    }
    Ok(())
}
#[tauri::command]
fn open_downloads(engine: State<'_, Engine>) -> Result<(), String> {
    let folder = engine.snapshot().settings.destination;
    if !folder.is_absolute() {
        return Err("Invalid download folder.".into());
    }
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    reveal_folder(&folder)
}
#[tauri::command]
fn open_folder(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    let job = engine
        .snapshot()
        .jobs
        .into_iter()
        .find(|j| j.id == id)
        .ok_or("Unknown job")?;
    if job.files.is_empty() {
        return Err("No saved media.".into());
    }
    let file = job.files.iter().find(|p| p.is_file());
    let folder = file
        .or_else(|| job.files.first())
        .and_then(|p| p.parent())
        .ok_or("No saved media folder.")?;
    if !folder.is_dir() {
        return Err("The original download folder no longer exists.".into());
    }
    reveal_folder(folder)
}
#[cfg(windows)]
fn reveal_folder(folder: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    if !folder.is_absolute() || !folder.is_dir() {
        return Err("Folder unavailable.".into());
    }
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let path: Vec<u16> = folder.as_os_str().encode_wide().chain(Some(0)).collect();
    let status = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if status as isize <= 32 {
        Err("Windows could not open the download folder.".into())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn reveal_folder(_: &std::path::Path) -> Result<(), String> {
    Err("Windows required".into())
}

#[tauri::command]
fn persist_fixture_report(app: tauri::AppHandle, passed: bool) -> Result<(), String> {
    let root = std::env::var_os("QC_V3_SMOKE_DIR").ok_or("Not a smoke test")?;
    std::fs::write(
        PathBuf::from(root).join("persist-fixture.txt"),
        if passed { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())?;
    app.exit(if passed { 0 } else { 1 });
    Ok(())
}
#[tauri::command]
fn ui_ready(
    app: tauri::AppHandle,
    passed: bool,
    theme_passed: bool,
    clipboard_passed: bool,
) -> Result<(), String> {
    if let Some(dir) = std::env::var_os("QC_V3_SMOKE_DIR") {
        let root = PathBuf::from(dir);
        let w = app.get_webview_window("main").ok_or("Window missing")?;
        let started_hidden =
            std::fs::read_to_string(root.join("startup-hidden.txt")).unwrap_or_default() == "true";
        let frameless = !w.is_decorated().unwrap_or(true);
        let topmost = w.is_always_on_top().unwrap_or(false);
        let tray = app.tray_by_id("quickclip321-tray").is_some();
        #[cfg(windows)]
        let no_taskbar = {
            #[link(name = "user32")]
            extern "system" {
                fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
            }
            let hwnd = w.hwnd().map_err(|e| e.to_string())?.0 as isize;
            let ex = unsafe { GetWindowLongPtrW(hwnd, -20) };
            ex & 0x00040000 == 0 && ex & 0x00000080 != 0
        };
        #[cfg(not(windows))]
        let no_taskbar = true;
        let within_workarea = if let (Ok(Some(m)), Ok(p), Ok(sz)) =
            (w.current_monitor(), w.outer_position(), w.outer_size())
        {
            let a = m.work_area();
            p.x >= a.position.x
                && p.y >= a.position.y
                && i64::from(p.x) + i64::from(sz.width)
                    <= i64::from(a.position.x) + i64::from(a.size.width)
                && i64::from(p.y) + i64::from(sz.height)
                    <= i64::from(a.position.y) + i64::from(a.size.height)
        } else {
            false
        };
        let _ = w.hide();
        let hide_works = !w.is_visible().unwrap_or(true);
        show_popup(&w, None);
        let show_works = w.is_visible().unwrap_or(false);
        let rounded = has_round_region(&w);
        let mut prefs = app.state::<Engine>().snapshot().settings.prefs;
        prefs.popup_mode = "Custom".into();
        prefs.popup_x = 0.;
        prefs.popup_y = 0.;
        let moved = place_with(&w, None, &prefs).is_ok();
        let custom_works = moved
            && w.current_monitor()
                .ok()
                .flatten()
                .zip(w.outer_position().ok())
                .is_some_and(|(m, p)| {
                    p.x == m.work_area().position.x && p.y == m.work_area().position.y
                });
        position_popup(&w, None);
        let ok = passed
            && theme_passed
            && clipboard_passed
            && started_hidden
            && frameless
            && topmost
            && tray
            && no_taskbar
            && within_workarea
            && hide_works
            && show_works
            && rounded
            && custom_works;
        let report=format!("{{\"ui_ready\":{passed},\"snapshot\":{passed},\"started_hidden\":{started_hidden},\"frameless\":{frameless},\"topmost\":{topmost},\"tray\":{tray},\"no_taskbar\":{no_taskbar},\"within_workarea\":{within_workarea},\"hide_works\":{hide_works},\"show_works\":{show_works},\"rounded\":{rounded},\"custom_position\":{custom_works},\"theme_matrix\":{theme_passed},\"clipboard_flow\":{clipboard_passed}}}");
        std::fs::write(root.join("ui-smoke.json"), report).map_err(|e| e.to_string())?;
        if !ok || std::env::var_os("QC_V3_VISUAL").is_none() {
            app.exit(if ok { 0 } else { 1 });
        }
    }
    Ok(())
}
fn find_tool(name: &str) -> PathBuf {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .filter(|p| p.is_absolute())
        .map(|p| p.join(name))
        .find(|p| p.is_file())
        .unwrap_or_default()
}
#[tauri::command]
fn preview_position(
    window: tauri::WebviewWindow,
    mode: String,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let mut p = window
        .app_handle()
        .state::<Engine>()
        .snapshot()
        .settings
        .prefs;
    p.popup_mode = mode;
    p.popup_x = x;
    p.popup_y = y;
    p.normalize();
    place_with(&window, None, &p)
}
fn position_popup(window: &tauri::WebviewWindow, point: Option<(f64, f64)>) {
    let p = window
        .app_handle()
        .state::<Engine>()
        .snapshot()
        .settings
        .prefs;
    let _ = place_with(window, point, &p);
}
fn place_with(
    window: &tauri::WebviewWindow,
    point: Option<(f64, f64)>,
    prefs: &quickclip_core::preferences::Preferences,
) -> Result<(), String> {
    let monitor = point
        .and_then(|(x, y)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    if let (Some(m), Ok(size)) = (monitor, window.outer_size()) {
        let area = m.work_area();
        let (x, y) = quickclip_core::preferences::popup_position(
            quickclip_core::preferences::WorkArea {
                left: area.position.x,
                top: area.position.y,
                width: area.size.width as i32,
                height: area.size.height as i32,
            },
            (size.width as i32, size.height as i32),
            point,
            prefs,
            (8.0 * m.scale_factor()).round() as i32,
        );
        window
            .set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("Monitor geometry unavailable.".into())
}
#[cfg(windows)]
fn enforce_tray_style(window: &tauri::WebviewWindow) {
    #[link(name = "user32")]
    extern "system" {
        fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
        fn SetWindowPos(
            hwnd: isize,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }
    if let Ok(hwnd) = window.hwnd() {
        let hwnd = hwnd.0 as isize;
        unsafe {
            let old = GetWindowLongPtrW(hwnd, -20);
            let style = (old & !0x00040000) | 0x00000080;
            if old != style {
                SetWindowLongPtrW(hwnd, -20, style);
                SetWindowPos(
                    hwnd,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0x0001 | 0x0002 | 0x0004 | 0x0010 | 0x0020,
                );
            }
        }
    }
}
#[cfg(not(windows))]
fn enforce_tray_style(_: &tauri::WebviewWindow) {}
#[cfg(windows)]
fn apply_topmost(window: &tauri::WebviewWindow, top: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as windows_sys::Win32::Foundation::HWND;
    let ok = unsafe {
        SetWindowPos(
            hwnd,
            if top { HWND_TOPMOST } else { HWND_NOTOPMOST },
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
    enforce_tray_style(window);
    if ok == 0 {
        Err("Cannot change window z-order.".into())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn apply_topmost(window: &tauri::WebviewWindow, top: bool) -> Result<(), String> {
    window.set_always_on_top(top).map_err(|e| e.to_string())
}
// A tray click can move focus to Explorer before the mouse-up event.
// Visibility and physical z-order, not focus, determine toggle behavior.
fn toggle_popup(window: &tauri::WebviewWindow, point: Option<(f64, f64)>) {
    if window.is_visible().unwrap_or(false) && !popup_covered(window, false) {
        let _ = window.hide();
    } else {
        show_popup(window, point);
    }
}
#[cfg(not(windows))]
fn popup_covered(_: &tauri::WebviewWindow, _: bool) -> bool {
    false
}
#[cfg(windows)]
fn popup_covered(window: &tauri::WebviewWindow, foreground_only: bool) -> bool {
    use windows_sys::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::*,
    };
    if window.state::<Engine>().popup_topmost() {
        return false;
    }
    let Ok(hwnd) = window.hwnd() else {
        return false;
    };
    let hwnd = hwnd.0 as HWND;
    unsafe {
        let mut own: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut own) == 0 {
            return false;
        }
        let mut own_pid = 0;
        GetWindowThreadProcessId(hwnd, &mut own_pid);
        let mut other = GetWindow(hwnd, GW_HWNDPREV);
        // Inspect only windows above ours. No screenshot, pixel capture or focus dependency.
        for _ in 0..512 {
            if other.is_null() {
                break;
            }
            let next = GetWindow(other, GW_HWNDPREV);
            let mut pid = 0;
            GetWindowThreadProcessId(other, &mut pid);
            if pid != own_pid && IsWindowVisible(other) != 0 && IsIconic(other) == 0 {
                let mut name = [0u16; 128];
                let n = GetClassNameW(other, name.as_mut_ptr(), name.len() as i32).max(0) as usize;
                let class = String::from_utf16_lossy(&name[..n]);
                let ex = GetWindowLongPtrW(other, GWL_EXSTYLE) as u32;
                let mut cloaked: u32 = 0;
                #[link(name = "dwmapi")]
                extern "system" {
                    fn DwmGetWindowAttribute(
                        hwnd: HWND,
                        attribute: u32,
                        value: *mut std::ffi::c_void,
                        size: u32,
                    ) -> i32;
                }
                let _ = DwmGetWindowAttribute(other, 14, (&mut cloaked as *mut u32).cast(), 4);
                let shell = matches!(
                    class.as_str(),
                    "Shell_TrayWnd"
                        | "Shell_SecondaryTrayWnd"
                        | "NotifyIconOverflowWindow"
                        | "TopLevelWindowForOverflowXamlIsland"
                        | "Progman"
                        | "WorkerW"
                        | "tooltips_class32"
                        | "XamlExplorerHostIslandWindow"
                        | "Windows.UI.Core.CoreWindow"
                );
                let mut r: RECT = std::mem::zeroed();
                if !shell
                    && cloaked == 0
                    && ex & WS_EX_TRANSPARENT == 0
                    && ex & WS_EX_TOOLWINDOW == 0
                    && GetWindowRect(other, &mut r) != 0
                    && rect_overlap(
                        (own.left, own.top, own.right, own.bottom),
                        (r.left, r.top, r.right, r.bottom),
                    )
                {
                    // Foreground windows must also be above the popup to count as covering.
                    let foreground = GetForegroundWindow();
                    if !foreground_only
                        || foreground == other
                        || GetAncestor(foreground, GA_ROOT) == other
                    {
                        return true;
                    }
                }
            }
            other = next;
        }
    }
    false
}
fn rect_overlap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0.max(b.0) < a.2.min(b.2) && a.1.max(b.1) < a.3.min(b.3)
}
#[derive(Default)]
struct PopupGuard {
    pending: std::sync::atomic::AtomicBool,
    last_show: Mutex<Option<std::time::Instant>>,
}
fn watch_popup(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if app.state::<Engine>().popup_topmost() {
            continue;
        }
        use std::sync::atomic::Ordering;
        if app
            .state::<PopupGuard>()
            .pending
            .swap(true, Ordering::AcqRel)
        {
            continue;
        }
        let handle = app.clone();
        if app
            .run_on_main_thread(move || {
                handle
                    .state::<PopupGuard>()
                    .pending
                    .store(false, Ordering::Release);
                let Some(w) = handle.get_webview_window("main") else {
                    return;
                };
                if !w.is_visible().unwrap_or(false) {
                    return;
                }
                let recent = handle
                    .state::<PopupGuard>()
                    .last_show
                    .lock()
                    .unwrap()
                    .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(500));
                if !recent && popup_covered(&w, true) {
                    let _ = w.hide();
                }
            })
            .is_err()
        {
            app.state::<PopupGuard>()
                .pending
                .store(false, Ordering::Release);
        }
    });
}
fn show_popup(window: &tauri::WebviewWindow, point: Option<(f64, f64)>) {
    *window.state::<PopupGuard>().last_show.lock().unwrap() = Some(std::time::Instant::now());
    let _ = window.set_shadow(false);
    let _ = apply_topmost(
        window,
        window
            .state::<Engine>()
            .snapshot()
            .settings
            .prefs
            .always_on_top,
    );
    enforce_tray_style(window);
    position_popup(window, point);
    let _ = window.show();
    enforce_tray_style(window);
    let _ = round_window(window);
    let _ = window.set_focus();
}
#[tauri::command]
fn set_mode(
    window: tauri::WebviewWindow,
    expanded: bool,
    detail: Option<bool>,
) -> Result<(), String> {
    let (w, h) = if expanded {
        (930., 740.)
    } else if detail.unwrap_or(false) {
        (600., 560.)
    } else {
        (600., 420.)
    };
    window
        .set_size(tauri::LogicalSize::new(w, h))
        .map_err(|e| e.to_string())?;
    apply_topmost(
        &window,
        window
            .state::<Engine>()
            .snapshot()
            .settings
            .prefs
            .always_on_top,
    )?;
    position_popup(&window, None);
    let _ = round_window(&window);
    Ok(())
}
#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}
#[cfg(windows)]
fn send_notification(app: &tauri::AppHandle, success: bool, test: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::*;
    let tray = app
        .tray_by_id("quickclip321-tray")
        .ok_or("Tray unavailable")?;
    let delivered = tray
        .with_inner_tray_icon(move |icon| unsafe {
            let mut n: NOTIFYICONDATAW = std::mem::zeroed();
            n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            n.hWnd = icon.window_handle() as _;
            n.uFlags = NIF_INFO;
            n.dwInfoFlags = if success { NIIF_INFO } else { NIIF_ERROR };
            let title = if test {
                "QuickClip321 test"
            } else if success {
                "Download completed"
            } else {
                "Download failed"
            };
            let body = if test {
                "Windows notifications are enabled."
            } else if success {
                "Your media is ready. Open QuickClip321 to view it."
            } else {
                "Open QuickClip321 to see the error and retry."
            };
            for (dst, src) in n.szInfoTitle.iter_mut().zip(title.encode_utf16()) {
                *dst = src;
            }
            for (dst, src) in n.szInfo.iter_mut().zip(body.encode_utf16()) {
                *dst = src;
            }
            // tray-icon 0.25 uses process-local sequential numeric IDs. The HWND is our own
            // tray's HWND, never another app's. NIM_MODIFY rejects non-existent IDs.
            (1..=16).any(|id| {
                n.uID = id;
                Shell_NotifyIconW(NIM_MODIFY, &n) != 0
            })
        })
        .map_err(|e| e.to_string())?;
    if delivered {
        Ok(())
    } else {
        Err("Windows rejected the tray notification.".into())
    }
}
#[cfg(not(windows))]
fn send_notification(_: &tauri::AppHandle, _: bool, _: bool) -> Result<(), String> {
    Err("Notifications require Windows.".into())
}
fn watch_tray_progress(app: tauri::AppHandle) {
    let Some(base) = app.default_window_icon().cloned() else {
        return;
    };
    let raw = base.rgba().to_vec();
    let (w, h) = (base.width(), base.height());
    std::thread::spawn(move || {
        let mut last = String::new();
        let mut last_step = -2;
        loop {
            let (active, queued, paused, avg, speeds, lang) = app.state::<Engine>().tray_summary();
            let percent = (avg * 100.).round() as i32;
            let tr = lang == "tr";
            let mut label = if active == 0 && queued == 0 && paused == 0 {
                "QuickClip321".to_string()
            } else if tr {
                format!("QuickClip321 | \u{0130}ndiriliyor: {active} | S\u{0131}rada: {queued} | Bekliyor: {paused} | %{percent}")
            } else {
                format!("QuickClip321 | Active: {active} | Queued: {queued} | Paused: {paused} | {percent}%")
            };
            if !speeds.is_empty() {
                label.push_str(" | ");
                label.push_str(&speeds);
            }
            let label = label.chars().take(120).collect::<String>();
            let step = if active == 0 { -1 } else { percent / 5 };
            if let Some(tray) = app.tray_by_id("quickclip321-tray") {
                if label != last {
                    let _ = tray.set_tooltip(Some(&label));
                    last = label;
                }
                if step != last_step {
                    let mut pixels = raw.clone();
                    if step >= 0 && w >= 16 && h >= 16 {
                        let cx = (w as f64 - 1.) / 2.;
                        let cy = (h as f64 - 1.) / 2.;
                        let radius = cx.min(cy) - 1.;
                        for y in 0..h {
                            for x in 0..w {
                                let dx = x as f64 - cx;
                                let dy = y as f64 - cy;
                                let d = (dx * dx + dy * dy).sqrt();
                                if (d - radius).abs() <= (w as f64 / 24.).max(0.8) {
                                    let angle = dx.atan2(-dy).rem_euclid(std::f64::consts::TAU)
                                        / std::f64::consts::TAU;
                                    let i = ((y * w + x) * 4) as usize;
                                    pixels[i..i + 4].copy_from_slice(
                                        if angle <= step as f64 / 20. {
                                            &[171, 123, 255, 255]
                                        } else {
                                            &[75, 67, 95, 255]
                                        },
                                    );
                                }
                            }
                        }
                    }
                    if tray
                        .set_icon(Some(tauri::image::Image::new_owned(pixels, w, h)))
                        .is_ok()
                    {
                        last_step = step;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });
}
fn watch_notifications(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let notices = app.state::<Engine>().take_notifications();
        if !notices.is_empty() {
            let tr = app.state::<Engine>().tray_summary().5 == "tr";
            let completed = notices.iter().filter(|x| **x).count();
            let failed = notices.len() - completed;
            let message = if notices.len() == 1 {
                match (tr, completed == 1) {
                    (true, true) => "\u{0130}ndirme tamamland\u{0131}",
                    (true, false) => "\u{0130}ndirme ba\u{015f}ar\u{0131}s\u{0131}z",
                    (false, true) => "Download completed",
                    (false, false) => "Download failed",
                }
                .to_string()
            } else if tr {
                format!("{completed} tamamland\u{0131} | {failed} ba\u{015f}ar\u{0131}s\u{0131}z")
            } else {
                format!("{completed} completed | {failed} failed")
            };
            let h = app.clone();
            let _ = app.run_on_main_thread(move || show_shortcut_toast(&h, &message));
            if let Some(dir) = std::env::var_os("QC_V3_BENCH_DIR") {
                let success = failed == 0;
                let result = send_notification(&app, success, false);
                let _ = std::fs::write(
                    PathBuf::from(dir).join("notification-result.txt"),
                    format!("success={success};shell_accepted={}", result.is_ok()),
                );
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(350));
    });
}

mod signed_update;
#[tauri::command]
async fn tool_versions(engine: State<'_, Engine>) -> Result<String, String> {
    let settings = engine.snapshot().settings;
    tauri::async_runtime::spawn_blocking(move||{
        let y=quickclip_core::tool_version(&settings.yt_dlp,"--version").unwrap_or_else(||"Unavailable".into());
        let f=quickclip_core::tool_version(&settings.ffmpeg,"-version").unwrap_or_else(||"Unavailable".into());
        let n=quickclip_core::managed_node_path(&settings.yt_dlp).unwrap_or_else(||find_tool("node.exe"));
        let node=quickclip_core::tool_version(&n,"--version").unwrap_or_else(||"Unavailable".into());
        let probe=quickclip_core::tool_version(&settings.ffmpeg.with_file_name("ffprobe.exe"),"-version").unwrap_or_else(||"Unavailable".into());
        format!("yt-dlp: {y}\nFFmpeg: {f}\nFFprobe: {probe}\nNode: {node}\nVerified recommended: yt-dlp 2026.11.23 / FFmpeg 9.1 / Node 24.22.3")
    }).await.map_err(|_|"Tool version check failed".into())
}
#[tauri::command]
async fn update_ytdlp(engine: State<'_, Engine>) -> Result<Settings, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.install_recommended_ytdlp())
        .await
        .map_err(|_| "Tool update failed".to_string())?
}
#[tauri::command]
fn rollback_ytdlp(engine: State<'_, Engine>) -> Result<Settings, String> {
    engine.rollback_ytdlp()
}

mod update_check;
#[derive(Default)]
struct UpdateRuntime {
    ready: Mutex<Option<signed_update::Staged>>,
    busy: std::sync::atomic::AtomicBool,
    notice: Mutex<String>,
}
#[derive(serde::Serialize)]
struct UpdateStatus {
    ready: Option<signed_update::ReadyInfo>,
    busy: bool,
    message: String,
}
#[tauri::command]
fn update_status(state: State<'_, UpdateRuntime>) -> UpdateStatus {
    UpdateStatus {
        ready: state.ready.lock().unwrap().as_ref().map(|x| x.info()),
        busy: state.busy.load(std::sync::atomic::Ordering::Acquire),
        message: state.notice.lock().unwrap().clone(),
    }
}
fn stage_for_channel(
    app: &tauri::AppHandle,
    channel: &str,
    explicit_switch: bool,
) -> Result<signed_update::ReadyInfo, String> {
    use std::sync::atomic::Ordering;
    let r = app.state::<UpdateRuntime>();
    if r.busy.swap(true, Ordering::AcqRel) {
        return Err("Update operation already running".into());
    }
    let result = (|| {
        if app
            .state::<Engine>()
            .snapshot()
            .settings
            .prefs
            .update_channel
            != channel
        {
            return Err("Selected channel changed; check again".into());
        }
        let i = update_check::check(channel)?;
        if !i.signed_available || (!explicit_switch && i.switch_available) {
            return Err("No eligible signed update".into());
        }
        let target = i
            .latest_in_channel
            .ok_or("No release on selected channel")?;
        *r.notice.lock().unwrap() = "Downloading and verifying signed update".into();
        let staged = signed_update::stage(&target, channel)?;
        if app
            .state::<Engine>()
            .snapshot()
            .settings
            .prefs
            .update_channel
            != channel
        {
            return Err("Selected channel changed; check again".into());
        }
        let info = staged.info();
        *r.ready.lock().unwrap() = Some(staged);
        let handle = app.clone();
        let tr = app.state::<Engine>().tray_summary().5 == "tr";
        let _ = app.run_on_main_thread(move || {
            show_shortcut_toast(
                &handle,
                if tr {
                    "G\u{00fc}ncelleme haz\u{0131}r"
                } else {
                    "Update ready to install"
                },
            )
        });
        Ok(info)
    })();
    *r.notice.lock().unwrap() = if result.is_ok() {
        "Verified update ready".into()
    } else {
        "Update not staged; check release and retry".into()
    };
    r.busy.store(false, Ordering::Release);
    result
}
#[tauri::command]
async fn stage_signed_update(
    app: tauri::AppHandle,
    channel: String,
) -> Result<signed_update::ReadyInfo, String> {
    if isolated_run() {
        return Err("Network updates disabled in fixture mode".into());
    }
    tauri::async_runtime::spawn_blocking(move || stage_for_channel(&app, &channel, true))
        .await
        .map_err(|_| "Update staging failed".to_string())?
}
fn install_ready(app: &tauri::AppHandle, explicit: bool) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if isolated_run() {
        return Err("Installation disabled in fixture mode".into());
    }
    let r = app.state::<UpdateRuntime>();
    if r.busy.swap(true, Ordering::AcqRel) {
        return Err("Update operation already running".into());
    }
    let result = (|| {
        let engine = app.state::<Engine>();
        let ready = r.ready.lock().unwrap();
        let staged = ready.as_ref().ok_or("No verified update ready")?;
        let info = staged.info();
        let prefs = engine.snapshot().settings.prefs;
        if app.state::<ExitPending>().0.load(Ordering::Acquire) {
            return Err("Exit already scheduled".into());
        }
        if info.channel != prefs.update_channel {
            return Err("Selected channel changed; check again".into());
        }
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|_| "Invalid current version")?;
        let target = semver::Version::parse(&info.version).map_err(|_| "Invalid target version")?;
        let current_nightly = current.pre.as_str().split('.').any(|p| p == "nightly");
        if !explicit
            && (target <= current
                || (info.channel == "nightly") != current_nightly
                || prefs.update_mode != "automatic")
        {
            return Err("Automatic channel switching is disabled".into());
        }
        if target <= current && (info.channel == "nightly") == current_nightly {
            return Err("Downgrade on the same channel refused".into());
        }
        let state = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "State directory unavailable")?
            .join("state.json");
        // Acquire the queue lock before starting the replacement helper.
        engine.begin_idle_update()?;
        if let Err(e) = signed_update::install(staged, &state) {
            engine.abort_safe_exit();
            return Err(e);
        }
        app.exit(0);
        Ok(())
    })();
    if result.is_err() {
        r.busy.store(false, Ordering::Release);
    }
    result
}
#[tauri::command]
fn install_signed_update(app: tauri::AppHandle) -> Result<(), String> {
    install_ready(&app, true)
}
#[tauri::command]
fn startup_health() -> Result<(), String> {
    signed_update::report_health()
}
fn watch_updates(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(15));
        let mut last_check = None::<std::time::Instant>;
        let mut last_channel = String::new();
        let mut interval = std::time::Duration::from_secs(86400);
        loop {
            let engine = app.state::<Engine>();
            let prefs = engine.snapshot().settings.prefs;
            let (a, q, p, _, _, _) = engine.tray_summary();
            let idle = a == 0 && q == 0 && p == 0;
            let due = last_channel != prefs.update_channel
                || last_check.is_none_or(|v| v.elapsed() >= interval);
            if idle && due {
                last_channel = prefs.update_channel.clone();
                last_check = Some(std::time::Instant::now());
                interval = std::time::Duration::from_secs(300);
                if let Ok(i) = update_check::check(&prefs.update_channel) {
                    interval = std::time::Duration::from_secs(86400);
                    if i.signed_available && !i.switch_available && i.available.is_some() {
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            show_shortcut_toast(&handle, "Update available")
                        });
                        if prefs.update_mode != "manual" {
                            if stage_for_channel(&app, &prefs.update_channel, false).is_err() {
                                interval = std::time::Duration::from_secs(300);
                            }
                        }
                    }
                }
            }
            if idle
                && prefs.update_mode == "automatic"
                && app.state::<UpdateRuntime>().ready.lock().unwrap().is_some()
            {
                let h = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let _ = install_ready(&h, false);
                });
            }
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    });
}

#[tauri::command]
async fn download_public_beta(app: tauri::AppHandle) -> Result<String, String> {
    if isolated_run() {
        return Err("Download disabled in fixture mode".into());
    }
    let destination = app
        .path()
        .download_dir()
        .map_err(|_| "Downloads folder unavailable")?
        .join("QuickClip321-Releases");
    tauri::async_runtime::spawn_blocking(move||{
        use sha2::{Digest,Sha256};
        let expected="e2ecf9ddc7494c04c1279f914dff1295d86cca05788c62642317a156f886d7a6";
        std::fs::create_dir_all(&destination).map_err(|e|e.to_string())?;
        let target=destination.join("QuickClip321-3.0.0-beta.5-win-x64.exe");
        let verified=|p:&std::path::Path|->bool{
            std::fs::metadata(p).ok().is_some_and(|m|m.len()==12_628_992)&&std::fs::read(p).ok().is_some_and(|b|Sha256::digest(b).iter().map(|n|format!("{n:02x}")).collect::<String>()==expected)
        };
        if !target.exists(){
            let tmp=tempfile::NamedTempFile::new_in(&destination).map_err(|e|e.to_string())?;
            let system=std::env::var_os("SystemRoot").ok_or("Windows directory unavailable")?;
            let mut cmd=std::process::Command::new(PathBuf::from(system).join("System32/curl.exe"));
            #[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
            let status=cmd.args(["-q","--silent","--show-error","--fail","--location","--max-redirs","5","--proto","=https","--proto-redir","=https","--connect-timeout","10","--max-time","120","--max-filesize","12628992","--output"]).arg(tmp.path()).arg("https://github.com/Tuna321/QuickClip321/releases/download/v3.0.0-beta.5/QuickClip321-3.0.0-beta.5-win-x64.exe").status().map_err(|_|"Download unavailable")?;
            if !status.success()||!verified(tmp.path()){return Err("Release download failed verification".into());}
            tmp.persist_noclobber(&target).map_err(|_|"Could not save verified release")?;
        }
        if !verified(&target){return Err("Existing release file has unexpected contents; not overwritten".into());}
        reveal_folder(&destination)?;
        Ok("Verified public Beta 5 saved. Exit Nightly before launching it manually.".into())
    }).await.map_err(|_|"Release download failed".to_string())?
}
#[tauri::command]
async fn check_app_updates(channel: String) -> Result<update_check::UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(move || update_check::check(&channel))
        .await
        .map_err(|_| "Update check failed".to_string())?
}
#[tauri::command]
fn copy_diagnostics(app: tauri::AppHandle, engine: State<'_, Engine>) -> Result<(), String> {
    let report = diagnostics(engine);
    app.clipboard()
        .write_text(report)
        .map_err(|_| "Could not copy diagnostic report".into())
}
#[tauri::command]
fn export_preferences(engine: State<'_, Engine>) -> Result<String, String> {
    let mut p =
        serde_json::to_value(engine.snapshot().settings.prefs).map_err(|e| e.to_string())?;
    let obj = p.as_object_mut().ok_or("Invalid preferences")?;
    for key in [
        "last_destination",
        "category_paths",
        "last_organize",
        "previous_ytdlp",
    ] {
        obj.remove(key);
    }
    serde_json::to_string_pretty(&serde_json::json!({"schema":1,"preferences":p}))
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn import_preferences(
    window: tauri::WebviewWindow,
    engine: State<'_, Engine>,
    data: String,
) -> Result<(), String> {
    if data.len() > 262144 {
        return Err("Settings backup is too large".into());
    }
    let v: serde_json::Value =
        serde_json::from_str(&data).map_err(|_| "Invalid settings backup")?;
    if v["schema"].as_u64() != Some(1) {
        return Err("Unsupported settings backup".into());
    }
    let mut p = engine.snapshot().settings;
    let mut current = serde_json::to_value(&p.prefs).map_err(|e| e.to_string())?;
    let target = current.as_object_mut().ok_or("Invalid preferences")?;
    let source = v["preferences"]
        .as_object()
        .ok_or("Invalid settings backup")?;
    for (k, value) in source {
        if target.contains_key(k)
            && !matches!(
                k.as_str(),
                "last_destination"
                    | "category_paths"
                    | "last_organize"
                    | "run_on_startup"
                    | "update_channel"
                    | "update_mode"
                    | "previous_ytdlp"
            )
        {
            target.insert(k.clone(), value.clone());
        }
    }
    p.prefs = serde_json::from_value(current).map_err(|_| "Invalid settings values")?;
    save_settings(window, engine, p)
}
fn main() {
    let builder = tauri::Builder::default();
    let builder = if isolated_run() {
        // Test copies must not hand off to the user's running app or use its
        // WebView profile. Normal launches retain the single-instance guard.
        if let Some(root) =
            std::env::var_os("QC_V3_SMOKE_DIR").or_else(|| std::env::var_os("QC_V3_BENCH_DIR"))
        {
            let root = PathBuf::from(root);
            if root.is_absolute() && root.is_dir() {
                std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", root.join("webview-profile"));
            }
        }
        builder
    } else {
        builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                show_popup(&w, None);
            }
        }))
    };
    builder.plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let (data, destination) = if let Some(dir) =
                std::env::var_os("QC_V3_SMOKE_DIR").or_else(|| std::env::var_os("QC_V3_BENCH_DIR"))
            {
                let root = PathBuf::from(dir);
                if !root.is_absolute() || !root.is_dir() {
                    return Err(std::io::Error::other("Smoke directory must already exist").into());
                }
                (root.join("state"), root.join("media"))
            } else {
                (
                    app.path().app_local_data_dir()?,
                    app.path().download_dir()?.join("QuickClip321 V3"),
                )
            };
            let engine = Engine::open(
                data.join("state.json"),
                Settings {
                    destination,
                    yt_dlp: find_tool("yt-dlp.exe"),
                    ffmpeg: find_tool("ffmpeg.exe"),
                    prefs: Default::default(),
                },
            )
            .map_err(std::io::Error::other)?;
            let needs_setup = !configured_tools_ready(&engine.snapshot().settings);
            if let Err(e) = sync_startup(engine.snapshot().settings.prefs.run_on_startup) {
                eprintln!("{e}");
            }
            if !needs_setup && !isolated_run() {
                let _ = engine.resume_saved_queue();
            }
            app.manage(engine);
            app.manage(PopupGuard::default());
            app.manage(HotkeyStatus::default());
            app.manage(CaptureUntil::default());
            app.manage(TestUntil::default());
            let smoke = std::env::var_os("QC_V3_SMOKE_DIR").is_some();
            app.manage(ClipboardConsumption::default());
            app.manage(DuplicateInbox::default());
            app.manage(ExitPending::default());
            app.manage(UpdateRuntime::default());
            app.manage(ClipboardInbox(Mutex::new(if smoke {
                Some("https://example.com/fixture".into())
            } else {
                None
            })));
            if std::env::var_os("QC_V3_SMOKE_DIR").is_none()
                && std::env::var_os("QC_V3_BENCH_DIR").is_none()
            {
                watch_clipboard(app.handle().clone());
            }
            let show = MenuItem::with_id(app, "show", "Show QuickClip321", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let cancel_exit=MenuItem::with_id(app,"cancel_exit","Cancel scheduled exit",true,None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit,&cancel_exit])?;
            let mut tray = TrayIconBuilder::with_id("quickclip321-tray")
                .menu(&menu)
                .tooltip("QuickClip321")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            show_popup(&w, None);
                        }
                    }
                    "quit" => {
                        use std::sync::atomic::Ordering;
                        let pending=app.state::<ExitPending>();
                        if pending.0.swap(true,Ordering::AcqRel){return;}
                        match app.state::<Engine>().prepare_safe_exit(){
                            Ok(())=>{
                                let handle=app.clone();
                                let tr=app.state::<Engine>().tray_summary().5=="tr";
                                show_shortcut_toast(app,if tr{"Kuyruk korunuyor; g\u{00fc}venli \u{00e7}\u{0131}k\u{0131}\u{015f}"}else{"Saving queue; exiting safely"});
                                std::thread::spawn(move||{
                                    // Unsupported active jobs may finish normally. Do not interrupt processing.
                                    while handle.state::<ExitPending>().0.load(Ordering::Acquire) {
                                        if handle.state::<Engine>().safe_exit_ready(){
                                            let h=handle.clone();
                                            let _=handle.run_on_main_thread(move||{
                                                if h.state::<ExitPending>().0.load(Ordering::Acquire){h.exit(0);}
                                            });
                                            return;
                                        }
                                        std::thread::sleep(std::time::Duration::from_millis(500));
                                    }
                                });
                            }
                            Err(e)=>{
                                pending.0.store(false,Ordering::Release);
                                show_shortcut_toast(app,&e);
                                if let Some(w)=app.get_webview_window("main"){show_popup(&w,None);}
                            }
                        }
                    }
                    "cancel_exit"=>{
                        use std::sync::atomic::Ordering;
                        if app.state::<ExitPending>().0.swap(false,Ordering::AcqRel){
                            app.state::<Engine>().abort_safe_exit();
                            show_shortcut_toast(app,"Exit cancelled; resume held jobs in Queue");
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            toggle_popup(&w, Some((position.x, position.y)));
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            #[cfg(windows)]
            if !isolated_run() || std::env::var_os("QC_SHORTCUT_SMOKE").is_some() {
                watch_hotkey(app.handle().clone());
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = apply_topmost(
                    &w,
                    app.state::<Engine>()
                        .snapshot()
                        .settings
                        .prefs
                        .always_on_top,
                );
            }
            if needs_setup && !isolated_run() {
                if let Some(w) = app.get_webview_window("main") {
                    show_popup(&w, None);
                }
            }
            if !smoke {
                watch_popup(app.handle().clone());
                watch_tray_progress(app.handle().clone());
                if !isolated_run(){watch_updates(app.handle().clone());}
            }
            if !smoke || std::env::var_os("QC_COMPLETION_SMOKE").is_some() {
                watch_notifications(app.handle().clone());
            }
            // Explicit isolated fixture only; normal users never auto-enqueue.
            if let (Some(_), Ok(url)) = (
                std::env::var_os("QC_V3_BENCH_DIR"),
                std::env::var("QC_NOTIFICATION_FIXTURE"),
            ) {
                let engine = app.state::<Engine>();
                let settings = engine.snapshot().settings;
                engine
                    .enqueue(Request {
                        url,
                        preset: "original".into(),
                        quality: "best".into(),
                        playlist: false,
                        organize: false,
                        options: Default::default(),
                        destination: settings.destination,
                    })
                    .map_err(std::io::Error::other)?;
            }
            if isolated_run() && std::env::var_os("QC_SHORTCUT_DOWNLOAD_FIXTURE").is_some() {
                run_shortcut(app.handle(), "download");
            }
            if let Some(dir) = std::env::var_os("QC_V3_SMOKE_DIR") {
                if let Some(w) = app.get_webview_window("main") {
                    let hidden = !w.is_visible().unwrap_or(true);
                    std::fs::write(
                        PathBuf::from(dir).join("startup-hidden.txt"),
                        hidden.to_string(),
                    )?;
                    show_popup(&w, None);
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(
                event,
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                if let Some(w) = window.app_handle().get_webview_window("main") {
                    let _ = round_window(&w);
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            visual_page,
            check_app_updates,
            tool_versions,
            update_ytdlp,
            rollback_ytdlp,
            stage_signed_update,
            install_signed_update,
            update_status,
            startup_health,
            download_public_beta,
            export_preferences,
            import_preferences,
            smoke_active,
            take_clipboard_link,
            take_submitted_links,
            take_duplicate_requests,
            media_preview,
            live_preview_frame,
            validate_links,
            queue_action,
            snapshot,
            enqueue,
            cancel,
            retry,
            pause_waiting,
            resume_waiting,
            detect_tools,
            tools_ready,
            setup_managed_tools,
            setup_progress,
            diagnostics,
            copy_diagnostics,
            organizer_counts,
            remove,
            save_settings,
            preview_position,
            open_folder,
            open_downloads,
            hotkey_status,
            shortcut_capture,
            shortcut_test,
            shortcut_hit,
            remember_download,
            set_mode,
            hide_window,
            persist_fixture_report,
            ui_ready
        ])
        .run(tauri::generate_context!())
        .expect("QuickClip321 startup failed");
}

#[cfg(test)]
mod popup_tests {
    use super::rect_overlap;
    #[test]
    fn overlap_geometry() {
        assert!(rect_overlap((0, 0, 100, 100), (50, 50, 150, 150)));
        assert!(!rect_overlap((0, 0, 100, 100), (100, 0, 200, 100)));
        assert!(!rect_overlap((-100, -100, 0, 0), (0, 0, 100, 100)));
    }
}
