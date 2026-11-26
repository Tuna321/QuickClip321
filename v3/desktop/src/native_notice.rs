// A native, short-lived, non-activating notification. No WebView or persistent
// transparent desktop window. Only an active notification owns an HWND.
use std::{
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};
struct Notice {
    text: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    dpi: f64,
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
const CLASS: &str = "QuickClip321.NativeNotice";
static TX: OnceLock<mpsc::Sender<Notice>> = OnceLock::new();
pub fn show(text: &str, x: i32, y: i32, w: i32, h: i32, dpi: f64) {
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || run(rx));
        tx
    });
    let _ = tx.send(Notice {
        text: text.into(),
        x,
        y,
        w,
        h,
        dpi,
    });
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut r);
            let bg = CreateSolidBrush(0x00291d1e);
            FillRect(dc, &r, bg);
            DeleteObject(bg as _);
            let h = r.bottom - r.top;
            let dot = CreateSolidBrush(0x00ffa6bf);
            let old = SelectObject(dc, dot as _);
            let pen = SelectObject(dc, GetStockObject(NULL_PEN));
            Ellipse(dc, h / 3, h / 2 - h / 12, h / 3 + h / 6, h / 2 + h / 12);
            SelectObject(dc, pen);
            SelectObject(dc, old);
            DeleteObject(dot as _);
            let font = CreateFontW(
                -(h * 29 / 100),
                0,
                0,
                0,
                600,
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
            let old_font = SelectObject(dc, font as _);
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, 0x00fff8fa);
            let mut buf = [0u16; 256];
            let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            r.left = h * 3 / 4;
            r.right -= h / 4;
            DrawTextW(
                dc,
                buf.as_ptr(),
                n,
                &mut r,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
            SelectObject(dc, old_font);
            DeleteObject(font as _);
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
fn smoke_event(name: &str, text: &str) {
    if std::env::var_os("QC_SHORTCUT_SMOKE").is_some() {
        if let Some(dir) = std::env::var_os("QC_V3_SMOKE_DIR") {
            let _ = std::fs::write(std::path::PathBuf::from(dir).join(name), text);
        }
    }
}
fn run(rx: mpsc::Receiver<Notice>) {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide(CLASS);
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let mut hwnd: HWND = std::ptr::null_mut();
        let mut shown = Instant::now();
        loop {
            let next = if hwnd.is_null() {
                match rx.recv() {
                    Ok(n) => Some(n),
                    Err(_) => break,
                }
            } else {
                match rx.recv_timeout(Duration::from_millis(16)) {
                    Ok(n) => Some(n),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(_) => {
                        DestroyWindow(hwnd);
                        break;
                    }
                }
            };
            if let Some(mut n) = next {
                // If shortcuts arrive in a burst, show the most recent message.
                while let Ok(new) = rx.try_recv() {
                    n = new;
                }
                if !hwnd.is_null() {
                    DestroyWindow(hwnd);
                }
                let before = GetForegroundWindow();
                hwnd = CreateWindowExW(
                    WS_EX_TOPMOST
                        | WS_EX_TOOLWINDOW
                        | WS_EX_NOACTIVATE
                        | WS_EX_TRANSPARENT
                        | WS_EX_LAYERED,
                    class.as_ptr(),
                    wide(&n.text).as_ptr(),
                    WS_POPUP,
                    n.x,
                    n.y,
                    n.w,
                    n.h,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    continue;
                }
                let radius = (24. * n.dpi).round() as i32;
                let region = CreateRoundRectRgn(0, 0, n.w + 1, n.h + 1, radius, radius);
                if !region.is_null() && SetWindowRgn(hwnd, region, 0) == 0 {
                    DeleteObject(region as _);
                }
                SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                UpdateWindow(hwnd);
                shown = Instant::now();
                smoke_event(
                    "notice-shown.txt",
                    if before == GetForegroundWindow() {
                        "visible; focus unchanged"
                    } else {
                        "focus changed"
                    },
                );
            }
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if !hwnd.is_null() {
                let t = shown.elapsed().as_secs_f64();
                if t >= 1.65 {
                    let old = hwnd;
                    DestroyWindow(old);
                    hwnd = std::ptr::null_mut();
                    smoke_event(
                        "notice-destroyed.txt",
                        if IsWindow(old) == 0 {
                            "destroyed"
                        } else {
                            "still exists"
                        },
                    );
                } else {
                    let alpha = (t / 0.18).min(1.).min((1.65 - t) / 0.28).clamp(0., 1.);
                    SetLayeredWindowAttributes(hwnd, 0, (alpha * 255.) as u8, LWA_ALPHA);
                }
            }
        }
    }
}
