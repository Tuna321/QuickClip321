#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use quickclip_core::{Engine, Request, Settings, Snapshot};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri_plugin_clipboard_manager::ClipboardExt;
#[derive(Default)]
struct ClipboardInbox(Mutex<Option<String>>);
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
                            if let Ok(mut pending) = app.state::<ClipboardInbox>().0.lock() {
                                *pending = Some(link);
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
fn enqueue(engine: State<'_, Engine>, request: Request) -> Result<String, String> {
    if !configured_tools_ready(&engine.snapshot().settings) {
        return Err("QuickClip321 is not ready yet. Complete media setup first.".into());
    }
    engine.enqueue(request)
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
    tauri::async_runtime::spawn_blocking(move || engine.setup_managed_tools())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn setup_progress(engine: State<'_, Engine>) -> quickclip_core::SetupProgress {
    engine.setup_progress()
}
fn configured_tools_ready(settings: &Settings) -> bool {
    let managed_node = settings.yt_dlp.parent().is_some_and(|p| {
        p.file_name().and_then(|v| v.to_str()) == Some("managed-2026.11.23-9.1-24.22.3")
            && p.join("node.exe").is_file()
    });
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
    out.push_str("Privacy: report excludes URLs, file paths, clipboard and error logs.");
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
    if position_changed {
        position_popup(&window, None);
    }
    Ok(())
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
    let folder = job
        .files
        .first()
        .and_then(|p| p.parent())
        .unwrap_or(&job.destination);
    if !folder.is_dir() {
        return Err("Folder unavailable.".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(folder)
        .spawn()
        .map_err(|e| e.to_string())?;
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
fn show_popup(window: &tauri::WebviewWindow, point: Option<(f64, f64)>) {
    let _ = window.set_shadow(false);
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
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
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
fn watch_notifications(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        for success in app.state::<Engine>().take_notifications() {
            let result = send_notification(&app, success, false);
            if let Some(dir) = std::env::var_os("QC_V3_BENCH_DIR") {
                let _ = std::fs::write(
                    PathBuf::from(dir).join("notification-result.txt"),
                    format!("success={success};shell_accepted={}", result.is_ok()),
                );
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(350));
    });
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                show_popup(&w, None);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
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
            app.manage(engine);
            let smoke = std::env::var_os("QC_V3_SMOKE_DIR").is_some();
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
            let menu = Menu::with_items(app, &[&show, &quit])?;
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
                        let active = app.state::<Engine>().snapshot().jobs.iter().any(|j| {
                            matches!(
                                j.status,
                                quickclip_core::Status::Running | quickclip_core::Status::Queued
                            )
                        });
                        if active {
                            if let Some(w) = app.get_webview_window("main") {
                                show_popup(&w, None);
                            }
                        } else {
                            app.exit(0);
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
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                show_popup(&w, Some((position.x, position.y)));
                            }
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if needs_setup && !isolated_run() {
                if let Some(w) = app.get_webview_window("main") {
                    show_popup(&w, None);
                }
            }
            if !smoke {
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
            smoke_active,
            take_clipboard_link,
            media_preview,
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
            organizer_counts,
            remove,
            save_settings,
            preview_position,
            open_folder,
            set_mode,
            hide_window,
            ui_ready
        ])
        .run(tauri::generate_context!())
        .expect("QuickClip321 startup failed");
}
