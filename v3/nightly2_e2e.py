
"""Isolated Windows tests; never touches real state, clipboard or startup."""
from pathlib import Path
import os, sys, json, subprocess, time, threading, http.server, functools, uuid
root=Path(__file__).resolve().parents[1]
exe=Path(sys.argv[1]).resolve()
tools=Path(sys.argv[2]).resolve()
test=root/("smoke-beta3-"+uuid.uuid4().hex);test.mkdir()
fixture=test/"fixture";fixture.mkdir()
source=fixture/"sample.mp4"
subprocess.run([str(tools/"ffmpeg.exe"),"-v","error","-f","lavfi","-i","testsrc2=s=640x360:r=30:d=4","-c:v","mpeg4","-q:v","1","-threads","2","-movflags","+faststart",str(source)],check=True,timeout=30)
class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*a):pass
    def copyfile(self,source,outputfile):
        try:super().copyfile(source,outputfile)
        except (BrokenPipeError,ConnectionResetError,ConnectionAbortedError):pass
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(Handler,directory=str(fixture)))
threading.Thread(target=server.serve_forever,daemon=True).start()
url=f"http://127.0.0.1:{server.server_port}/sample.mp4"
def seed(name,cap=0):
    d=test/name;d.mkdir();(d/"state").mkdir();out=d/"media";out.mkdir()
    state=d/"state/state.json"
    data={"settings":{"destination":str(out),"yt_dlp":str(tools/"yt-dlp.exe"),"ffmpeg":str(tools/"ffmpeg.exe"),"prefs":{"default_preset":"original","default_format":"original","default_quality":"best","limit_kib_per_second":cap,"notifications":True,"run_on_startup":False}},"jobs":[]}
    state.write_text(json.dumps(data),encoding="utf-8")
    return d,state,data
def launch(d,mode=None,bench=False,shortcut=False):
    env=os.environ.copy()
    for k in ("QC_V3_SMOKE_DIR","QC_V3_BENCH_DIR","QC_V3_VISUAL","QC_NOTIFICATION_FIXTURE","QC_COMPLETION_SMOKE","QC_SHORTCUT_SMOKE","QC_SHORTCUT_DOWNLOAD_FIXTURE"):env.pop(k,None)
    env["QC_V3_BENCH_DIR" if bench else "QC_V3_SMOKE_DIR"]=str(d)
    if mode:env["QC_V3_VISUAL"]=mode
    env["QC_COMPLETION_SMOKE"]="1"
    if shortcut:env["QC_SHORTCUT_DOWNLOAD_FIXTURE"]=url
    elif bench:env["QC_NOTIFICATION_FIXTURE"]=url
    return subprocess.Popen([str(exe)],env=env)
def stop(p):
    if p.poll() is None:p.terminate();p.wait(timeout=15)
def wait_job(state,p,timeout=80):
    start=time.monotonic()
    while time.monotonic()-start<timeout:
        assert p.poll() is None,f"App exited: {p.returncode}"
        try:j=json.loads(state.read_text(encoding="utf-8"))["jobs"][-1]
        except (OSError,ValueError,IndexError):time.sleep(.1);continue
        if j["status"]=="completed":return j,time.monotonic()-start
        assert j["status"] not in ("failed","cancelled"),j["message"]
        time.sleep(.1)
    raise TimeoutError("Download test timeout")
def wait_file(path,timeout=8):
    t=time.monotonic()
    while time.monotonic()-t<timeout:
        if path.is_file():return path.read_text(encoding="utf-8")
        time.sleep(.1)
    raise TimeoutError(path.name)
