use super::{command, err, terminate, ProcessTree, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct MediaOptions {
    pub clip_start: Option<f64>,
    pub clip_end: Option<f64>,
    pub subtitle_languages: Vec<String>,
    pub subtitle_format: String,
    pub subtitle_auto: bool,
    pub archive: bool,
}
impl MediaOptions {
    pub fn validate(&self) -> Result<()> {
        for time in [self.clip_start, self.clip_end].into_iter().flatten() {
            if !time.is_finite() || time < 0. || time > 86400. * 30. {
                return Err("Clip times must be finite and non-negative.".into());
            }
        }
        if self
            .clip_end
            .is_some_and(|end| end <= self.clip_start.unwrap_or(0.))
        {
            return Err("Clip end must be after start.".into());
        }
        if self.subtitle_languages.len() > 12
            || self.subtitle_languages.iter().any(|l| {
                l.is_empty()
                    || l.len() > 32
                    || !l
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
        {
            return Err("Enter valid subtitle language codes.".into());
        }
        if !self.subtitle_languages.is_empty()
            && !matches!(self.subtitle_format.as_str(), "srt" | "vtt")
        {
            return Err("Choose SRT or VTT.".into());
        }
        Ok(())
    }
    pub fn clipping(&self) -> bool {
        self.clip_start.is_some() || self.clip_end.is_some()
    }
}
fn duration(path: &Path, ffprobe: &Path) -> Result<f64> {
    let out = command(ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .map_err(err)?;
    if !out.status.success() {
        return Err("Cannot inspect clip duration.".into());
    }
    let d = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .map_err(err)?;
    if !d.is_finite() || d <= 0. {
        return Err("Invalid media duration.".into());
    }
    Ok(d)
}
pub fn clip(
    source: &Path,
    ffmpeg: &Path,
    options: &MediaOptions,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    if !options.clipping() {
        return Ok(source.to_owned());
    }
    let probe = ffmpeg.with_file_name(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    let d = duration(source, &probe)?;
    let start = options.clip_start.unwrap_or(0.);
    let end = options.clip_end.unwrap_or(d);
    if start >= d || end > d + 0.05 {
        return Err("Clip range is outside the media duration.".into());
    }
    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid clip filename")?;
    let audio = matches!(
        ext.as_str(),
        "mp3" | "m4a" | "wav" | "aac" | "opus" | "flac" | "ogg"
    );
    let target = source.with_file_name(format!(
        "{stem} [clip {start}-{end}].{}",
        if audio { ext.as_str() } else { "mp4" }
    ));
    let mut c = command(ffmpeg);
    c.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-i"])
        .arg(source)
        .args([
            "-ss",
            &format!("{start:.6}"),
            "-t",
            &format!("{:.6}", end - start),
        ]);
    if audio {
        c.args([
            "-map",
            "0:a:0",
            "-vn",
            "-c:a",
            match ext.as_str() {
                "mp3" => "libmp3lame",
                "m4a" | "aac" => "aac",
                "wav" => "pcm_s16le",
                "opus" | "ogg" => "libopus",
                "flac" => "flac",
                _ => "copy",
            },
        ]);
    } else {
        c.args([
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-movflags",
            "+faststart",
        ]);
    }
    c.arg(&target).stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = c.spawn().map_err(err)?;
    let _owner = ProcessTree::attach(&mut child)?;
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(4 * 3600) {
            terminate(&mut child);
            return Err("Clipping cancelled or timed out.".into());
        }
        match child.try_wait() {
            Ok(Some(s)) if s.success() => break,
            Ok(Some(s)) => return Err(format!("Clip conversion failed: {s}")),
            Ok(None) => {}
            Err(e) => {
                terminate(&mut child);
                return Err(err(e));
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    let actual = duration(&target, &probe)?;
    let expected = end - start;
    if (actual - expected).abs() > 0.25_f64.max(expected * 0.02) {
        return Err("Clip duration verification failed.".into());
    }
    Ok(target)
}
fn parse_stamp(s: &str) -> Option<f64> {
    let values = s.replace(',', ".");
    let mut t = 0.;
    let mut count = 0;
    for part in values.split(':') {
        t = t * 60. + part.parse::<f64>().ok()?;
        count += 1;
    }
    if (2..=3).contains(&count) && t.is_finite() && t >= 0. {
        Some(t)
    } else {
        None
    }
}
fn stamp(t: f64, srt: bool) -> String {
    let ms = (t.max(0.) * 1000.).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        if srt { ',' } else { '.' },
        ms % 1000
    )
}
pub fn trim_subtitle(text: &str, options: &MediaOptions, srt: bool) -> String {
    if !options.clipping() {
        return text.to_owned();
    }
    let start = options.clip_start.unwrap_or(0.);
    let end = options.clip_end.unwrap_or(f64::INFINITY);
    let normalized = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut result = vec![];
    for block in normalized.split("\n\n") {
        let lines = block.lines().collect::<Vec<_>>();
        let Some((index, line)) = lines
            .iter()
            .enumerate()
            .find(|(_, line)| line.contains(" --> "))
        else {
            continue;
        };
        let Some((a, b)) = line.split_once(" --> ") else {
            continue;
        };
        let (b, suffix) = b.split_once(' ').unwrap_or((b, ""));
        let (Some(a), Some(b)) = (parse_stamp(a.trim()), parse_stamp(b.trim())) else {
            continue;
        };
        let low = a.max(start);
        let high = b.min(end);
        if high <= low {
            continue;
        }
        let mut cue = vec![];
        if srt {
            cue.push((result.len() + 1).to_string());
        }
        cue.push(format!(
            "{} --> {}{}",
            stamp(low - start, srt),
            stamp(high - start, srt),
            if suffix.is_empty() {
                String::new()
            } else {
                format!(" {suffix}")
            }
        ));
        cue.extend(lines[index + 1..].iter().map(|v| v.to_string()));
        result.push(cue.join("\n"));
    }
    format!(
        "{}{}\n",
        if srt { "" } else { "WEBVTT\n\n" },
        result.join("\n\n")
    )
}
pub fn subtitle_files(source: &Path, options: &MediaOptions) -> Result<Vec<PathBuf>> {
    if options.subtitle_languages.is_empty() {
        return Ok(vec![]);
    }
    let parent = source.parent().ok_or("Missing media parent")?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid media filename")?;
    let mut files = vec![];
    for entry in fs::read_dir(parent).map_err(err)? {
        let entry = entry.map_err(err)?;
        let path = entry.path();
        if entry.file_type().map_err(err)?.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some(options.subtitle_format.as_str())
            && entry
                .file_name()
                .to_string_lossy()
                .starts_with(&format!("{stem}."))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clip_options() {
        let mut o = MediaOptions {
            clip_start: Some(5.),
            clip_end: Some(4.),
            ..Default::default()
        };
        assert!(o.validate().is_err());
        o.clip_end = Some(9.);
        assert!(o.validate().is_ok());
        o.subtitle_languages = vec![".*".into()];
        assert!(o.validate().is_err());
    }
    #[test]
    fn shift_subtitles() {
        let o = MediaOptions {
            clip_start: Some(2.),
            clip_end: Some(4.),
            ..Default::default()
        };
        let s =
            "1\n00:00:01,000 --> 00:00:03,000\nHello\n\n2\n00:00:04,000 --> 00:00:05,000\nDrop\n";
        let out = trim_subtitle(s, &o, true);
        assert!(out.contains("00:00:00,000 --> 00:00:01,000"));
        assert!(!out.contains("Drop"));
    }
}
