"""Collect license notices from the installed runtime distributions for test packaging."""

import importlib.metadata
import json
import sys
from pathlib import Path

destination = Path(sys.argv[1])
destination.mkdir(parents=True, exist_ok=True)
records = []
for name in [
    "customtkinter",
    "Pillow",
    "pystray",
    "plyer",
    "yt-dlp",
    "yt-dlp-ejs",
    "darkdetect",
    "packaging",
    "six",
    "brotli",
    "certifi",
    "mutagen",
    "pycryptodomex",
    "requests",
    "urllib3",
    "websockets",
    "charset-normalizer",
    "idna",
]:
    distribution = importlib.metadata.distribution(name)
    record = {
        "name": name,
        "version": distribution.version,
        "license_expression": distribution.metadata.get("License-Expression"),
        "license": distribution.metadata.get("License"),
        "project_urls": distribution.metadata.get_all("Project-URL"),
        "files": [],
    }
    for file in distribution.files or []:
        if any(token in Path(file).name.casefold() for token in ("license", "copying", "notice")):
            source = Path(distribution.locate_file(file))
            if source.is_file():
                target = destination / name / str(file).replace("../", "").replace("..\\", "")
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes())
                record["files"].append(str(target.relative_to(destination)))
    records.append(record)
(destination / "distributions.json").write_text(json.dumps(records, indent=2), encoding="utf-8")
