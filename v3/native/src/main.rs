#![windows_subsystem = "windows"]
// Native frontend feasibility candidate. No webview. Separate test state.
use quickclip_core::{is_playlist, validate_url, Engine, Request, Settings, Status};
use std::{
    cell::RefCell,
    path::PathBuf,
    ptr::{null, null_mut},
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{DataExchange::*, LibraryLoader::*, Memory::*, Threading::*},
    UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
};
const TRAY: u32 = WM_APP + 1;
const URL: i32 = 101;
const PRESET: i32 = 102;
const GO: i32 = 103;
const INFO: i32 = 104;
const PLAYLIST: i32 = 105;
const QUEUE: i32 = 106;
const LIST: i32 = 107;
const QUIT: i32 = 108;
const PASTE: i32 = 109;
const DEST: i32 = 110;
struct Ui {
    engine: Engine,
    hwnd: HWND,
    url: HWND,
    preset: HWND,
    info: HWND,
    playlist: HWND,
    list: HWND,
    queue: HWND,
    tray: NOTIFYICONDATAW,
    expanded: bool,
    last_clip: u32,
    auto_text: String,
    pending: Option<String>,
    font: HFONT,
    shown: Instant,
}
thread_local! {static UI:RefCell<Option<Ui>>=const{RefCell::new(None)};}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
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
unsafe fn set(hwnd: HWND, s: &str) {
    SetWindowTextW(hwnd, wide(s).as_ptr());
}
unsafe fn get(hwnd: HWND) -> String {
    let mut v = vec![0; GetWindowTextLengthW(hwnd).max(0) as usize + 1];
    let n = GetWindowTextW(hwnd, v.as_mut_ptr(), v.len() as i32);
    String::from_utf16_lossy(&v[..n.max(0) as usize])
}
unsafe fn control(
    parent: HWND,
    class: &str,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: u32,
    font: HFONT,
) -> HWND {
    let hwnd = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        h,
        parent,
        id as usize as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1);
    hwnd
}
unsafe fn clipboard(hwnd: HWND) -> Option<String> {
    if OpenClipboard(hwnd) == 0 {
        return None;
    }
    let h = GetClipboardData(13);
    let mut out = None;
    if !h.is_null() {
        let p = GlobalLock(h) as *const u16;
        if !p.is_null() {
            let max = (GlobalSize(h) / 2).min(4097);
            let data = std::slice::from_raw_parts(p, max);
            if let Some(n) = data.iter().position(|&c| c == 0) {
                out = validate_url(String::from_utf16_lossy(&data[..n]).trim()).ok();
            }
            GlobalUnlock(h);
        }
    }
    CloseClipboard();
    out
}
unsafe fn update_playlist(u: &Ui) {
    let visible = is_playlist(get(u.url).trim());
    SendMessageW(u.playlist, BM_SETCHECK, 0, 0);
    ShowWindow(u.playlist, if visible { SW_SHOW } else { SW_HIDE });
}
unsafe fn placement(u: &Ui) {
    let mut p: POINT = std::mem::zeroed();
    GetCursorPos(&mut p);
    let mon = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
    let mut mi: MONITORINFO = std::mem::zeroed();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(mon, &mut mi) == 0 {
        return;
    }
    let dpi = GetDpiForWindow(u.hwnd).max(96);
    let scale = |n: i32| n * dpi as i32 / 96;
    let pref = u.engine.snapshot().settings.prefs;
    let (x, y) = quickclip_core::preferences::popup_position(
        quickclip_core::preferences::WorkArea {
            left: mi.rcWork.left,
            top: mi.rcWork.top,
            width: mi.rcWork.right - mi.rcWork.left,
            height: mi.rcWork.bottom - mi.rcWork.top,
        },
        (scale(480), scale(if u.expanded { 420 } else { 260 })),
        Some((p.x as f64, p.y as f64)),
        &pref,
        scale(8),
    );
    SetWindowPos(
        u.hwnd,
        HWND_TOPMOST,
        x,
        y,
        scale(480),
        scale(if u.expanded { 420 } else { 260 }),
        SWP_NOACTIVATE,
    );
}
fn clipboard_allowed(u: &Ui) -> bool {
    std::env::var_os("QC_NATIVE_TEST_DIR").is_none() && u.engine.clipboard_enabled()
}
unsafe fn show(u: &mut Ui) {
    if IsWindowVisible(u.hwnd) != 0 {
        ShowWindow(u.hwnd, SW_HIDE);
        return;
    }
    if clipboard_allowed(u) {
        if let Some(v) = clipboard(u.hwnd) {
            let old = get(u.url);
            if old.is_empty() || old == u.auto_text {
                set(u.url, &v);
                u.auto_text = v;
            } else if old != v {
                u.pending = Some(v);
                set(u.info, "New copied link available — Paste to replace.");
            }
        }
    }
    update_playlist(u);
    placement(u);
    ShowWindow(u.hwnd, SW_SHOW);
    SetForegroundWindow(u.hwnd);
    windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(u.url);
    u.shown = Instant::now();
}
unsafe fn refresh(u: &mut Ui) {
    let snap = u.engine.snapshot();
    let running = snap
        .jobs
        .iter()
        .filter(|j| matches!(j.status, Status::Running | Status::Queued))
        .count();
    if u.pending.is_none() {
        if let Some(j) = snap
            .jobs
            .iter()
            .rev()
            .find(|j| j.status == Status::Running)
            .or_else(|| snap.jobs.last())
        {
            set(u.info, &j.message);
        }
    }
    let title = format!("Queue ({running})");
    if get(u.queue) != title {
        set(u.queue, &title);
    }
    if u.expanded {
        let current = get(u.list);
        let next = snap
            .jobs
            .iter()
            .rev()
            .take(20)
            .map(|j| {
                format!(
                    "{:?} — {}% — {}",
                    j.status,
                    (j.progress * 100.) as u32,
                    j.message
                )
            })
            .collect::<Vec<_>>()
            .join("\r\n");
        if current != next {
            set(u.list, &next);
        }
    }
}
unsafe extern "system" fn wnd(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            return 0;
        }
        WM_DESTROY => {
            UI.with(|c| {
                if let Some(u) = c.borrow_mut().take() {
                    Shell_NotifyIconW(NIM_DELETE, &u.tray);
                    DeleteObject(u.font);
                }
            });
            PostQuitMessage(0);
            return 0;
        }
        WM_COMMAND | WM_TIMER | TRAY | WM_ACTIVATE | WM_DPICHANGED => {
            UI.with(|c| {
                let Ok(mut cell) = c.try_borrow_mut() else {
                    return;
                };
                let Some(u) = cell.as_mut() else { return };
                match msg {
                    WM_COMMAND => {
                        let id = (wp & 0xffff) as i32;
                        let code = (wp >> 16) & 0xffff;
                        match id {
                            URL if code == EN_CHANGE as usize => update_playlist(u),
                            PASTE => {
                                if let Some(v) = u.pending.take().or_else(|| {
                                    if std::env::var_os("QC_NATIVE_TEST_DIR").is_none() {
                                        clipboard(hwnd)
                                    } else {
                                        None
                                    }
                                }) {
                                    set(u.url, &v);
                                    u.auto_text = v;
                                    set(u.info, "Ready");
                                    update_playlist(u);
                                }
                            }
                            GO => {
                                let raw = get(u.url);
                                let idx = SendMessageW(u.preset, CB_GETCURSEL, 0, 0);
                                let (preset, quality) = match idx {
                                    1 => ("original", "best"),
                                    2 => ("mp3", "best"),
                                    _ => ("editor", "1080"),
                                };
                                let s = u.engine.snapshot().settings;
                                let request = Request {
                                    url: raw,
                                    preset: preset.into(),
                                    quality: quality.into(),
                                    playlist: is_playlist(&get(u.url))
                                        && SendMessageW(u.playlist, BM_GETCHECK, 0, 0) == 1,
                                    organize: s.prefs.smart_organizer,
                                    options: Default::default(),
                                    destination: s.destination,
                                };
                                match u.engine.enqueue(request) {
                                    Ok(_) => {
                                        set(u.url, "");
                                        u.auto_text.clear();
                                        u.pending = None;
                                        set(u.info, "Added to queue");
                                    }
                                    Err(e) => {
                                        set(u.info, &e);
                                    }
                                }
                            }
                            QUEUE => {
                                u.expanded = !u.expanded;
                                ShowWindow(u.list, if u.expanded { SW_SHOW } else { SW_HIDE });
                                placement(u);
                                refresh(u);
                            }
                            DEST => {
                                let s = u.engine.snapshot().settings;
                                let _ = ShellExecuteW(
                                    hwnd,
                                    wide("open").as_ptr(),
                                    wide(&s.destination.to_string_lossy()).as_ptr(),
                                    null(),
                                    null(),
                                    SW_SHOWNORMAL,
                                );
                            }
                            QUIT => {
                                if u.engine
                                    .snapshot()
                                    .jobs
                                    .iter()
                                    .any(|j| matches!(j.status, Status::Running | Status::Queued))
                                {
                                    set(u.info, "Downloads active; wait before exiting.");
                                    ShowWindow(hwnd, SW_SHOW);
                                } else {
                                    PostMessageW(hwnd, WM_APP + 2, 0, 0);
                                }
                            }
                            _ => {}
                        }
                    }
                    TRAY => {
                        if lp as u32 == WM_LBUTTONUP {
                            show(u)
                        } else if lp as u32 == WM_RBUTTONUP {
                            let menu = CreatePopupMenu();
                            AppendMenuW(
                                menu,
                                MF_STRING,
                                QUIT as usize,
                                wide("Exit QuickClip321").as_ptr(),
                            );
                            let mut p: POINT = std::mem::zeroed();
                            GetCursorPos(&mut p);
                            SetForegroundWindow(hwnd);
                            TrackPopupMenu(menu, TPM_RIGHTBUTTON, p.x, p.y, 0, hwnd, null());
                            DestroyMenu(menu);
                        }
                    }
                    WM_TIMER => {
                        let seq = GetClipboardSequenceNumber();
                        if seq != u.last_clip {
                            u.last_clip = seq;
                            if clipboard_allowed(u) {
                                if let Some(v) = clipboard(hwnd) {
                                    let old = get(u.url);
                                    if old.is_empty() || old == u.auto_text {
                                        set(u.url, &v);
                                        u.auto_text = v;
                                        u.pending = None;
                                        update_playlist(u);
                                    } else if old != v {
                                        u.pending = Some(v);
                                        set(
                                            u.info,
                                            "New copied link available — Paste to replace.",
                                        );
                                    }
                                }
                            }
                        }
                        if IsWindowVisible(hwnd) != 0 {
                            refresh(u)
                        }
                    }
                    WM_ACTIVATE => {
                        if wp & 0xffff == 0 && !u.expanded && u.shown.elapsed().as_millis() > 250 {
                            ShowWindow(hwnd, SW_HIDE);
                        }
                    }
                    WM_DPICHANGED => {
                        placement(u);
                    }
                    _ => {}
                }
            });
            return 0;
        }
        _ => {}
    }
    if msg == WM_APP + 2 {
        DestroyWindow(hwnd);
        return 0;
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}
fn main() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex = CreateMutexW(
            null(),
            0,
            wide("Local\\QuickClip321NativeCandidate").as_ptr(),
        );
        if mutex.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        let root = std::env::var_os("QC_NATIVE_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
                    .join("QuickClip321NativeCandidate")
            });
        std::fs::create_dir_all(&root).unwrap();
        let destination = root.join("downloads");
        let _ = std::fs::create_dir_all(&destination);
        let engine = Engine::open(
            root.join("state.json"),
            Settings {
                destination,
                yt_dlp: find_tool("yt-dlp.exe"),
                ffmpeg: find_tool("ffmpeg.exe"),
                prefs: Default::default(),
            },
        )
        .unwrap();
        let instance = GetModuleHandleW(null());
        let class = wide("QuickClip321Native");
        let icon = LoadIconW(null_mut(), IDI_APPLICATION);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hIcon: icon,
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.as_ptr(),
            wide("QuickClip321").as_ptr(),
            WS_POPUP | WS_BORDER,
            0,
            0,
            480,
            260,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return;
        }
        let font = CreateFontW(
            -17,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe UI").as_ptr(),
        );
        control(hwnd, "STATIC", "QuickClip321", 0, 20, 18, 310, 28, 0, font);
        let queue = control(
            hwnd,
            "BUTTON",
            "Queue (0)",
            QUEUE,
            352,
            16,
            108,
            32,
            WS_TABSTOP,
            font,
        );
        let url = control(
            hwnd,
            "EDIT",
            "",
            URL,
            20,
            60,
            330,
            34,
            WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
            font,
        );
        control(
            hwnd, "BUTTON", "Paste", PASTE, 360, 60, 100, 34, WS_TABSTOP, font,
        );
        let preset = control(
            hwnd,
            "COMBOBOX",
            "",
            PRESET,
            20,
            108,
            330,
            130,
            WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
            font,
        );
        for s in [
            "Premiere Ready — MP4 1080p",
            "Best available — Original",
            "Audio — MP3",
        ] {
            SendMessageW(preset, CB_ADDSTRING, 0, wide(s).as_ptr() as LPARAM);
        }
        SendMessageW(preset, CB_SETCURSEL, 0, 0);
        control(
            hwnd,
            "BUTTON",
            "Download",
            GO,
            360,
            108,
            100,
            34,
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            font,
        );
        let playlist = control(
            hwnd,
            "BUTTON",
            "Entire playlist",
            PLAYLIST,
            20,
            151,
            210,
            26,
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            font,
        );
        ShowWindow(playlist, SW_HIDE);
        let info = control(hwnd, "STATIC", "Ready", INFO, 20, 183, 440, 36, 0, font);
        control(
            hwnd,
            "BUTTON",
            "Open downloads",
            DEST,
            20,
            220,
            170,
            28,
            WS_TABSTOP,
            font,
        );
        let list = control(
            hwnd,
            "EDIT",
            "",
            LIST,
            20,
            265,
            440,
            140,
            WS_BORDER | WS_VSCROLL | ES_MULTILINE as u32 | ES_READONLY as u32 | WS_TABSTOP,
            font,
        );
        ShowWindow(list, SW_HIDE);
        let mut tray: NOTIFYICONDATAW = std::mem::zeroed();
        tray.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        tray.hWnd = hwnd;
        tray.uID = 1;
        tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        tray.uCallbackMessage = TRAY;
        tray.hIcon = icon;
        let tip = wide("QuickClip321 Native");
        tray.szTip[..tip.len()].copy_from_slice(&tip);
        if Shell_NotifyIconW(NIM_ADD, &tray) == 0 {
            DestroyWindow(hwnd);
            return;
        }
        UI.with(|c| {
            *c.borrow_mut() = Some(Ui {
                engine,
                hwnd,
                url,
                preset,
                info,
                playlist,
                list,
                queue,
                tray,
                expanded: false,
                last_clip: GetClipboardSequenceNumber(),
                auto_text: String::new(),
                pending: None,
                font,
                shown: Instant::now(),
            })
        });
        SetTimer(hwnd, 1, 500, None);
        if std::env::var_os("QC_NATIVE_SHOW").is_some() {
            UI.with(|c| {
                if let Some(u) = c.borrow_mut().as_mut() {
                    show(u)
                }
            });
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_KEYDOWN && msg.wParam == 27 {
                ShowWindow(hwnd, SW_HIDE);
                continue;
            }
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        CloseHandle(mutex);
    }
}
