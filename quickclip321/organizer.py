"""Deterministic local rules. No network requests and no scan of existing media."""

from __future__ import annotations

import json
import logging
import os
import re
import shutil
import unicodedata
import uuid
from dataclasses import dataclass
from pathlib import Path


def normalize(value):
    value = str(value).casefold().replace("ı", "i")
    return " ".join(
        re.findall(r"\w+", "".join(c for c in unicodedata.normalize("NFKD", value) if not unicodedata.combining(c)))
    )


DEFAULT_DATABASE = {
    "artists": [
        {"name": "Ceza", "aliases": [], "ambiguous": True},
        {"name": "Sagopa Kajmer", "aliases": ["Sagopa"], "ambiguous": False},
        {"name": "Eminem", "aliases": ["Marshall Mathers"], "ambiguous": False},
        {"name": "Kendrick Lamar", "aliases": [], "ambiguous": False},
        {"name": "Ezhel", "aliases": [], "ambiguous": False},
        {"name": "Şehinşah", "aliases": ["Sehinsah"], "ambiguous": False},
        {"name": "Şanışer", "aliases": ["Saniser"], "ambiguous": False},
        {"name": "Norm Ender", "aliases": [], "ambiguous": False},
        {"name": "Gazapizm", "aliases": [], "ambiguous": False},
        {"name": "Ben Fero", "aliases": [], "ambiguous": False},
        {"name": "Allame", "aliases": [], "ambiguous": False},
        {"name": "Khontkar", "aliases": [], "ambiguous": False},
        {"name": "Uzi", "aliases": [], "ambiguous": True},
        {"name": "Motive", "aliases": [], "ambiguous": True},
    ],
    "movies": [
        {"name": "The Godfather", "aliases": ["Godfather"], "ambiguous": False},
        {"name": "Interstellar", "aliases": ["Yıldızlararası"], "ambiguous": False},
        {"name": "Fight Club", "aliases": ["Dövüş Kulübü"], "ambiguous": False},
        {"name": "The Dark Knight", "aliases": ["Kara Şövalye"], "ambiguous": False},
    ],
    "tv": [
        {"name": "Kurtlar Vadisi", "aliases": ["Kurtlar Vadisi Pusu"], "ambiguous": False},
        {"name": "Ezel", "aliases": [], "ambiguous": True},
        {"name": "Behzat Ç", "aliases": ["Behzat C"], "ambiguous": False},
        {"name": "Breaking Bad", "aliases": [], "ambiguous": False},
        {"name": "The Sopranos", "aliases": ["Sopranos"], "ambiguous": False},
    ],
    "schema_version": 3,
    "rules": [],
    "destinations": {
        "beats": str(Path.home() / "Downloads/QuickClip321/Audio"),
        "audio": str(Path.home() / "Downloads/QuickClip321/Audio"),
        "film": str(Path.home() / "Downloads/QuickClip321/Video"),
        "rap": str(Path.home() / "Downloads/QuickClip321/Video"),
        "video": str(Path.home() / "Downloads/QuickClip321/Video"),
        "unsorted": str(Path.home() / "Downloads/QuickClip321/Unsorted"),
    },
}

LEGACY_DESTINATIONS = {
    "beats": "03_Ses/01_Instrumental_ve_Beatler",
    "audio": "03_Ses/04_Sarkilar_ve_Diger_Ses",
    "film": "01_Video/06_Film_Dizi_Klipleri",
    "rap": "01_Video/09_Muzik_Videolari",
    "video": "01_Video/10_Diger_Videolar",
}


