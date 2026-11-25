import json

from quickclip321.settings import AppSettings, SettingsStore


def test_bad_fields_do_not_reset_valid_preferences(tmp_path, monkeypatch):
    monkeypatch.setenv("QUICKCLIP321_DATA_DIR", str(tmp_path))
    store = SettingsStore()
    store.path.write_text(json.dumps({
        "output_dir": str(tmp_path / "media"),
        "theme_mode": "Dark",
        "max_retries": "broken",
        "concurrent_downloads": 99,
        "fragment_concurrency": True,
        "stall_timeout_seconds": float("inf"),
        "clipboard_monitor": "false",
        "default_format": [],
        "popup_x_ratio": float("nan"),
        "popup_y_ratio": 0.25,
    }), encoding="utf-8")
    settings = store.load()
    defaults = AppSettings()
    assert settings.output_dir == str(tmp_path / "media")
    assert settings.theme_mode == "Dark"
    assert settings.max_retries == defaults.max_retries
    assert settings.concurrent_downloads == 3
    assert settings.fragment_concurrency == defaults.fragment_concurrency
    assert settings.stall_timeout_seconds == defaults.stall_timeout_seconds
    assert settings.clipboard_monitor == defaults.clipboard_monitor
    assert settings.default_format == defaults.default_format
    assert settings.popup_x_ratio == 0.5
    assert settings.popup_y_ratio == 0.25
