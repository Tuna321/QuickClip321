"""Isolated Windows native UI interaction smoke. Never touches the system clipboard."""
import ctypes as c, sys, time
from ctypes import wintypes as w
u=c.windll.user32
u.SetProcessDpiAwarenessContext(c.c_void_p(-4))
u.GetDlgItem.restype=w.HWND
u.SendMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM]
u.SendMessageW.restype=w.LPARAM
u.SetWindowTextW.argtypes=[w.HWND,w.LPCWSTR]
u.IsWindowVisible.argtypes=[w.HWND]
u.GetWindowRect.argtypes=[w.HWND,c.POINTER(w.RECT)]
pid=int(sys.argv[1]);found=[]
@c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
def cb(hwnd,_):
    p=w.DWORD();u.GetWindowThreadProcessId(hwnd,c.byref(p))
    title=c.create_unicode_buffer(256);u.GetWindowTextW(hwnd,title,256)
    if p.value==pid and title.value=="QuickClip321":found.append(hwnd)
    return True
for _ in range(40):
    found.clear();u.EnumWindows(cb,0)
    if len(found)==1:break
    time.sleep(.1)
assert len(found)==1,"Expected exactly one candidate window"
hwnd=found[0]
def control(i):
    h=u.GetDlgItem(hwnd,i);assert h,i;return h
def send(h,m,a=0,b=0):return u.SendMessageW(h,m,a,b)
def visible(i):return bool(u.IsWindowVisible(control(i)))
def click(i):send(control(i),0xf5);time.sleep(.12)
def setlink(s):
    buf=c.create_unicode_buffer(s);send(control(101),0x000C,0,c.addressof(buf));time.sleep(.1)
def height():
    r=w.RECT();u.GetWindowRect(hwnd,c.byref(r));return r.bottom-r.top
assert u.IsWindowVisible(hwnd)
setlink("https://example.com/video");assert not visible(105)
setlink("https://www.youtube.com/playlist?list=PLfixture");assert visible(105)
send(control(105),0xf1,1);assert send(control(105),0xf0)==1
setlink("https://www.youtube.com/playlist?list=PLother");assert send(control(105),0xf0)==0
setlink("");assert not visible(105)
h=height();click(106);assert visible(107) and height()>h
click(106);assert not visible(107) and height()==h
click(103)
buf=c.create_unicode_buffer(500);u.GetWindowTextW(control(104),buf,500)
assert "valid" in buf.value.lower() or "http" in buf.value.lower(),buf.value
print("Native playlist, consent, queue toggle, invalid URL: PASS")