results={}
try:
    times=[]
    for cap in (0,256):
        d,state,data=seed(f"rate-{cap}",cap)
        p=launch(d,"first-run:"+url+f"?cap={cap}")
        try:
            j,elapsed=wait_job(state,p)
            assert Path(j["files"][0]).read_bytes()==source.read_bytes()
            assert j["preset"]=="original"
            assert wait_file(d/"notice-message.txt")=="Download completed"
            times.append(elapsed)
            print(f"Actual app rate {cap}: {source.stat().st_size} bytes, {elapsed:.2f}s",flush=True)
        finally:stop(p)
    assert times[1]>=source.stat().st_size/(256*1024)*.75 and times[1]>times[0]+2
    results["rate"]=times
    d,state,data=seed("completion")
    p=launch(d,bench=True)
    try:
        j,_=wait_job(state,p)
        assert Path(j["files"][0]).read_bytes()==source.read_bytes()
        assert wait_file(d/"notice-message.txt")=="Download completed"
        assert wait_file(d/"notice-shown.txt")=="visible; focus unchanged"
        assert wait_file(d/"notice-destroyed.txt")=="destroyed"
        print("PASS actual download -> native completion notification -> destroyed",flush=True)
        results["completion"]=True
    finally:stop(p)
    d,state,data=seed("shortcut")
    p=launch(d,bench=True,shortcut=True)
    try:
        j,_=wait_job(state,p)
        assert Path(j["files"][0]).read_bytes()==source.read_bytes()
        # Started/queued and completed notices may overwrite the same marker.
        start=time.monotonic()
        while time.monotonic()-start<8:
            if (d/"notice-message.txt").is_file() and (d/"notice-message.txt").read_text(encoding="utf-8")=="Download completed":
                break
            time.sleep(.1)
        else:raise AssertionError("Shortcut completion notice missing")
        print("PASS actual shortcut dispatcher -> download -> completion notification",flush=True)
        results["shortcut_completion"]=True
    finally:stop(p)
    d,state,data=seed("duplicate")
    folder=d/"media"/"Video"/"Games with spaces";folder.mkdir(parents=True)
    existing=folder/"existing.mp4";existing.write_bytes(source.read_bytes())
    data["jobs"].append({"id":"a"*32,"url":url,"preset":"original","quality":"best","playlist":False,"organize":False,"options":{"clip_start":None,"clip_end":None,"subtitle_languages":[],"subtitle_format":"","subtitle_auto":False,"archive":False},"destination":data["settings"]["destination"],"status":"completed","progress":1,"message":"Download completed","files":[str(existing)]})
    state.write_text(json.dumps(data),encoding="utf-8")
    p=launch(d,"nightly-duplicate:"+url)
    try:
        assert p.wait(timeout=55)==0
        assert wait_file(d/"persist-fixture.txt")=="true"
        jobs=json.loads(state.read_text(encoding="utf-8"))["jobs"]
        assert len(jobs)==2 and jobs[1]["status"]=="completed"
        assert existing.read_bytes()==source.read_bytes()
        assert Path(jobs[1]["files"][0]).read_bytes()==source.read_bytes()
        # Use Shell.Application to verify the actual Explorer location, not just return code.
        ps=str(Path(os.environ["SYSTEMROOT"])/"System32/WindowsPowerShell/v1.0/powershell.exe")
        command="$s=New-Object -ComObject Shell.Application; foreach($w in $s.Windows()){try{if($w.Document.Folder.Self.Path -eq $env:QC_EXPECT_FOLDER){Write-Output 'MATCH';$w.Quit()}}catch{}}"
        output=subprocess.check_output([ps,"-NoProfile","-NonInteractive","-Command",command],env={**os.environ,"QC_EXPECT_FOLDER":str(folder)},text=True,timeout=15)
        assert "MATCH" in output,"Actual Explorer location not confirmed"
        print("PASS duplicate warning, exact nested Explorer folder, explicit redownload",flush=True)
        results["duplicate_folder"]=True
    finally:stop(p)
    d,state,data=seed("settings")
    p=launch(d,"nightly-settings")
    try:assert p.wait(timeout=30)==0 and wait_file(d/"persist-fixture.txt")=="true"
    finally:stop(p)
    print("PASS settings export/import and private path exclusion",flush=True)
    results["settings_backup"]=True
    (test/"nightly2-e2e.json").write_text(json.dumps(results,indent=2))
    print("Fixture evidence:",test,flush=True)
finally:server.shutdown()
