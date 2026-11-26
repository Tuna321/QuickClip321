"""Windows end-to-end first launch: real pinned HTTPS downloads, then local media.
Requires FFmpeg on the test runner to CREATE the authorized synthetic fixture only.
The app itself gets no preinstalled media tools on PATH, and uses isolated state.
Never changes the user's clipboard, startup registration or existing preferences.
"""
from pathlib import Path
import http.server
import functools
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import uuid

root = Path(__file__).resolve().parents[1]
exe = Path(sys.argv[1]).resolve()
test = root / ("smoke-beta3-" + uuid.uuid4().hex)
test.mkdir()
fixture = test / "fixture"
fixture.mkdir()
source = fixture / "sample.mp4"
ff = shutil.which("ffmpeg")
assert ff, "Fixture generator requires FFmpeg on the test runner"
subprocess.run([ff, "-v", "error", "-f", "lavfi", "-i", "testsrc2=s=320x240:r=24:d=2",
                "-f", "lavfi", "-i", "sine=frequency=440:duration=2", "-c:v", "mpeg4",
                "-c:a", "aac", "-threads", "2", "-shortest", str(source)], check=True)

class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print("HTTP fixture:", fmt % args, flush=True)

server = http.server.ThreadingHTTPServer(("127.0.0.1", 0),
    functools.partial(Handler, directory=str(fixture)))
threading.Thread(target=server.serve_forever, daemon=True).start()
state = test / "state/state.json"
env = os.environ.copy()
env["PATH"] = os.pathsep.join([str(Path(env["SYSTEMROOT"]) / "System32"), env["SYSTEMROOT"]])
for tool in ("yt-dlp.exe", "ffmpeg.exe", "ffprobe.exe", "node.exe"):
    assert not shutil.which(tool, path=env["PATH"]), f"{tool} unexpectedly on isolated PATH"
env["QC_V3_SMOKE_DIR"] = str(test)
env.pop("QC_V3_BENCH_DIR", None)
env.pop("QC_NOTIFICATION_FIXTURE", None)
print("Isolated root:", test, flush=True)
assert not state.exists()
start = time.monotonic()

def run(num):
    env["QC_V3_VISUAL"] = f"first-run:http://127.0.0.1:{server.server_port}/sample.mp4?run={num}"
    p = subprocess.Popen([str(exe)], env=env)
    seen = None
    try:
        while time.monotonic() - start < 840:
            assert p.poll() is None, f"App exited {p.returncode}"
            try:
                s = json.loads(state.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                time.sleep(.3)
                continue
            jobs = s["jobs"]
            settings = s["settings"]
            stage = "tools configured" if settings["yt_dlp"] else "waiting for setup"
            if len(jobs) >= num:
                j = jobs[-1]
                stage = j["status"] + ": " + j["message"]
                assert j["status"] not in ("failed", "cancelled"), stage
                if j["status"] == "completed":
                    assert j["preset"] == "editor" and j["quality"] == "1080", j
                    tools = Path(settings["yt_dlp"]).parent
                    assert tools == Path(settings["ffmpeg"]).parent
                    assert tools.is_relative_to(test / "state/tools")
                    assert all((tools / f).is_file() for f in ("yt-dlp.exe", "ffmpeg.exe", "ffprobe.exe", "node.exe"))
                    assert len(j["files"]) == 1
                    file = Path(j["files"][0])
                    assert file.is_relative_to(test / "media") and file.stat().st_size > 0
                    probe = json.loads(subprocess.check_output([str(tools / "ffprobe.exe"),
                        "-v", "error", "-show_streams", "-of", "json", str(file)], env=env))
                    codecs = {x["codec_name"] for x in probe["streams"]}
                    assert {"h264", "aac"} <= codecs, codecs
                    print(f"PASS launch {num}: actual {file.stat().st_size} byte MP4, codecs {codecs}", flush=True)
                    return tools
            if seen != stage:
                print(f"Launch {num}: {stage}", flush=True)
                seen = stage
            time.sleep(.5)
        raise TimeoutError("Setup or first download timed out")
    finally:
        p.terminate()
        p.wait(timeout=15)

try:
    tools = run(1)
    stamps = {f.name: f.stat().st_mtime_ns for f in tools.iterdir() if f.is_file()}
    elapsed = time.monotonic() - start
    print(f"Cold setup and first download: {elapsed:.1f}s", flush=True)
    assert run(2) == tools
    assert stamps == {f.name: f.stat().st_mtime_ns for f in tools.iterdir() if f.is_file()}
    assert not list(tools.parent.glob(".install-*"))
    print("PASS second launch: existing tools reused; no new installation", flush=True)
    (test / "first-run-pass.json").write_text(json.dumps({"cold_install": True, "first_download": True,
        "restart_download": True, "no_reinstall": True, "elapsed_seconds": elapsed}), encoding="utf-8")
finally:
    server.shutdown()
