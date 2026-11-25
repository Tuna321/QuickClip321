import json
from quickclip321.organizer import DEFAULT_DATABASE, LEGACY_DESTINATIONS, classify, load_database
from quickclip321.popup import Rect, positioned
from quickclip321.settings import AppSettings, SettingsStore
from quickclip321.ui import parse_urls


def test_batch_urls_validate_atomically_and_deduplicate():
    urls, error = parse_urls("https://example.com/a\nhttps://example.com/a\nhttps://example.com/b")
    assert error == ""
    assert urls == ["https://example.com/a", "https://example.com/b"]
    assert parse_urls("https://example.com/a\nnot-a-url")[0] == []


def test_settings_restore_validated_view_and_position(tmp_path, monkeypatch):
    monkeypatch.setenv("QUICKCLIP321_DATA_DIR", str(tmp_path))
    store = SettingsStore()
    settings = AppSettings(startup_mode="remember", remembered_mode="advanced", theme_mode="Light",
                           color_theme="Ocean", popup_position="Custom", popup_x_ratio=0.75)
    store.save(settings)
    restored = store.load()
    assert (restored.startup_mode, restored.remembered_mode, restored.theme_mode, restored.color_theme) == (
        "remember", "advanced", "Light", "Ocean")
    assert restored.popup_position == "Custom"
    assert restored.popup_x_ratio == 0.75
    data = json.loads(store.path.read_text(encoding="utf-8"))
    data.update(popup_x_ratio=8, popup_y_ratio=-2, theme_mode="Invalid", ai_provider="random")
    store.path.write_text(json.dumps(data), encoding="utf-8")
    restored = store.load()
    assert (restored.popup_x_ratio, restored.popup_y_ratio, restored.theme_mode, restored.ai_provider) == (
        1.0, 0.0, "System", "Off")


def test_monitor_relative_custom_popup_clamps():
    work = Rect(-1500, 40, -100, 900)
    result = positioned(work, (600, 300), "Custom", ratios=(0.75, 0.5))
    assert result == Rect(-900, 320, -300, 620)
    assert positioned(work, (3000, 2000), "Custom", ratios=(2, -1)) == work


def test_organizer_rule_and_neutral_default(tmp_path):
    database = json.loads(json.dumps(DEFAULT_DATABASE))
    assert all("QuickClip321" in path for path in database["destinations"].values())
    database["destinations"]["unsorted"] = str(tmp_path / "Unsorted")
    database["rules"] = [{"contains": "odd phrase", "category": "unsorted"}]
    path = tmp_path / "organizer.json"
    path.write_text(json.dumps(database), encoding="utf-8")
    choice = classify({"title": "An odd phrase clip"}, "mp4", tmp_path, load_database(path))
    assert (choice.category, choice.destination) == ("unsorted", tmp_path / "Unsorted")


def test_legacy_builtin_paths_migrate_without_overwriting_custom_path(tmp_path):
    database = json.loads(json.dumps(DEFAULT_DATABASE))
    database["destinations"]["video"] = str(tmp_path.home() / "Downloads" / LEGACY_DESTINATIONS["video"])
    database["destinations"]["audio"] = str(tmp_path / "My Music")
    path = tmp_path / "organizer.json"
    path.write_text(json.dumps(database), encoding="utf-8")
    migrated = load_database(path)["destinations"]
    assert migrated["video"] == DEFAULT_DATABASE["destinations"]["video"]
    assert migrated["audio"] == str(tmp_path / "My Music")
