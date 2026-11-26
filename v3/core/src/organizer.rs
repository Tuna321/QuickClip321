use serde_json::Value;
use std::sync::OnceLock;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

fn normalized(s: &str) -> String {
    let folded: String = s
        .to_lowercase()
        .replace('ı', "i")
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn text<'a>(info: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|k| info[*k].as_str().filter(|v| !v.is_empty()))
        .unwrap_or("")
}
fn contains(s: &str, term: &str) -> bool {
    format!(" {s} ").contains(&format!(" {term} "))
}
fn any(s: &str, terms: &[&str]) -> bool {
    terms.iter().any(|t| contains(s, t))
}
fn aliases(entry: &Value) -> Vec<String> {
    std::iter::once(entry["name"].as_str().unwrap_or(""))
        .chain(
            entry["aliases"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        )
        .map(normalized)
        .filter(|s| !s.is_empty())
        .collect()
}
fn database() -> &'static Value {
    static DB: OnceLock<Value> = OnceLock::new();
    DB.get_or_init(|| {
        serde_json::from_str(include_str!("organizer_data.json"))
            .expect("built-in organizer database")
    })
}
pub fn counts() -> [usize; 4] {
    let db = database();
    ["artists", "movies", "tv", "games"].map(|k| db[k].as_array().map_or(0, Vec::len))
}
pub fn category(info: &Value, preset: &str) -> (&'static str, &'static str) {
    let title = text(info, &["title"]);
    let n = normalized(title);
    if matches!(preset, "mp3" | "m4a" | "wav") || text(info, &["vcodec"]) == "none" {
        let evidence = normalized(&format!(
            "{title} {} {}",
            text(info, &["track"]),
            text(info, &["genre"])
        ));
        return if any(
            &evidence,
            &[
                "instrumental",
                "beat",
                "beats",
                "type beat",
                "karaoke",
                "enstrumantal",
                "altyapi",
                "backing track",
            ],
        ) {
            ("beats", "Instrumental or beat")
        } else {
            ("audio", "Audio media")
        };
    }
    let db = database();
    let game = normalized(text(info, &["game", "game_title"]));
    let explicit_screen = !text(info, &["series", "tv_show", "movie", "movie_title"]).is_empty();
    let game_context = any(
        &n,
        &[
            "gameplay",
            "walkthrough",
            "playthrough",
            "speedrun",
            "boss fight",
            "game trailer",
            "gameplay trailer",
            "oynanis",
            "oyun",
            "let s play",
        ],
    );
    // Title-prefix numbered trailers for known games; require gaming evidence for ambiguous cross-media names.
    for e in db["games"].as_array().into_iter().flatten() {
        let a = aliases(e);
        if (!game.is_empty() && a.contains(&game))
            || a.iter().any(|v| {
                if !contains(&n, v) {
                    return false;
                }
                if game_context && !explicit_screen {
                    return true;
                }
                let distinctive = v.split_whitespace().count() >= 3
                    || matches!(
                        v.as_str(),
                        "minecraft"
                            | "fortnite"
                            | "valorant"
                            | "roblox"
                            | "gta 6"
                            | "gta vi"
                            | "gta 5"
                            | "gta v"
                            | "gta6"
                            | "gta5"
                    );
                distinctive
                    && (n.starts_with(&format!("{v} trailer"))
                        || n.starts_with(&format!("{v} official trailer"))
                        || n == format!("official {v} trailer"))
            })
        {
            return ("games", "Game title and gaming context");
        }
    }
    let series = normalized(text(info, &["series", "tv_show"]));
    let movie = normalized(text(info, &["movie", "movie_title"]));
    let clip = any(
        &n,
        &[
            "scene", "clip", "sahne", "fragman", "trailer", "film", "dizi", "bolum", "episode",
        ],
    );
    let tv_context = any(&n, &["dizi", "bolum", "episode", "season", "sezon"]);
    for key in ["movies", "tv"] {
        for e in db[key].as_array().into_iter().flatten() {
            let a = aliases(e);
            if (!series.is_empty() && a.contains(&series))
                || (!movie.is_empty() && a.contains(&movie))
                || (clip
                    && a.iter().any(|v| {
                        if !contains(&n, v) {
                            return false;
                        }
                        if !e["ambiguous"].as_bool().unwrap_or(true) {
                            return true;
                        }
                        let contextual = if key == "tv" {
                            tv_context || any(&n, &["scene", "clip", "sahne", "trailer", "fragman"])
                        } else {
                            any(&n, &["scene", "trailer", "fragman", "sahne", "film"])
                        };
                        if !contextual {
                            return false;
                        }
                        // Short titles such as Dark, Her or Glass need a title boundary and adjacent media context.
                        if v.split_whitespace().count() == 1 && v.len() <= 6 {
                            return [
                                "scene", "trailer", "fragman", "sahne", "episode", "bolum", "film",
                                "dizi", "clip",
                            ]
                            .iter()
                            .any(|marker| {
                                n == format!("{v} {marker}")
                                    || n.starts_with(&format!("{v} {marker} "))
                                    || n == format!("{marker} {v}")
                            });
                        }
                        true
                    }))
            {
                return ("film", "Movie or TV metadata");
            }
        }
    }
    if !series.is_empty() {
        return ("film", "Series metadata");
    }
    let music = any(
        &n,
        &[
            "official video",
            "music video",
            "resmi klip",
            "official music video",
            "video klip",
            "official audio",
            "lyric video",
            "official visualizer",
            "official lyrics",
        ],
    );
    let commentary = any(
        &n,
        &[
            "interview",
            "roportaj",
            "belgesel",
            "documentary",
            "haber",
            "review",
            "reaction",
            "tepki",
            "inceleme",
        ],
    );
    let artist = normalized(text(info, &["artist", "album_artist"]));
    let prefix = normalized(
        title
            .split(" - ")
            .next()
            .unwrap_or(title)
            .split(" | ")
            .next()
            .unwrap_or(title),
    );
    let channel = normalized(text(info, &["channel", "uploader"]));
    let channel = channel
        .split_whitespace()
        .filter(|v| !matches!(*v, "official" | "topic" | "vevo" | "resmi"))
        .collect::<Vec<_>>()
        .join(" ");
    let genre = normalized(text(info, &["genre"]));
    let track = text(info, &["track"]);
    let music_context =
        music || !track.is_empty() || any(&genre, &["hip hop", "rap", "trap", "drill"]);
    if !commentary {
        for e in db["artists"].as_array().into_iter().flatten() {
            let a = aliases(e);
            if (!artist.is_empty() && a.contains(&artist))
                || (a.contains(&prefix) && a.contains(&channel) && title.contains(" - "))
                || (music_context
                    && (a.contains(&prefix)
                        || a.contains(&channel)
                        || (!e["ambiguous"].as_bool().unwrap_or(true)
                            && a.iter().any(|v| contains(&n, v)))))
            {
                return ("rap", "Rap artist metadata");
            }
        }
    }
    ("video", "Video media")
}
pub fn configured_destination(
    info: &Value,
    preset: &str,
    root: &std::path::Path,
    p: &crate::preferences::Preferences,
) -> std::path::PathBuf {
    let title = normalized(text(info, &["title"]));
    let category = p
        .rules
        .iter()
        .find(|r| {
            let term = normalized(&r.contains);
            !term.is_empty() && contains(&title, &term)
        })
        .map(|r| r.category.as_str())
        .unwrap_or_else(|| category(info, preset).0);
    p.category_paths.get(category).cloned().unwrap_or_else(|| {
        root.join(if p.detailed_folders {
            match category {
                "rap" => "Video/Rap",
                "games" => "Video/Games",
                "film" => "Video/Film and TV",
                "beats" => "Audio/Beats",
                "audio" => "Audio/Music",
                _ => "Video/Other",
            }
        } else {
            folder(category)
        })
    })
}
pub fn folder(category: &str) -> &'static str {
    match category {
        "beats" | "audio" => "Audio",
        "film" | "rap" | "video" | "games" => "Video",
        _ => "Unsorted",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn configured_rules_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = crate::preferences::Preferences::default();
        p.rules.push(crate::preferences::Rule {
            contains: "Rick and Morty".into(),
            category: "rap".into(),
        });
        p.category_paths
            .insert("rap".into(), dir.path().join("custom"));
        assert_eq!(
            configured_destination(
                &json!({"title":"Rick and Morty episode"}),
                "editor",
                dir.path(),
                &p
            ),
            dir.path().join("custom")
        );
        assert_eq!(
            configured_destination(&json!({"title":"other"}), "editor", dir.path(), &p),
            dir.path().join("Video")
        );
    }
    #[test]
    fn all_catalog_records_recognized_in_context() {
        let db = database();
        for e in db["artists"].as_array().unwrap() {
            let name = e["name"].as_str().unwrap();
            assert_eq!(
                category(
                    &json!({"artist":name,"title":"Fixture (Official Audio)"}),
                    "editor"
                )
                .0,
                "rap",
                "{name}"
            );
        }
        for k in ["movies", "tv"] {
            for e in db[k].as_array().unwrap() {
                let name = e["name"].as_str().unwrap();
                let info = if k == "movies" {
                    json!({"movie":name})
                } else {
                    json!({"series":name})
                };
                assert_eq!(category(&info, "editor").0, "film", "{name}");
            }
        }
        let d = tempfile::tempdir().unwrap();
        let mut p = crate::preferences::Preferences::default();
        p.detailed_folders = true;
        assert_eq!(
            configured_destination(&json!({"artist":"XXXTENTACION"}), "editor", d.path(), &p),
            d.path().join("Video/Rap")
        );
    }
    #[test]
    fn game_catalog_and_context() {
        let db = database();
        assert!(counts()[3] >= 100);
        for e in db["games"].as_array().unwrap() {
            let name = e["name"].as_str().unwrap();
            assert_eq!(
                category(&json!({"game":name}), "editor").0,
                "games",
                "{name}"
            );
        }
        for (title, expected) in [
            ("Grand Theft Auto VI Trailer 2", "games"),
            ("GTA 6 gameplay", "games"),
            ("Roblox gameplay", "games"),
            ("Rust programming tutorial", "video"),
            ("Portal door installation", "video"),
            ("Control panel tutorial", "video"),
            ("The Last of Us episode 1", "film"),
            (
                "Rick and Morty | Season 8 Official Trailer | adult swim",
                "film",
            ),
            ("Oppenheimer | Official Trailer", "film"),
        ] {
            assert_eq!(
                category(&json!({"title":title}), "editor").0,
                expected,
                "{title}"
            );
        }
    }
    #[test]
    fn labeled_sample() {
        let rows: Value = serde_json::from_str(include_str!("organizer_sample.json")).unwrap();
        let mut totals = std::collections::BTreeMap::<String, (usize, usize, usize)>::new();
        let mut correct = 0;
        let mut wrong = vec![];
        for row in rows.as_array().unwrap() {
            let expected = row["expected"].as_str().unwrap();
            let actual = category(&row["info"], row["preset"].as_str().unwrap_or("editor")).0;
            if actual == expected {
                correct += 1;
                totals.entry(expected.into()).or_default().0 += 1;
            } else {
                totals.entry(actual.into()).or_default().1 += 1;
                totals.entry(expected.into()).or_default().2 += 1;
                wrong.push(format!("{}: {expected} -> {actual}", row["info"]["title"]));
            }
        }
        println!(
            "Labeled sample: {correct}/{}; (TP, FP, FN) = {totals:?}",
            rows.as_array().unwrap().len()
        );
        assert!(wrong.is_empty(), "{wrong:?}");
    }
    #[test]
    fn expanded_catalog_cases() {
        let n = counts();
        assert!(n[0] >= 200 && n[1] >= 200 && n[2] >= 200);
        for (title, preset, expected) in [
            (
                "XXXTENTACION - SAD! (Official Music Video)",
                "editor",
                "rap",
            ),
            (
                "Juice WRLD - Lucid Dreams (Official Audio)",
                "editor",
                "rap",
            ),
            ("Rick and Morty episode 1", "editor", "film"),
            ("Oppenheimer trailer", "editor", "film"),
            ("Drake interview", "editor", "video"),
            ("Drake type beat", "mp3", "beats"),
            ("How to repair a glass door", "editor", "video"),
            ("Dark mode tutorial", "editor", "video"),
            ("Future of computers", "editor", "video"),
            ("Glass door repair scene", "editor", "video"),
            ("Dark mode tutorial episode 1", "editor", "video"),
            ("Dark episode 1", "editor", "film"),
            ("Random song", "mp3", "audio"),
        ] {
            assert_eq!(
                category(&json!({"title":title}), preset).0,
                expected,
                "{title}"
            );
        }
    }
    #[test]
    fn categories() {
        assert_eq!(
            category(&json!({"title":"Kendrick type beat"}), "mp3").0,
            "beats"
        );
        assert_eq!(
            category(&json!({"title":"Ceza - interview"}), "editor").0,
            "video"
        );
        assert_eq!(
            category(&json!({"title":"Ceza - Official Music Video"}), "editor").0,
            "rap"
        );
        assert_eq!(
            category(&json!({"title":"Interstellar scene"}), "editor").0,
            "film"
        );
    }
}
