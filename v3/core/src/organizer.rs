use serde_json::Value;
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
    let db: Value = serde_json::from_str(include_str!("organizer_data.json"))
        .expect("built-in organizer database");
    let series = normalized(text(info, &["series", "tv_show"]));
    let movie = normalized(text(info, &["movie", "movie_title"]));
    let clip = any(
        &n,
        &[
            "scene", "clip", "sahne", "fragman", "trailer", "film", "dizi", "bolum", "episode",
        ],
    );
    let tv_context = any(&n, &["dizi", "bolum", "episode"]);
    for key in ["movies", "tv"] {
        for e in db[key].as_array().into_iter().flatten() {
            let a = aliases(e);
            if (!series.is_empty() && a.contains(&series))
                || (!movie.is_empty() && a.contains(&movie))
                || (clip
                    && a.iter().any(|v| contains(&n, v))
                    && (!e["ambiguous"].as_bool().unwrap_or(true) || (key == "tv" && tv_context)))
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
    if !commentary {
        for e in db["artists"].as_array().into_iter().flatten() {
            let a = aliases(e);
            if (!artist.is_empty() && a.contains(&artist))
                || (music
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
    p.category_paths
        .get(category)
        .cloned()
        .unwrap_or_else(|| root.join(folder(category)))
}
pub fn folder(category: &str) -> &'static str {
    match category {
        "beats" | "audio" => "Audio",
        "film" | "rap" | "video" => "Video",
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
