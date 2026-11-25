#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use quickclip_core::{Engine, Request, Settings, Snapshot};
use std::path::PathBuf;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};
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
    if !job.destination.is_dir() {
        return Err("Folder unavailable.".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(job.destination)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn ui_ready(app: tauri::AppHandle, passed: bool) -> Result<(), String> {
    if let Some(dir) = std::env::var_os("QC_V3_SMOKE_DIR") {
        let root = PathBuf::from(dir);
        let report = if passed {
            "{\"ui_ready\":true,\"snapshot\":true}"
        } else {
            "{\"ui_ready\":false,\"snapshot\":false}"
        };
        std::fs::write(root.join("ui-smoke.json"), report).map_err(|e| e.to_string())?;
        app.exit(if passed { 0 } else { 1 });
    }
    Ok(())
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
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
                    yt_dlp: PathBuf::new(),
                    ffmpeg: PathBuf::new(),
                },
            )
            .map_err(std::io::Error::other)?;
            app.manage(engine);
            let show = MenuItem::with_id(app, "show", "Show QuickClip321", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("QuickClip321")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
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
                                let _ = w.show();
                                let _ = w.set_focus();
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
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            enqueue,
            cancel,
            retry,
            remove,
            save_settings,
            open_folder,
            ui_ready
        ])
        .run(tauri::generate_context!())
        .expect("QuickClip321 startup failed");
}
