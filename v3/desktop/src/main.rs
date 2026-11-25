#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use quickclip_core::{Engine, Request, Settings, Snapshot};
use std::path::PathBuf;
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
    use quickclip_core::Status::*;
    for j in engine.snapshot().jobs {
        match action.as_str() {
            "cancel_all" if matches!(j.status, Queued | Running) => {
                let _ = engine.cancel(&j.id);
            }
            "clear_pending" if j.status == Queued => {
                let _ = engine.cancel(&j.id);
            }
            "clear_completed" if j.status == Completed => {
                engine.remove(&j.id)?;
            }
            "cancel_all" | "clear_pending" | "clear_completed" => {}
            _ => return Err("Unknown queue action.".into()),
        }
    }
    Ok(())
}
#[tauri::command]
fn snapshot(engine: State<'_, Engine>) -> Snapshot {
    engine.snapshot()
}
#[tauri::command]
fn enqueue(engine: State<'_, Engine>, request: Request) -> Result<String, String> {
    engine.enqueue(request)
}
#[tauri::command]
fn cancel(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    engine.cancel(&id)
}
#[tauri::command]
fn retry(engine: State<'_, Engine>, id: String) -> Result<String, String> {
    engine.retry(&id)
}
#[tauri::command]
fn remove(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    engine.remove(&id)
}
#[tauri::command]
fn save_settings(engine: State<'_, Engine>, settings: Settings) -> Result<(), String> {
    engine.settings(settings)
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
fn ui_ready(app: tauri::AppHandle, passed: bool) -> Result<(), String> {
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
        let ok = passed
            && started_hidden
            && frameless
            && topmost
            && tray
            && no_taskbar
            && within_workarea
            && hide_works
            && show_works;
        let report=format!("{{\"ui_ready\":{passed},\"snapshot\":{passed},\"started_hidden\":{started_hidden},\"frameless\":{frameless},\"topmost\":{topmost},\"tray\":{tray},\"no_taskbar\":{no_taskbar},\"within_workarea\":{within_workarea},\"hide_works\":{hide_works},\"show_works\":{show_works}}}");
        std::fs::write(root.join("ui-smoke.json"), report).map_err(|e| e.to_string())?;
        app.exit(if ok { 0 } else { 1 });
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
fn position_popup(window: &tauri::WebviewWindow, point: Option<(f64, f64)>) {
    let monitor = point
        .and_then(|(x, y)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    if let (Some(m), Ok(size)) = (monitor, window.outer_size()) {
        let area = m.work_area();
        let prefs = window
            .app_handle()
            .state::<Engine>()
            .snapshot()
            .settings
            .prefs;
        let (x, y) = quickclip_core::preferences::popup_position(
            quickclip_core::preferences::WorkArea {
                left: area.position.x,
                top: area.position.y,
                width: area.size.width as i32,
                height: area.size.height as i32,
            },
            (size.width as i32, size.height as i32),
            point,
            &prefs,
            (8.0 * m.scale_factor()).round() as i32,
        );
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
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
    enforce_tray_style(window);
    position_popup(window, point);
    let _ = window.show();
    enforce_tray_style(window);
    let _ = window.set_focus();
}
#[tauri::command]
fn set_mode(window: tauri::WebviewWindow, expanded: bool) -> Result<(), String> {
    let (w, h) = if expanded { (930., 720.) } else { (600., 395.) };
    window
        .set_size(tauri::LogicalSize::new(w, h))
        .map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    position_popup(&window, None);
    Ok(())
}
#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
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
            let (data, destination) = if let Some(dir) = std::env::var_os("QC_V3_SMOKE_DIR") {
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
            app.manage(engine);
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
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            media_preview,
            validate_links,
            queue_action,
            snapshot,
            enqueue,
            cancel,
            retry,
            remove,
            save_settings,
            open_folder,
            set_mode,
            hide_window,
            ui_ready
        ])
        .run(tauri::generate_context!())
        .expect("QuickClip321 startup failed");
}
