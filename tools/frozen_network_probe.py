import http.server
import json
import multiprocessing as mp
import os
import queue
import ssl
import sys
import threading
import traceback
from pathlib import Path


def probe(url, events):
    import certifi
    from yt_dlp import YoutubeDL

    result = {
        "pid": os.getpid(),
        "meipass": getattr(sys, "_MEIPASS", None),
        "certifi": certifi.where(),
        "certifi_exists": Path(certifi.where()).is_file(),
        "sslkeylogfile": os.environ.get("SSLKEYLOGFILE"),
    }
    try:
        context = ssl.create_default_context(cafile=certifi.where())
        result["ssl_certs"] = context.cert_store_stats()
        with YoutubeDL({"quiet": True}) as ydl:
            response = ydl.urlopen(url)
            result["response"] = response.read().decode()
    except BaseException as exc:
        result["error"] = traceback.format_exc()
        result["nested"] = [
            "".join(traceback.format_exception(type(e), e, e.__traceback__))
            for e in getattr(exc, "unexpected_errors", [])
        ]
    events.put(result)


if __name__ == "__main__":
    mp.freeze_support()

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"network works")

        def log_message(self, *args):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    events = mp.Queue()
    process = mp.Process(target=probe, args=(f"http://localhost:{server.server_port}/probe", events))
    process.start()
    try:
        result = events.get(timeout=20)
    except queue.Empty:
        result = {"error": "child timeout", "exitcode": process.exitcode}
    process.join(timeout=3)
    server.shutdown()
    Path(sys.argv[1]).write_text(json.dumps(result, indent=2), encoding="utf-8")