def ensure_database(path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("x", encoding="utf-8") as handle:
            json.dump(DEFAULT_DATABASE, handle, indent=2, ensure_ascii=False)
    except FileExistsError:
        pass
    return path


@dataclass(frozen=True)
class Classification:
    category: str
    confidence: float
    reason: str
    destination: Path


def classify(info, output_format, fallback, database=None, enabled=True):
    db = DEFAULT_DATABASE if database is None else database
    default = Path(fallback)
    if not enabled:
        return Classification("default", 1, "Smart Organizer disabled", default)
    title = normalize(info.get("title", ""))
    for rule in db.get("rules", []):
        if not isinstance(rule, dict):
            continue
        term = normalize(rule.get("contains", ""))
        category = rule.get("category")
        if term and isinstance(category, str) and f" {term} " in f" {title} ":
            destination = db.get("destinations", {}).get(category)
            if destination:
                return Classification(category, 0.9, f"Title contains: {term}", Path(destination))
    padded = f" {title} "
    audio = output_format in {"mp3", "m4a", "wav", "flac", "opus"} or info.get("vcodec") == "none"
    if audio:
        evidence = f" {normalize(' '.join(str(info.get(field) or '') for field in ('title', 'track', 'genre')))} "
        terms = ["instrumental", "beat", "beats", "type beat", "karaoke", "enstrumantal", "altyapi", "backing track"]
        category = "beats" if any(f" {term} " in evidence for term in terms) else "audio"
        confidence, reason = 0.95, "Instrumental/beat/karaoke evidence" if category == "beats" else "Audio media"
    else:
        category, confidence, reason = "video", 0.35, "Unknown/ambiguous video: default video folder"
        artist = normalize(info.get("artist") or info.get("album_artist") or "")
        music = any(
            f" {term} " in padded
            for term in ["official video", "music video", "resmi klip", "official music video", "video klip"]
        )
        commentary = any(
            f" {term} " in padded
            for term in [
                "interview",
                "roportaj",
                "belgesel",
                "documentary",
                "haber",
                "review",
                "reaction",
                "tepki",
                "inceleme",
            ]
        )
        # An anchored artist prefix plus music context disambiguates names such as Ceza.
        prefix = normalize(re.split(r"\s[-–—|]\s", str(info.get("title", "")), maxsplit=1)[0])
        channel = normalize(info.get("channel") or info.get("uploader") or "")
        channel = " ".join(word for word in channel.split() if word not in {"official", "topic", "vevo", "resmi"})
        for entry in db.get("artists", []):
            aliases = [normalize(a) for a in [entry["name"], *entry.get("aliases", [])]]
            exact_artist = artist and artist in aliases
            anchored = prefix in aliases and music
            title_match = not entry.get("ambiguous", True) and music and any(f" {a} " in padded for a in aliases)
            channel_match = channel in aliases and music
            if not commentary and (exact_artist or anchored or title_match or channel_match):
                category, confidence, reason = (
                    "rap",
                    0.98 if exact_artist else 0.9,
                    f"Rap artist match: {entry['name']}",
                )
                break
        clip = any(
            f" {term} " in padded
            for term in ["scene", "clip", "sahne", "fragman", "trailer", "film", "dizi", "bolum", "episode"]
        )
        series = normalize(info.get("series") or info.get("tv_show") or "")
        movie = normalize(info.get("movie") or info.get("movie_title") or "")
        for entry in [*db.get("movies", []), *db.get("tv", [])]:
            aliases = [normalize(a) for a in [entry["name"], *entry.get("aliases", [])]]
            exact_metadata = (series and series in aliases) or (movie and movie in aliases)
            # An ambiguous TV title is accepted only with explicit TV/episode context.
            tv_context = entry in db.get("tv", []) and any(
                f" {term} " in padded for term in ["dizi", "bolum", "episode"]
            )
            title_match = (
                clip and any(f" {a} " in padded for a in aliases) and (not entry.get("ambiguous", True) or tv_context)
            )
            if exact_metadata or title_match:
                category, confidence, reason = (
                    "film",
                    0.97 if exact_metadata else 0.9,
                    f"Movie/TV match: {entry['name']}",
                )
                break
        if category == "video" and series:
            category, confidence, reason = "film", 0.9, "Explicit series metadata"
    # Missing keys in custom databases still use the requested category folder.
    destinations = {**DEFAULT_DATABASE["destinations"], **db.get("destinations", {})}
    return Classification(category, confidence, reason, Path(destinations.get(category, default)))


def resolve_destination(info, payload, database=None):
    """Shared by preview and actual publication; manual choice has highest precedence."""
    if payload.get("manual_destination"):
        selected = payload.get("selected_destination") or payload["output_dir"]
        return Classification("manual", 1, "Manual destination override", Path(selected))
    return classify(
        info, payload["output_format"], payload["output_dir"], database, payload.get("organizer_enabled", True)
    )


def load_database(path):
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            raise ValueError("Database must be an object")
        for field in ("artists", "movies", "tv"):
            entries = data.get(field, [])
            if not isinstance(entries, list):
                raise ValueError(f"{field} must be a list")
            data[field] = [
                entry
                for entry in entries
                if isinstance(entry, dict)
                and isinstance(entry.get("name"), str)
                and isinstance(entry.get("aliases", []), list)
                and all(isinstance(alias, str) for alias in entry.get("aliases", []))
            ]
        destinations = data.get("destinations", {})
        data["destinations"] = (
            {key: value for key, value in destinations.items() if isinstance(value, str) and Path(value).is_absolute()}
            if isinstance(destinations, dict)
            else {}
        )
        # Migrate only the old built-in paths. User-chosen paths remain untouched.
        for category, suffix in LEGACY_DESTINATIONS.items():
            if data["destinations"].get(category) == str(Path.home() / "Downloads" / suffix):
                data["destinations"][category] = DEFAULT_DATABASE["destinations"][category]
        rules = data.get("rules", [])
        data["rules"] = [
            {"contains": rule["contains"].strip(), "category": rule["category"]}
            for rule in rules
            if isinstance(rule, dict)
            and isinstance(rule.get("contains"), str)
            and rule["contains"].strip()
            and isinstance(rule.get("category"), str)
            and rule["category"] in data["destinations"]
        ] if isinstance(rules, list) else []
        return data
    except (OSError, ValueError, TypeError):
        logging.exception("Organizer database invalid; using safe fallback")
        return {"artists": [], "movies": [], "destinations": {}}


def publish(source: Path, destination: Path) -> Path:
    """Stage on destination volume, then publish atomically without overwriting media."""
    destination.mkdir(parents=True, exist_ok=True)
    staging = destination / ".quickclip321" / "publishing"
    staging.mkdir(parents=True, exist_ok=True)
    temporary = staging / f"{uuid.uuid4().hex}.part"
    try:
        with temporary.open("xb") as handle, source.open("rb") as input_file:
            shutil.copyfileobj(input_file, handle, 1024 * 1024)
            handle.flush()
            os.fsync(handle.fileno())
        for number in range(10000):
            target = destination / (source.name if not number else f"{source.stem} ({number}){source.suffix}")
            try:
                if os.name == "nt":
                    os.rename(temporary, target)  # Windows rename is exclusive and atomic.
                else:
                    os.link(temporary, target)
                    temporary.unlink()
            except FileExistsError:
                continue
            try:
                source.unlink()
            except OSError:
                logging.warning("Published output; staging source could not be removed: %s", source)
            return target
        raise OSError("No unique destination filename available")
    finally:
        temporary.unlink(missing_ok=True)
