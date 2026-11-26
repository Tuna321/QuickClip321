use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub name: String,
    pub format: String,
    pub quality: String,
    pub archive: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub contains: String,
    pub category: String,
}
pub const CATEGORIES: [&str; 10] = [
    "audio",
    "beats",
    "film",
    "rap",
    "video",
    "games",
    "pop",
    "rock",
    "electronic",
    "music",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Preferences {
    pub popup_mode: String,
    pub popup_x: f64,
    pub popup_y: f64,
    pub startup_view: String,
    pub theme: String,
    pub language: String,
    pub update_channel: String,
    pub update_mode: String,
    pub open_hotkey: String,
    pub shortcuts: BTreeMap<String, String>,
    pub always_on_top: bool,
    pub last_destination: Option<PathBuf>,
    pub last_organize: bool,
    pub last_video_quality: String,
    pub accent: String,
    pub clipboard_monitor: bool,
    pub smart_organizer: bool,
    pub notifications: bool,
    pub run_on_startup: bool,
    pub default_preset: String,
    pub default_format: String,
    pub default_quality: String,
    pub fragment_concurrency: u8,
    pub material: String,
    pub detailed_folders: bool,
    pub concurrent_downloads: u8,
    pub limit_kib_per_second: u32,
    pub previous_ytdlp: Option<PathBuf>,
    pub profiles: Vec<Profile>,
    pub rules: Vec<Rule>,
    pub category_paths: BTreeMap<String, PathBuf>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            popup_mode: "Tray".into(),
            popup_x: 0.5,
            popup_y: 0.5,
            startup_view: "quick".into(),
            theme: "Dark".into(),
            language: "en".into(),
            update_channel: "stable".into(),
            update_mode: "semi".into(),
            open_hotkey: "Ctrl+Alt+Space".into(),
            shortcuts: BTreeMap::new(),
            always_on_top: true,
            last_destination: None,
            last_organize: true,
            last_video_quality: "1080".into(),
            accent: "Purple".into(),
            clipboard_monitor: true,
            smart_organizer: true,
            notifications: true,
            run_on_startup: true,
            default_preset: "editor".into(),
            default_format: "editor".into(),
            default_quality: "1080".into(),
            fragment_concurrency: 4,
            material: "Flat".into(),
            detailed_folders: true,
            concurrent_downloads: 1,
            limit_kib_per_second: 0,
            previous_ytdlp: None,
            profiles: vec![],
            rules: vec![],
            category_paths: BTreeMap::new(),
        }
    }
}
pub const SHORTCUTS: [(&str, &str, bool); 8] = [
    ("open", "Ctrl+Alt+Space", true),
    ("download", "Ctrl+Shift+D", true),
    ("mp3", "Ctrl+Shift+3", true),
    ("mp4", "Ctrl+Shift+4", true),
    ("folder", "Ctrl+Shift+O", true),
    ("submit", "Ctrl+Enter", false),
    ("link", "Ctrl+L", false),
    ("hide", "Esc", false),
];
pub fn parse_shortcut(key: &str, global: bool) -> Option<(u32, u32)> {
    let parts = key.split('+').collect::<Vec<_>>();
    let last = *parts.last()?;
    let mut mods = 0;
    for part in &parts[..parts.len() - 1] {
        let bit = match *part {
            "Ctrl" => 2,
            "Alt" => 1,
            "Shift" => 4,
            _ => return None,
        };
        if mods & bit != 0 {
            return None;
        }
        mods |= bit;
    }
    // Do not reserve plain typing keys or AltGr combinations with printable characters.
    if global && (mods & 3 == 0 || (mods & 3 == 3 && last != "Space" && last.len() == 1)) {
        return None;
    }
    if !global && mods == 0 && last != "Esc" && !last.starts_with('F') {
        return None;
    }
    let vk = match last {
        "Space" => 32,
        "Enter" => 13,
        "Esc" => 27,
        "Tab" => 9,
        v if v.len() == 1 && v.as_bytes()[0].is_ascii_uppercase() => v.as_bytes()[0] as u32,
        v if v.len() == 1 && v.as_bytes()[0].is_ascii_digit() => v.as_bytes()[0] as u32,
        v if v.starts_with('F') => {
            let n = v[1..].parse::<u32>().ok()?;
            if !(1..=12).contains(&n) {
                return None;
            }
            111 + n
        }
        _ => return None,
    };
    Some((mods, vk))
}
impl Preferences {
    pub fn effective_shortcuts(&self) -> BTreeMap<String, String> {
        SHORTCUTS
            .iter()
            .map(|(id, default, _)| {
                (
                    id.to_string(),
                    self.shortcuts.get(*id).cloned().unwrap_or_else(|| {
                        if *id == "open" {
                            self.open_hotkey.clone()
                        } else {
                            default.to_string()
                        }
                    }),
                )
            })
            .collect()
    }
    pub fn normalize(&mut self) {
        let d = Self::default();
        if !matches!(self.update_channel.as_str(), "stable" | "nightly") {
            self.update_channel = d.update_channel.clone();
        }
        if !matches!(self.update_mode.as_str(), "manual" | "semi" | "automatic") {
            self.update_mode = d.update_mode.clone();
        }
        if !matches!(self.language.as_str(), "en" | "tr") {
            self.language = d.language.clone();
        }
        if parse_shortcut(&self.open_hotkey, true).is_none() && self.open_hotkey != "Off" {
            self.open_hotkey = d.open_hotkey.clone();
        }
        self.shortcuts.retain(|action, key| {
            SHORTCUTS.iter().any(|(name, _, global)| {
                *name == action && (key == "Off" || parse_shortcut(key, *global).is_some())
            })
        });
        if self
            .last_destination
            .as_ref()
            .is_some_and(|p| !p.is_absolute())
        {
            self.last_destination = None;
        }
        if !matches!(self.popup_mode.as_str(), "Tray" | "Corner" | "Custom") {
            self.popup_mode = d.popup_mode;
        }
        if !self.popup_x.is_finite() {
            self.popup_x = 0.5;
        }
        if !self.popup_y.is_finite() {
            self.popup_y = 0.5;
        }
        self.popup_x = self.popup_x.clamp(0., 1.);
        self.popup_y = self.popup_y.clamp(0., 1.);
        if !matches!(
            self.startup_view.as_str(),
            "quick" | "advanced" | "remember"
        ) {
            self.startup_view = d.startup_view;
        }
        if !matches!(
            self.theme.as_str(),
            "System" | "Light" | "Dark" | "Midnight" | "AMOLED" | "Forest" | "Sand"
        ) {
            self.theme = d.theme;
        }
        if !matches!(
            self.accent.as_str(),
            "Purple" | "Graphite" | "Ocean" | "Emerald" | "Rose" | "Amber"
        ) {
            self.accent = d.accent;
        }
        if !matches!(
            self.default_preset.as_str(),
            "editor" | "original" | "mp3" | "archive" | "custom"
        ) {
            self.default_preset = d.default_preset;
        }
        if !matches!(
            self.default_format.as_str(),
            "editor" | "original" | "mp3" | "m4a" | "wav"
        ) {
            self.default_format = d.default_format;
        }
        if !matches!(
            self.default_quality.as_str(),
            "best" | "2160" | "1440" | "1080" | "720" | "480" | "360"
        ) {
            self.default_quality = d.default_quality;
        }
        if !matches!(
            self.last_video_quality.as_str(),
            "best" | "2160" | "1440" | "1080" | "720" | "480" | "360"
        ) {
            self.last_video_quality = "1080".into();
        }
        self.fragment_concurrency = self.fragment_concurrency.clamp(1, 8);
        if !matches!(self.material.as_str(), "Flat" | "Glass" | "Carbon" | "Sand") {
            self.material = "Flat".into();
        }
        self.concurrent_downloads = self.concurrent_downloads.clamp(1, 3);
        self.limit_kib_per_second = self.limit_kib_per_second.min(1_048_576);
        if self
            .previous_ytdlp
            .as_ref()
            .is_some_and(|p| !p.is_absolute())
        {
            self.previous_ytdlp = None;
        }
        let mut names = std::collections::HashSet::new();
        self.profiles.retain(|p| {
            !p.name.trim().is_empty()
                && p.name.chars().count() <= 60
                && !p.name.chars().any(char::is_control)
                && crate::selector(&p.format, &p.quality).is_ok()
                && names.insert(p.name.to_lowercase())
        });
        self.profiles.truncate(50);
        self.rules.retain(|r| {
            !r.contains.trim().is_empty()
                && r.contains.chars().count() <= 120
                && CATEGORIES.contains(&r.category.as_str())
        });
        self.rules.truncate(100);
        self.category_paths
            .retain(|k, v| CATEGORIES.contains(&k.as_str()) && v.is_absolute());
    }
}
#[derive(Clone, Copy, Debug)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}
pub fn popup_position(
    area: WorkArea,
    size: (i32, i32),
    point: Option<(f64, f64)>,
    prefs: &Preferences,
    margin: i32,
) -> (i32, i32) {
    let (w, h) = size;
    let room_x = (area.width - w).max(0);
    let room_y = (area.height - h).max(0);
    let max_x = area.left.saturating_add(room_x);
    let max_y = area.top.saturating_add(room_y);
    let right = area.left.saturating_add(area.width);
    let bottom = area.top.saturating_add(area.height);
    let (x, y) = match prefs.popup_mode.as_str() {
        "Custom" => (
            area.left + (room_x as f64 * prefs.popup_x).round() as i32,
            area.top + (room_y as f64 * prefs.popup_y).round() as i32,
        ),
        "Corner" => (max_x - margin, max_y - margin),
        _ => {
            let (ax, ay) = point
                .map(|(x, y)| (x as i32, y as i32))
                .unwrap_or((right, bottom));
            (
                ax - w / 2,
                if ay < area.top + area.height / 2 {
                    ay + margin
                } else {
                    ay - h - margin
                },
            )
        }
    };
    (x.clamp(area.left, max_x), y.clamp(area.top, max_y))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_preferences_survive_normalization_and_serialization() {
        for theme in [
            "System", "Dark", "Light", "Midnight", "AMOLED", "Forest", "Sand",
        ] {
            for accent in ["Purple", "Graphite", "Ocean", "Emerald", "Rose", "Amber"] {
                let mut p = Preferences {
                    theme: theme.into(),
                    accent: accent.into(),
                    ..Default::default()
                };
                p.normalize();
                assert_eq!(p.theme, theme);
                assert_eq!(p.accent, accent);
                let serialized = serde_json::to_string(&p).unwrap();
                assert_eq!(serde_json::from_str::<Preferences>(&serialized).unwrap(), p);
            }
        }
    }
    #[test]
    fn custom_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Preferences::default();
        p.concurrent_downloads = 9;
        p.profiles.push(Profile {
            name: "Edit".into(),
            format: "editor".into(),
            quality: "720".into(),
            archive: false,
        });
        p.profiles.push(Profile {
            name: "edit".into(),
            format: "editor".into(),
            quality: "720".into(),
            archive: false,
        });
        p.rules.push(Rule {
            contains: "sample".into(),
            category: "rap".into(),
        });
        p.category_paths.insert("rap".into(), dir.path().into());
        p.normalize();
        assert_eq!(p.concurrent_downloads, 3);
        assert_eq!(p.profiles.len(), 1);
        assert_eq!(p.rules.len(), 1);
        assert_eq!(
            serde_json::from_str::<Preferences>(&serde_json::to_string(&p).unwrap()).unwrap(),
            p
        );
    }
    #[test]
    fn materials_roundtrip() {
        for finish in ["Flat", "Glass", "Carbon", "Sand"] {
            let mut p = Preferences {
                material: finish.into(),
                detailed_folders: true,
                ..Default::default()
            };
            p.normalize();
            assert_eq!(p.material, finish);
            assert!(p.detailed_folders);
            assert_eq!(
                serde_json::from_str::<Preferences>(&serde_json::to_string(&p).unwrap()).unwrap(),
                p
            );
        }
    }
    #[test]
    fn popup_placement() {
        let a = WorkArea {
            left: -1920,
            top: 0,
            width: 1920,
            height: 1040,
        };
        let mut p = Preferences::default();
        assert_eq!(
            popup_position(a, (600, 395), Some((-30., 1060.)), &p, 8),
            (-600, 645)
        );
        p.popup_mode = "Custom".into();
        p.popup_x = 1.;
        p.popup_y = 0.;
        assert_eq!(popup_position(a, (600, 395), None, &p, 8), (-600, 0));
        p.popup_mode = "Corner".into();
        assert_eq!(popup_position(a, (600, 395), None, &p, 8), (-608, 637));
    }
}

#[cfg(test)]
#[test]
fn shortcut_validation() {
    let p = Preferences::default();
    let keys = p.effective_shortcuts();
    assert_eq!(keys.len(), 8);
    for (id, _, global) in SHORTCUTS {
        assert!(parse_shortcut(&keys[id], global).is_some(), "{id}");
    }
    assert_eq!(parse_shortcut("Ctrl+Shift+3", true), Some((6, 51)));
    assert!(parse_shortcut("A", true).is_none());
    assert!(parse_shortcut("Ctrl+Alt+Q", true).is_none());
    assert!(parse_shortcut("Esc", false).is_some());
}
