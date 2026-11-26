"""Capture only the visible, foreground test candidate window."""
import ctypes as c
from ctypes import wintypes as w
import sys
from PIL import ImageGrab, Image, ImageDraw
u=c.windll.user32
u.SetProcessDpiAwarenessContext(c.c_void_p(-4))
pid=int(sys.argv[1]); found=[]
@c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
def callback(hwnd,_):
    p=w.DWORD()
    u.GetWindowThreadProcessId(hwnd,c.byref(p))
    title=c.create_unicode_buffer(256)
    u.GetWindowTextW(hwnd,title,256)
    if p.value==pid and u.IsWindowVisible(hwnd) and title.value=="QuickClip321":
        found.append(hwnd)
    return True
u.EnumWindows(callback,0)
if len(found)!=1: raise SystemExit(f"Expected one candidate window, got {len(found)}")
hwnd=found[0]; u.GetForegroundWindow.restype=w.HWND
if u.GetForegroundWindow()!=hwnd: raise SystemExit("Candidate not foreground; refusing to capture")
r=w.RECT();u.GetWindowRect(hwnd,c.byref(r))
im=ImageGrab.grab(bbox=(r.left,r.top,r.right,r.bottom),all_screens=True)
# Exclude pixels outside rounded app window.
mask=Image.new("L",im.size,0);ImageDraw.Draw(mask).rounded_rectangle((0,0,im.width-1,im.height-1),radius=16,fill=255)
im.putalpha(mask); im.save(sys.argv[2])
print(im.size, "unique colors",len(im.getcolors(1_000_000) or []))
