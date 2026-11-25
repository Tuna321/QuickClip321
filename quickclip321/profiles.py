import json
from pathlib import Path

DEFAULT_PROFILES = {
    "Premiere Ready": {"format": "mp4", "quality": "1080", "premiere": True, "metadata": False},
    "Fast Download": {"format": "original", "quality": "best", "premiere": False, "metadata": False},
    "Audio": {"format": "mp3", "quality": "best", "premiere": False, "metadata": False},
    "Archive": {"format": "original", "quality": "best", "premiere": False, "metadata": True},
}


def load_profiles(path):
    profiles = {key: dict(value) for key, value in DEFAULT_PROFILES.items()}
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        for name, profile in data.items():
            if (
                isinstance(profile, dict)
                and profile.get("format") in {"mp4", "mp3", "wav", "m4a", "original"}
                and profile.get("quality") in {"best", "2160", "1440", "1080", "720", "480"}
            ):
                profiles[str(name)[:60]] = profile
    except (OSError, ValueError, TypeError, AttributeError):
        pass
    return profiles
