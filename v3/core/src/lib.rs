pub mod media_options;
pub mod organizer;
pub mod preferences;
use media_options::MediaOptions;
use preferences::Preferences;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Paused,
    Running,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub url: String,
    pub preset: String,
    pub quality: String,
    pub playlist: bool,
    #[serde(default)]
    pub organize: bool,
    #[serde(default)]
    pub options: MediaOptions,
    pub destination: PathBuf,
    pub status: Status,
    pub progress: f64,
    pub message: String,
    pub files: Vec<PathBuf>,
    #[serde(default)]
    pub resume_on_startup: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub url: String,
    pub preset: String,
    pub quality: String,
    pub playlist: bool,
    #[serde(default)]
    pub organize: bool,
    #[serde(default)]
    pub options: MediaOptions,
    pub destination: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub destination: PathBuf,
    pub yt_dlp: PathBuf,
    pub ffmpeg: PathBuf,
    #[serde(default)]
    pub prefs: Preferences,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub settings: Settings,
    pub jobs: Vec<Job>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn validate_url(raw: &str) -> Result<String> {
    if raw.len() > 4096 || raw.chars().any(char::is_whitespace) {
        return Err("Enter a valid media URL.".into());
    }
    let normalized = if raw.get(..4).is_some_and(|v| v.eq_ignore_ascii_case("www.")) {
        format!("https://{raw}")
    } else {
        raw.to_owned()
    };
    let parsed = url::Url::parse(&normalized)
        .map_err(|_| "Enter a valid HTTP or HTTPS media URL.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Only HTTP(S) links without embedded credentials are allowed.".into());
    }
    Ok(parsed.into())
}
#[derive(Debug, Serialize)]
pub struct Preview {
    pub title: String,
    pub duration: Option<f64>,
    pub channel: String,
    pub qualities: Vec<u64>,
    pub playlist: bool,
    pub count: Option<u64>,
    pub thumbnail: Option<String>,
    pub sizes: Vec<PreviewSize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewSize {
    pub preset: String,
    pub quality: String,
    pub bytes: u64,
}
#[cfg(test)]
#[test]
fn preview_size_metadata_only() {
    let info = serde_json::json!({"formats":[
        {"vcodec":"none","acodec":"mp4a.40.2","filesize_approx":100},
        {"vcodec":"avc1.64001f","acodec":"none","height":720,"filesize":1000},
        {"vcodec":"avc1.640028","acodec":"none","height":1080,"filesize":2000}
    ]});
    let estimates = preview_sizes(&info);
    assert_eq!(
        estimates
            .iter()
            .find(|s| s.preset == "editor" && s.quality == "720")
            .unwrap()
            .bytes,
        1100
    );
    assert_eq!(
        estimates
            .iter()
            .find(|s| s.preset == "editor" && s.quality == "1080")
            .unwrap()
            .bytes,
        2100
    );
    assert_eq!(
        estimates.iter().find(|s| s.preset == "mp3").unwrap().bytes,
        100
    );
    assert!(preview_sizes(&serde_json::json!({"formats":[]})).is_empty());
}
fn preview_sizes(info: &serde_json::Value) -> Vec<PreviewSize> {
    // Metadata-only estimate; no HEAD requests, no changes to yt-dlp download selection.
    let Some(formats) = info["formats"].as_array() else {
        return vec![];
    };
    let mut result = vec![];
    let is_video = |f: &serde_json::Value| f["vcodec"].as_str().is_some_and(|s| s != "none");
    let is_audio = |f: &serde_json::Value| f["acodec"].as_str().is_some_and(|s| s != "none");
    let size = |f: &serde_json::Value| {
        f["filesize"]
            .as_u64()
            .or_else(|| f["filesize_approx"].as_u64())
            .filter(|n| *n > 0)
    };
    let audio = formats.iter().rev().find(|f| is_audio(f) && !is_video(f));
    for q in ["best", "2160", "1440", "1080", "720", "480", "360"] {
        let cap = q.parse::<u64>().ok();
        let fits = |f: &serde_json::Value| {
            cap.is_none_or(|h| f["height"].as_u64().is_some_and(|v| v <= h))
        };
        for preset in ["editor", "original"] {
            let preferred = if preset == "editor" {
                formats
                    .iter()
                    .rev()
                    .find(|f| {
                        fits(f)
                            && is_video(f)
                            && !is_audio(f)
                            && f["vcodec"].as_str().is_some_and(|s| s.starts_with("avc1"))
                    })
                    .zip(formats.iter().rev().find(|f| {
                        is_audio(f)
                            && !is_video(f)
                            && f["acodec"].as_str().is_some_and(|s| s.starts_with("mp4a"))
                    }))
            } else {
                None
            };
            let selected = preferred.or_else(|| {
                formats
                    .iter()
                    .rev()
                    .find(|f| fits(f) && is_video(f))
                    .and_then(|v| {
                        if is_audio(v) {
                            Some((v, v))
                        } else {
                            audio.map(|a| (v, a))
                        }
                    })
            });
            let bytes = selected.and_then(|(v, a)| {
                if std::ptr::eq(v, a) {
                    size(v)
                } else {
                    size(v)?.checked_add(size(a)?)
                }
            });
            if let Some(bytes) = bytes {
                result.push(PreviewSize {
                    preset: preset.into(),
                    quality: q.into(),
                    bytes,
                });
            }
        }
    }
    if let Some(bytes) = audio
        .or_else(|| formats.iter().rev().find(|f| is_audio(f)))
        .and_then(size)
    {
        for preset in ["mp3", "m4a", "wav"] {
            result.push(PreviewSize {
                preset: preset.into(),
                quality: "best".into(),
                bytes,
            });
        }
    }
    result
}
pub fn is_playlist(raw: &str) -> bool {
    let Ok(u) = url::Url::parse(raw) else {
        return false;
    };
    u.query_pairs()
        .any(|(key, val)| key == "list" && !val.is_empty())
        || u.path()
            .to_ascii_lowercase()
            .split('/')
            .any(|p| matches!(p, "playlist" | "playlists" | "sets" | "albums" | "album"))
}
pub fn managed_node_path(tool: &Path) -> Option<PathBuf> {
    let dir = tool.parent()?;
    match dir.file_name()?.to_str()? {
        "managed-2026.11.23-9.1-24.22.3" => {
            let p = dir.join("node.exe");
            p.is_file().then_some(p)
        }
        "yt-dlp-2026.11.23" => {
            let p = dir
                .parent()?
                .join("managed-2026.11.23-9.1-24.22.3/node.exe");
            p.is_file().then_some(p)
        }
        _ => None,
    }
}
fn js_runtime(tool: &Path) -> String {
    managed_node_path(tool)
        .map(|p| format!("node:{}", p.display()))
        .unwrap_or_else(|| "node".into())
}
pub fn preview(raw: &str, tool: &Path) -> Result<Preview> {
    preview_cancel(raw, tool, &AtomicBool::new(false))
}
pub fn preview_cancel(raw: &str, tool: &Path, cancel: &AtomicBool) -> Result<Preview> {
    use std::io::{Read, Seek, SeekFrom};
    let url = validate_url(raw.trim())?;
    if !executable_file(tool) {
        return Err("Media engine is not configured.".into());
    }
    if cancel.load(Ordering::SeqCst) {
        return Err("Preview superseded.".into());
    }
    let mut output = tempfile::tempfile().map_err(err)?;
    let thumbs = tempfile::tempdir().map_err(err)?;
    let mut c = command(tool);
    c.args([
        "--ignore-config",
        "--no-plugin-dirs",
        "--no-cache-dir",
        "--no-warnings",
        "--skip-download",
        "--dump-single-json",
        "--flat-playlist",
        "--no-simulate",
        "--write-thumbnail",
        "--playlist-items",
        "1",
        "--socket-timeout",
        "8",
        "--retries",
        "0",
        "--extractor-retries",
        "0",
        "--js-runtimes",
        &js_runtime(tool),
    ]);
    c.arg("-o").arg(format!(
        "thumbnail:{}",
        thumbs.path().join("preview.%(ext)s").display()
    ));
    if is_playlist(&url) {
        c.arg("--yes-playlist");
    } else {
        c.arg("--no-playlist");
    }
    #[cfg(test)]
    if url.starts_with("http://127.0.0.1:") {
        c.args(["--proxy", ""]);
    }
    c.arg("--")
        .arg(&url)
        .stdout(Stdio::from(output.try_clone().map_err(err)?))
        .stderr(Stdio::null());
    let mut child = c.spawn().map_err(err)?;
    let _owner = ProcessTree::attach(&mut child)?;
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst)
            || started.elapsed() > Duration::from_secs(22)
            || output.metadata().map_err(err)?.len() > 8_000_000
        {
            terminate(&mut child);
            return Err("Preview cancelled, timed out or exceeded its size limit.".into());
        }
        match child.try_wait() {
            Ok(Some(s)) if s.success() => break,
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => {
                terminate(&mut child);
                return Err(err(e));
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    output.seek(SeekFrom::Start(0)).map_err(err)?;
    let mut bytes = Vec::new();
    output
        .take(8_000_001)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > 8_000_000 {
        return Err("Preview too large.".into());
    }
    let info: serde_json::Value = serde_json::from_slice(&bytes).map_err(err)?;
    if info["title"].as_str().is_none_or(|s| s.trim().is_empty())
        && info["entries"].as_array().is_none_or(Vec::is_empty)
    {
        return Err("No media metadata returned by the source.".into());
    }
    let mut qualities = info["formats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["height"].as_u64())
        .filter(|h| *h > 0)
        .collect::<Vec<_>>();
    qualities.sort_unstable();
    qualities.dedup();
    qualities.reverse();
    Ok(Preview {
        title: info["title"]
            .as_str()
            .unwrap_or("Untitled media")
            .chars()
            .take(180)
            .collect(),
        duration: info["duration"].as_f64(),
        channel: info["channel"]
            .as_str()
            .or_else(|| info["uploader"].as_str())
            .unwrap_or("")
            .chars()
            .take(100)
            .collect(),
        qualities,
        thumbnail: read_thumbnail(thumbs.path()),
        sizes: if is_playlist(&url)
            || matches!(info["_type"].as_str(), Some("playlist" | "multi_video"))
        {
            vec![]
        } else {
            preview_sizes(&info)
        },
        playlist: matches!(info["_type"].as_str(), Some("playlist" | "multi_video"))
            || is_playlist(&url),
        count: info["playlist_count"]
            .as_u64()
            .or_else(|| info["n_entries"].as_u64()),
    })
}
fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        s.push(TABLE[(a >> 2) as usize] as char);
        s.push(TABLE[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        s.push(if chunk.len() > 1 {
            TABLE[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            TABLE[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}
fn read_thumbnail(root: &Path) -> Option<String> {
    use std::io::Read;
    for entry in fs::read_dir(root).ok()?.flatten() {
        if !entry.file_type().ok()?.is_file() {
            continue;
        }
        let mut bytes = vec![];
        if File::open(entry.path())
            .ok()?
            .take(2_000_001)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > 2_000_000
        {
            continue;
        }
        let mime = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "image/webp"
        } else {
            continue;
        };
        return Some(format!("data:{mime};base64,{}", encode_base64(&bytes)));
    }
    None
}
pub fn selector(preset: &str, quality: &str) -> Result<String> {
    if !matches!(
        quality,
        "best" | "2160" | "1440" | "1080" | "720" | "480" | "360"
    ) {
        return Err("Unknown quality.".into());
    }
    let cap = if quality == "best" {
        String::new()
    } else {
        format!("[height<={quality}]")
    };
    let video = format!("bestvideo*{cap}+bestaudio/best{cap}");
    match preset {
        "editor" => {
            // Generic direct media sometimes has no extractor height. Prefer known capped
            // formats first; verify/limit the actual height during editor conversion.
            let unknown = if quality == "best" {
                String::new()
            } else {
                format!("/best[height<=?{quality}]")
            };
            Ok(format!(
                "bestvideo{cap}[vcodec^=avc1]+bestaudio[acodec^=mp4a]/{video}{unknown}"
            ))
        }
        "original" => Ok(video),
        "subtitles" => Ok("best".into()),
        "mp3" | "wav" | "m4a" => Ok("bestaudio/best".into()),
        _ => Err("Unknown preset.".into()),
    }
}
fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().ok_or("Missing state directory")?;
    fs::create_dir_all(dir).map_err(err)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(err)?;
    tmp.write_all(bytes).map_err(err)?;
    tmp.as_file().sync_all().map_err(err)?;
    tmp.persist(path).map_err(err)?;
    Ok(())
}
fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.backup")
}
fn persist(path: &Path, state: &Snapshot) -> Result<()> {
    // Never replace a good backup with unreadable or corrupt state.
    match fs::read(path) {
        Ok(bytes) if serde_json::from_slice::<Snapshot>(&bytes).is_ok() => {
            atomic_bytes(&backup_path(path), &bytes)?;
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(err(e)),
    }
    let dir = path.parent().ok_or("Missing state directory")?;
    fs::create_dir_all(dir).map_err(err)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(err)?;
    serde_json::to_writer(&mut tmp, state).map_err(err)?;
    tmp.as_file().sync_all().map_err(err)?;
    tmp.persist(path).map_err(err)?;
    Ok(())
}
struct Inner {
    state: Snapshot,
    active: std::collections::HashMap<String, Arc<AtomicBool>>,
    pause_requests: std::collections::HashSet<String>,
    notices: std::collections::VecDeque<bool>,
    shutting_down: bool,
    maintenance: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct SetupProgress {
    pub stage: String,
    pub received: u64,
    pub total: u64,
}
impl Default for SetupProgress {
    fn default() -> Self {
        Self {
            stage: "Preparing media tools".into(),
            received: 0,
            total: 0,
        }
    }
}
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Mutex<Inner>>,
    setup_progress: Arc<Mutex<SetupProgress>>,
    file: PathBuf,
    recovery: Option<String>,
}
struct ToolMaintenance(Engine);
impl Drop for ToolMaintenance {
    fn drop(&mut self) {
        self.0.inner.lock().unwrap().maintenance = false;
    }
}
impl Engine {
    fn tool_maintenance(&self) -> Result<ToolMaintenance> {
        let mut x = self.inner.lock().unwrap();
        if x.maintenance
            || x.shutting_down
            || !x.active.is_empty()
            || x.state
                .jobs
                .iter()
                .any(|j| matches!(j.status, Status::Running | Status::Queued))
        {
            return Err("Finish active downloads before changing media tools.".into());
        }
        x.maintenance = true;
        Ok(ToolMaintenance(self.clone()))
    }

    pub fn open(file: PathBuf, defaults: Settings) -> Result<Self> {
        let fresh = || Snapshot {
            settings: defaults.clone(),
            jobs: vec![],
        };
        let mut recovery = None;
        let mut state = match fs::read(&file) {
            Ok(bytes) => match serde_json::from_slice::<Snapshot>(&bytes) {
                Ok(state) => state,
                Err(_) => {
                    // Preserve exact bytes before writing anything to the original location.
                    let damaged = file.with_extension(format!("json.corrupt-{}", Uuid::new_v4()));
                    atomic_bytes(&damaged, &bytes)?;
                    let backup = fs::read(backup_path(&file))
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Snapshot>(&b).ok());
                    recovery = Some(if backup.is_some() {
                        format!(
                            "Damaged state recovered from backup. Original preserved at {}",
                            damaged.display()
                        )
                    } else {
                        format!("Damaged state preserved at {}. Started with default settings; history needs manual recovery.", damaged.display())
                    });
                    backup.unwrap_or_else(fresh)
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(backup) = fs::read(backup_path(&file))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Snapshot>(&b).ok())
                {
                    recovery = Some("Missing state restored from backup.".into());
                    backup
                } else {
                    fresh()
                }
            }
            Err(e) => return Err(err(e)),
        };
        if state.settings.yt_dlp.as_os_str().is_empty() {
            state.settings.yt_dlp = defaults.yt_dlp;
        }
        if state.settings.ffmpeg.as_os_str().is_empty() {
            state.settings.ffmpeg = defaults.ffmpeg;
        }
        state.settings.prefs.normalize();
        for j in &mut state.jobs {
            if j.status == Status::Queued {
                j.status = Status::Paused;
                j.resume_on_startup = true;
                j.message = "Saved queue; resume when ready".into();
            } else if j.status == Status::Running {
                if j.files.is_empty() && safe_partial(j) {
                    j.status = Status::Paused;
                    j.resume_on_startup = true;
                    j.message = "Interrupted; partial data retained".into();
                } else {
                    j.status = Status::Failed;
                    j.resume_on_startup = false;
                    j.message = "Interrupted. Retry explicitly.".into();
                }
            }
        }
        if recovery.is_some() {
            for job in &mut state.jobs {
                job.resume_on_startup = false;
            }
        }
        persist(&file, &state)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                state,
                active: Default::default(),
                pause_requests: Default::default(),
                notices: Default::default(),
                shutting_down: false,
                maintenance: false,
            })),
            setup_progress: Arc::new(Mutex::new(SetupProgress::default())),
            file,
            recovery,
        })
    }
    pub fn recovery_notice(&self) -> Option<&str> {
        self.recovery.as_deref()
    }
    pub fn setup_progress(&self) -> SetupProgress {
        self.setup_progress.lock().unwrap().clone()
    }
    #[cfg(windows)]
    pub fn install_recommended_ytdlp(&self) -> Result<Settings> {
        let _maintenance = self.tool_maintenance()?;
        let root = self
            .file
            .parent()
            .ok_or("Missing app data directory")?
            .join("tools");
        fs::create_dir_all(&root).map_err(err)?;
        if !ordinary(&fs::symlink_metadata(&root).map_err(err)?) {
            return Err("Unsafe tool directory".into());
        }
        let dir = root.join("yt-dlp-2026.11.23");
        fs::create_dir_all(&dir).map_err(err)?;
        if !ordinary(&fs::symlink_metadata(&dir).map_err(err)?) {
            return Err("Unsafe tool directory".into());
        }
        let target = dir.join("yt-dlp.exe");
        if !verified_ytdlp(&target) {
            if let Ok(meta) = fs::symlink_metadata(&target) {
                if !meta.is_file() || !ordinary(&meta) {
                    return Err("Unsafe managed yt-dlp path.".into());
                }
            }
            let cached = root.join("managed-2026.11.23-9.1-24.22.3/yt-dlp.exe");
            let tmp = tempfile::NamedTempFile::new_in(&dir).map_err(err)?;
            if verified_ytdlp(&cached) {
                fs::copy(&cached, tmp.path()).map_err(err)?;
            } else {
                let system = PathBuf::from(
                    std::env::var_os("SystemRoot").ok_or("Windows directory unavailable")?,
                );
                let mut c = command(&system.join("System32/curl.exe"));
                c.args([
                    "-q",
                    "--silent",
                    "--show-error",
                    "--fail",
                    "--location",
                    "--max-redirs",
                    "5",
                    "--proto",
                    "=https",
                    "--proto-redir",
                    "=https",
                    "--connect-timeout",
                    "10",
                    "--max-time",
                    "120",
                    "--max-filesize",
                    "50000000",
                    "--output",
                ])
                .arg(tmp.path())
                .arg("https://github.com/yt-dlp/yt-dlp/releases/download/2026.11.23/yt-dlp.exe")
                .stdout(Stdio::null())
                .stderr(Stdio::null());
                if !c.status().map_err(err)?.success() {
                    return Err("yt-dlp download failed; previous tool preserved".into());
                }
            }
            if !verified_ytdlp(tmp.path()) {
                return Err("yt-dlp integrity verification failed; previous tool preserved".into());
            }
            if target.exists() {
                let quarantine = dir.join(format!("yt-dlp.corrupt-{}.exe", Uuid::new_v4()));
                fs::rename(&target, &quarantine).map_err(err)?;
                if let Err(e) = tmp.persist_noclobber(&target) {
                    if !target.exists() {
                        let _ = fs::rename(&quarantine, &target);
                    }
                    return Err(err(e));
                }
            } else {
                tmp.persist_noclobber(&target).map_err(err)?;
            }
        }
        if !verified_ytdlp(&target) {
            return Err(
                "Existing managed yt-dlp failed verification; previous tool preserved".into(),
            );
        }
        if tool_version(&target, "--version").as_deref() != Some("2026.11.23") {
            return Err("Verified yt-dlp failed version smoke; previous tool preserved".into());
        }
        let mut x = self.inner.lock().unwrap();
        let mut next = x.state.clone();
        if next.settings.yt_dlp != target {
            if next.settings.yt_dlp.is_file() {
                next.settings.prefs.previous_ytdlp = Some(next.settings.yt_dlp.clone());
            }
            next.settings.yt_dlp = target;
            persist(&self.file, &next)?;
            x.state = next;
        }
        Ok(x.state.settings.clone())
    }
    pub fn rollback_ytdlp(&self) -> Result<Settings> {
        let _maintenance = self.tool_maintenance()?;
        let mut x = self.inner.lock().unwrap();
        let mut next = x.state.clone();
        let previous = next
            .settings
            .prefs
            .previous_ytdlp
            .clone()
            .filter(|p| p.is_absolute() && p.is_file())
            .ok_or("No previous yt-dlp executable available")?;
        next.settings.prefs.previous_ytdlp = Some(next.settings.yt_dlp.clone());
        next.settings.yt_dlp = previous;
        persist(&self.file, &next)?;
        x.state = next;
        Ok(x.state.settings.clone())
    }
    /// Install version-pinned tools in the app-owned directory, not on PATH.
    #[cfg(windows)]
    pub fn setup_managed_tools(&self) -> Result<Settings> {
        static INSTALL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = INSTALL
            .try_lock()
            .map_err(|_| "Tool setup is already running.")?;
        let _maintenance = self.tool_maintenance()?;
        *self.setup_progress.lock().unwrap() = SetupProgress::default();
        if self
            .snapshot()
            .jobs
            .iter()
            .any(|j| matches!(j.status, Status::Running | Status::Queued))
        {
            return Err("Finish or cancel pending downloads before changing media tools.".into());
        }
        let root = self
            .file
            .parent()
            .ok_or("Missing app data directory")?
            .join("tools");
        let system = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .ok_or("Windows system directory is unavailable.")?;
        let ps = system.join("System32/WindowsPowerShell/v1.0/powershell.exe");
        if !ps.is_absolute() || !ps.is_file() {
            return Err("Windows PowerShell is unavailable.".into());
        }
        let body = include_str!("setup-tools.ps1");
        let bytes = body
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let encoded = encode_base64(&bytes);
        let mut c = command(&ps);
        c.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ])
        .env("QC_MANAGED_ROOT", &root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
        let mut child = c.spawn().map_err(err)?;
        let _owner = ProcessTree::attach(&mut child)?;
        let stdout = child.stdout.take().ok_or("Missing setup output")?;
        let progress = self.setup_progress.clone();
        let heartbeat = Arc::new(Mutex::new(Instant::now()));
        let beat = heartbeat.clone();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout)
                .lines()
                .map_while(std::result::Result::ok)
            {
                let Some(message) = line.strip_prefix("QC_SETUP|") else {
                    continue;
                };
                let parts = message.split('|').collect::<Vec<_>>();
                if parts.len() != 3 || parts[0].len() > 100 {
                    continue;
                }
                if let (Ok(received), Ok(total)) = (parts[1].parse(), parts[2].parse()) {
                    *progress.lock().unwrap() = SetupProgress {
                        stage: parts[0].to_owned(),
                        received,
                        total,
                    };
                    *beat.lock().unwrap() = Instant::now();
                }
            }
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status.success()),
                Ok(None) => {}
                Err(e) => {
                    terminate(&mut child);
                    break Err(err(e));
                }
            }
            if heartbeat.lock().unwrap().elapsed() > Duration::from_secs(90) {
                terminate(&mut child);
                break Err(format!(
                    "Setup stopped responding at: {}. Check your connection and retry.",
                    self.setup_progress().stage
                ));
            }
            if start.elapsed() > Duration::from_secs(1800) {
                terminate(&mut child);
                break Err("Tool setup timed out. Retry on a stable connection.".into());
            }
            thread::sleep(Duration::from_millis(200));
        };
        drop(_owner);
        let _ = reader.join();
        if !status? {
            return Err(format!("Setup failed at: {}. Check your connection, disk space or tool integrity and retry.",self.setup_progress().stage));
        }
        let dir = root.join("managed-2026.11.23-9.1-24.22.3");
        let y = dir.join("yt-dlp.exe");
        let f = dir.join("ffmpeg.exe");
        if ![&y, &f, &dir.join("ffprobe.exe"), &dir.join("node.exe")]
            .iter()
            .all(|p| p.is_file())
        {
            return Err("Managed tools are incomplete.".into());
        }
        let mut x = self.inner.lock().unwrap();
        if x.state
            .jobs
            .iter()
            .any(|j| matches!(j.status, Status::Running | Status::Queued | Status::Paused))
        {
            return Err("Tools were installed but not selected because a job is active. Retry setup when idle.".into());
        }
        let mut next = x.state.clone();
        next.settings.yt_dlp = y;
        next.settings.ffmpeg = f;
        persist(&self.file, &next)?;
        x.state = next;
        Ok(x.state.settings.clone())
    }
    pub fn take_notifications(&self) -> Vec<bool> {
        self.inner.lock().unwrap().notices.drain(..).collect()
    }
    pub fn clipboard_enabled(&self) -> bool {
        self.inner
            .lock()
            .unwrap()
            .state
            .settings
            .prefs
            .clipboard_monitor
    }
    pub fn shortcut_config(&self) -> std::collections::BTreeMap<String, String> {
        self.inner
            .lock()
            .unwrap()
            .state
            .settings
            .prefs
            .effective_shortcuts()
    }
    pub fn open_hotkey(&self) -> String {
        self.inner
            .lock()
            .unwrap()
            .state
            .settings
            .prefs
            .open_hotkey
            .clone()
    }
    pub fn popup_topmost(&self) -> bool {
        self.inner
            .lock()
            .unwrap()
            .state
            .settings
            .prefs
            .always_on_top
    }
    pub fn tray_summary(&self) -> (usize, usize, usize, f64, String, String) {
        let x = self.inner.lock().unwrap();
        let (mut active, mut queued, mut paused, mut progress) = (0, 0, 0, 0.);
        let mut speeds = Vec::new();
        for j in &x.state.jobs {
            match j.status {
                Status::Running => {
                    active += 1;
                    progress += j.progress.clamp(0., 1.);
                    if let Some(speed) = j
                        .message
                        .strip_prefix("Downloading ")
                        .and_then(|m| m.split(" | ").nth(1))
                    {
                        if speed.len() < 30 {
                            speeds.push(speed);
                        }
                    }
                }
                Status::Queued => queued += 1,
                Status::Paused => paused += 1,
                _ => {}
            }
        }
        (
            active,
            queued,
            paused,
            if active > 0 {
                progress / active as f64
            } else {
                0.
            },
            speeds.join(" + "),
            x.state.settings.prefs.language.clone(),
        )
    }
    pub fn snapshot(&self) -> Snapshot {
        self.inner.lock().unwrap().state.clone()
    }
    pub fn settings_snapshot(&self) -> Settings {
        self.inner.lock().unwrap().state.settings.clone()
    }
    pub fn active_snapshot(&self) -> Snapshot {
        let x = self.inner.lock().unwrap();
        let state = &x.state;
        let mut jobs = state
            .jobs
            .iter()
            .filter(|j| matches!(j.status, Status::Running | Status::Queued | Status::Paused))
            .cloned()
            .collect::<Vec<_>>();
        if let Some(last) = state.jobs.last() {
            if !jobs.iter().any(|j| j.id == last.id) {
                jobs.push(last.clone());
            }
        }
        Snapshot {
            settings: state.settings.clone(),
            jobs,
        }
    }
    pub fn settings(&self, settings: Settings) -> Result<()> {
        self.modify_settings(true, |current| {
            *current = settings;
            Ok(())
        })
        .map(|_| ())
    }
    // All read/modify/write operations hold the same engine lock as queue updates.
    pub fn modify_settings(
        &self,
        validate_all: bool,
        edit: impl FnOnce(&mut Settings) -> Result<()>,
    ) -> Result<Settings> {
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down || x.maintenance {
            return Err("Wait for maintenance to finish.".into());
        }
        let mut next = x.state.clone();
        edit(&mut next.settings)?;
        let settings = &mut next.settings;
        settings.prefs.normalize();
        let old = &x.state.settings;
        if validate_all || settings.destination != old.destination {
            validate_destination(&settings.destination)?;
        }
        if validate_all || settings.prefs.category_paths != old.prefs.category_paths {
            for path in settings.prefs.category_paths.values() {
                validate_destination(path)?;
            }
        }
        if validate_all || settings.prefs.last_destination != old.prefs.last_destination {
            if let Some(path) = &settings.prefs.last_destination {
                validate_destination(path)?;
            }
        }
        for (tool, previous) in [
            (&settings.yt_dlp, &old.yt_dlp),
            (&settings.ffmpeg, &old.ffmpeg),
        ] {
            if (validate_all || tool != previous)
                && !tool.as_os_str().is_empty()
                && !executable_file(tool)
            {
                return Err("Tool paths must identify existing executable files.".into());
            }
        }
        // An unrelated appearance or shortcut patch must not rewrite a corrupt tool path.
        let result = settings.clone();
        if &result != old {
            persist(&self.file, &next)?;
            x.state = next;
        }
        drop(x);
        self.kick();
        Ok(result)
    }
    pub fn enqueue(&self, r: Request) -> Result<String> {
        self.enqueue_checked(r, true)
    }
    pub fn enqueue_checked(&self, mut r: Request, allow_duplicate: bool) -> Result<String> {
        if r.preset == "subtitles" {
            r.quality = "best".into();
            if r.options.subtitle_languages.is_empty() {
                r.options.subtitle_languages = vec!["en".into(), "tr".into()];
                r.options.subtitle_auto = true;
            }
            if r.options.subtitle_format.is_empty() {
                r.options.subtitle_format = "srt".into();
            }
            if r.options.clipping() || r.options.archive {
                return Err(
                    "Subtitle-only downloads do not support clipping or media archives.".into(),
                );
            }
        }
        let url = validate_url(r.url.trim())?;
        selector(&r.preset, &r.quality)?;
        r.options.validate()?;
        validate_destination(&r.destination)?;
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down || x.maintenance {
            return Err("Saving queue before exit".into());
        }
        if x.state.jobs.iter().any(|j| {
            matches!(j.status, Status::Queued | Status::Running | Status::Paused)
                && same_media_url(&j.url, &url, j.playlist || r.playlist)
                && j.preset == r.preset
                && (j.quality == r.quality || matches!(r.preset.as_str(), "mp3" | "m4a" | "wav"))
                && j.playlist == r.playlist
                && j.organize == r.organize
                && same_output_options(&j.options, &r.options)
                && j.destination == r.destination
        }) {
            return Err("Already in the queue.".into());
        }
        if !allow_duplicate {
            if let Some(j) = x
                .state
                .jobs
                .iter()
                .rev()
                .find(|j| completed_duplicate(j, &r, &url))
            {
                return Err(format!("Duplicate completed job: {}", j.id));
            }
        }
        let id = Uuid::new_v4().simple().to_string();
        let mut next = x.state.clone();
        next.settings.prefs.default_format = r.preset.clone();
        next.settings.prefs.default_quality = r.quality.clone();
        next.settings.prefs.default_preset = if r.options.archive {
            "archive".into()
        } else if r.preset == "editor" && r.quality == "1080" {
            "editor".into()
        } else if r.preset == "original" && r.quality == "best" {
            "original".into()
        } else if r.preset == "mp3" {
            "mp3".into()
        } else {
            "custom".into()
        };
        next.jobs.push(Job {
            id: id.clone(),
            url,
            preset: r.preset,
            quality: r.quality,
            playlist: r.playlist,
            organize: r.organize,
            options: r.options,
            destination: r.destination,
            status: Status::Queued,
            progress: 0.,
            message: "Queued".into(),
            files: vec![],
            resume_on_startup: false,
        });
        persist(&self.file, &next)?;
        x.state = next;
        drop(x);
        self.kick();
        Ok(id)
    }
    pub fn begin_idle_update(&self) -> Result<()> {
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down
            || x.maintenance
            || !x.active.is_empty()
            || x.state
                .jobs
                .iter()
                .any(|j| matches!(j.status, Status::Running | Status::Queued | Status::Paused))
        {
            return Err(
                "Finish pending downloads and tool setup before installing updates.".into(),
            );
        }
        persist(&self.file, &x.state)?;
        x.shutting_down = true;
        Ok(())
    }
    pub fn prepare_safe_exit(&self) -> Result<()> {
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down || x.maintenance {
            return Err("Wait for current maintenance to finish.".into());
        }
        let mut next = x.state.clone();
        let mut running = Vec::new();
        for j in &mut next.jobs {
            if j.status == Status::Queued {
                j.status = Status::Paused;
                j.message = "Saved queue; resume when ready".into();
                j.resume_on_startup = true;
            } else if j.status == Status::Running
                && resumable(j)
                && j.message.starts_with("Downloading")
                && safe_partial(j)
            {
                j.resume_on_startup = true;
                running.push(j.id.clone());
                j.message = "Pausing; preserving partial data".into();
            }
        }
        persist(&self.file, &next)?;
        x.state = next;
        x.shutting_down = true;
        for id in running {
            if let Some(flag) = x.active.get(&id).cloned() {
                x.pause_requests.insert(id);
                flag.store(true, Ordering::SeqCst);
            }
        }
        Ok(())
    }
    pub fn resume_saved_queue(&self) -> Result<usize> {
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down || x.maintenance {
            return Err("Saving queue before exit".into());
        }
        let mut next = x.state.clone();
        let mut count = 0;
        for j in &mut next.jobs {
            if j.status == Status::Paused && j.resume_on_startup {
                if j.progress > 0. && !safe_partial(j) {
                    j.resume_on_startup = false;
                    j.message = "Partial data unavailable; retry explicitly".into();
                    continue;
                }
                j.status = Status::Queued;
                j.resume_on_startup = false;
                j.message = "Queued".into();
                count += 1;
            }
        }
        if count == 0
            && next
                .jobs
                .iter()
                .zip(&x.state.jobs)
                .all(|(a, b)| a.resume_on_startup == b.resume_on_startup)
        {
            return Ok(0);
        }
        persist(&self.file, &next)?;
        x.state = next;
        drop(x);
        if count > 0 {
            self.kick();
        }
        Ok(count)
    }
    pub fn abort_safe_exit(&self) {
        self.inner.lock().unwrap().shutting_down = false;
    }
    pub fn safe_exit_ready(&self) -> bool {
        let x = self.inner.lock().unwrap();
        x.active.is_empty()
            && persist(&self.file, &x.state).is_ok()
            && !x
                .state
                .jobs
                .iter()
                .any(|j| matches!(j.status, Status::Running | Status::Queued))
    }
    pub fn bulk_action(&self, action: &str) -> Result<()> {
        if !matches!(action, "cancel_all" | "clear_pending" | "clear_completed") {
            return Err("Unknown queue action.".into());
        }
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down || x.maintenance {
            return Err("Saving queue before exit".into());
        }
        let mut next = x.state.clone();
        match action {
            "cancel_all" | "clear_pending" => {
                for j in &mut next.jobs {
                    if matches!(j.status, Status::Queued | Status::Paused) {
                        j.status = Status::Cancelled;
                        j.message = "Cancelled".into();
                    }
                }
            }
            "clear_completed" => {
                next.jobs.retain(|j| j.status != Status::Completed);
            }
            _ => unreachable!(),
        }
        persist(&self.file, &next)?;
        x.state = next;
        if action == "cancel_all" {
            x.pause_requests.clear();
            for flag in x.active.values() {
                flag.store(true, Ordering::SeqCst);
            }
        }
        Ok(())
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down || x.maintenance {
            return Err("Saving queue before exit".into());
        }
        x.pause_requests.remove(id);
        if let Some(flag) = x.active.get(id) {
            flag.store(true, Ordering::SeqCst);
            return Ok(());
        }
        let mut next = x.state.clone();
        let j = next
            .jobs
            .iter_mut()
            .find(|j| j.id == id)
            .ok_or("Unknown job")?;
        if !matches!(j.status, Status::Queued | Status::Paused) {
            return Err("Job is not queued.".into());
        }
        j.status = Status::Cancelled;
        j.message = "Cancelled".into();
        persist(&self.file, &next)?;
        x.state = next;
        Ok(())
    }
    pub fn hold(&self, id: &str, paused: bool) -> Result<()> {
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down || x.maintenance {
            return Err("Saving queue before exit".into());
        }
        let mut next = x.state.clone();
        let j = next
            .jobs
            .iter_mut()
            .find(|j| j.id == id)
            .ok_or("Unknown job")?;
        j.resume_on_startup = false;
        if paused && j.status == Status::Running {
            if !resumable(j) || !j.message.starts_with("Downloading") || !safe_partial(j) {
                return Err("Pause is available only during a supported single transfer with a reusable partial file.".into());
            }
            let flag = x.active.get(id).cloned().ok_or("Download is finishing.")?;
            j.message = "Pausing; preserving partial data".into();
            persist(&self.file, &next)?;
            x.state = next;
            x.pause_requests.insert(id.into());
            flag.store(true, Ordering::SeqCst);
            return Ok(());
        }
        match (&j.status, paused) {
            (Status::Queued, true) => {
                j.status = Status::Paused;
                j.message = "Waiting (paused)".into();
            }
            (Status::Paused, false) => {
                j.status = Status::Queued;
                j.message = "Queued".into();
            }
            _ => return Err("Only waiting jobs can be paused or resumed.".into()),
        }
        persist(&self.file, &next)?;
        x.state = next;
        drop(x);
        if !paused {
            self.kick();
        }
        Ok(())
    }
    pub fn retry(&self, id: &str) -> Result<String> {
        let j = self
            .snapshot()
            .jobs
            .into_iter()
            .find(|j| j.id == id)
            .ok_or("Unknown job")?;
        if !matches!(j.status, Status::Failed | Status::Cancelled) {
            return Err("Only failed or cancelled jobs can be retried.".into());
        }
        if j.files.iter().any(|path| path.is_file()) {
            return Err("Some outputs were already saved. Open their folder before starting a new download.".into());
        }
        self.enqueue(Request {
            url: j.url,
            preset: j.preset,
            quality: j.quality,
            playlist: false,
            organize: j.organize,
            options: j.options,
            destination: j.destination,
        })
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        let mut x = self.inner.lock().unwrap();
        let mut next = x.state.clone();
        if let Some(job) = next
            .jobs
            .iter()
            .find(|j| j.id == id && matches!(j.status, Status::Cancelled | Status::Failed))
        {
            cleanup_partial(job)?;
        }
        next.jobs.retain(|j| {
            j.id != id || matches!(j.status, Status::Running | Status::Queued | Status::Paused)
        });
        persist(&self.file, &next)?;
        x.state = next;
        Ok(())
    }
    fn kick(&self) {
        let mut x = self.inner.lock().unwrap();
        if x.shutting_down
            || x.active.len() >= usize::from(x.state.settings.prefs.concurrent_downloads)
        {
            return;
        }
        let Some(i) = x.state.jobs.iter().position(|j| j.status == Status::Queued) else {
            return;
        };
        x.state.jobs[i].status = Status::Running;
        x.state.jobs[i].message = "Starting".into();
        let j = x.state.jobs[i].clone();
        let settings = x.state.settings.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        x.active.insert(j.id.clone(), cancel.clone());
        if let Err(e) = persist(&self.file, &x.state) {
            x.state.jobs[i].status = Status::Failed;
            x.state.jobs[i].message = format!("Cannot save state: {e}");
            x.active.remove(&j.id);
            return;
        }
        drop(x);
        let engine = self.clone();
        thread::spawn(move || {
            let mut recorded = Vec::new();
            let result = download_recording(
                &j,
                &settings,
                &cancel,
                |p, m| {
                    let mut x = engine.inner.lock().unwrap();
                    if x.pause_requests.contains(&j.id) {
                        return;
                    }
                    if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                        job.progress = p;
                        job.message = m;
                    }
                },
                |path| {
                    recorded.push(path.clone());
                    let mut x = engine.inner.lock().unwrap();
                    if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                        job.files.push(path);
                    }
                    persist(&engine.file, &x.state)
                },
            );
            let mut x = engine.inner.lock().unwrap();
            let pause_requested = x.pause_requests.remove(&j.id);
            if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                match result {
                    Ok(files) => {
                        job.files = files;
                        job.status = Status::Completed;
                        job.resume_on_startup = false;
                        job.progress = 1.;
                        job.message = "Download completed".into();
                    }
                    Err(e) => {
                        job.files = recorded;
                        if pause_requested && job.files.is_empty() && safe_partial(job) {
                            job.status = Status::Paused;
                            job.message="Paused. Partial data retained; Resume continues when supported by the source.".into();
                        } else {
                            job.resume_on_startup = false;
                            job.status = if pause_requested {
                                Status::Failed
                            } else if cancel.load(Ordering::SeqCst) {
                                Status::Cancelled
                            } else {
                                Status::Failed
                            };
                            job.message = if pause_requested {
                                format!("Could not pause safely. {e}")
                            } else {
                                e
                            };
                        }
                    }
                }
            }
            if let Err(e) = persist(&engine.file, &x.state) {
                if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                    job.message = format!("Result not saved: {e}");
                }
            }
            if x.state.settings.prefs.notifications {
                let completed = x
                    .state
                    .jobs
                    .iter()
                    .find(|job| job.id == j.id)
                    .and_then(|job| match job.status {
                        Status::Completed => Some(true),
                        Status::Failed => Some(false),
                        _ => None,
                    });
                if let Some(success) = completed {
                    if x.notices.len() >= 64 {
                        x.notices.pop_front();
                    }
                    x.notices.push_back(success);
                }
            }
            x.active.remove(&j.id);
            drop(x);
            engine.kick();
        });
        self.kick();
    }
}
fn verified_ytdlp(path: &Path) -> bool {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let Ok(m) = fs::symlink_metadata(path) else {
        return false;
    };
    if !m.is_file() || !ordinary(&m) || m.len() == 0 || m.len() > 50_000_000 {
        return false;
    }
    let Ok(mut f) = fs::File::open(path) else {
        return false;
    };
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        match f.read(&mut bytes) {
            Ok(0) => break,
            Ok(n) => hash.update(&bytes[..n]),
            Err(_) => return false,
        }
    }
    hash.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        == "622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c"
}
pub fn tool_version(path: &Path, flag: &str) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    if !path.is_absolute() || !path.is_file() || !matches!(flag, "--version" | "-version") {
        return None;
    }
    let mut output = tempfile::tempfile().ok()?;
    let mut c = command(path);
    c.arg(flag)
        .stdout(Stdio::from(output.try_clone().ok()?))
        .stderr(Stdio::null());
    let mut child = c.spawn().ok()?;
    let _tree = ProcessTree::attach(&mut child).ok()?;
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(8) || output.metadata().ok()?.len() > 65536 {
            terminate(&mut child);
            return None;
        }
        match child.try_wait() {
            Ok(Some(s)) if s.success() => break,
            Ok(Some(_)) => return None,
            Ok(None) => {}
            Err(_) => {
                terminate(&mut child);
                return None;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    output.seek(SeekFrom::Start(0)).ok()?;
    let mut bytes = vec![];
    output.take(1024).read_to_end(&mut bytes).ok()?;
    let line = String::from_utf8_lossy(&bytes);
    let text = line
        .lines()
        .next()?
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect::<String>();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
fn validate_destination(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err("Choose an absolute destination folder.".into());
    }
    for ancestor in path.ancestors() {
        match fs::metadata(ancestor) {
            Ok(meta) if meta.is_dir() => return Ok(()),
            Ok(_) => {
                return Err(format!(
                    "Destination is not a folder: {}",
                    ancestor.display()
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(err(e)),
        }
    }
    Ok(())
}
/// Structural check only; does not execute an arbitrary selected file.
pub fn executable_file(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = File::open(path) else {
            return false;
        };
        let mut dos = [0u8; 64];
        if f.read_exact(&mut dos).is_err() || &dos[..2] != b"MZ" {
            return false;
        }
        let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
        let mut pe = [0u8; 24];
        if offset < 64 || f.seek(SeekFrom::Start(offset)).is_err() || f.read_exact(&mut pe).is_err()
        {
            return false;
        }
        let machine = u16::from_le_bytes([pe[4], pe[5]]);
        let flags = u16::from_le_bytes([pe[22], pe[23]]);
        &pe[..4] == b"PE\0\0"
            && matches!(machine, 0x14c | 0x8664)
            && flags & 2 != 0
            && flags & 0x2000 == 0
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
}
fn command(tool: &Path) -> Command {
    let mut c = Command::new(tool);
    c.env("PYTHONUTF8", "1").env("PYTHONIOENCODING", "utf-8");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c
}
/// A Windows Job Object owns descendants and kills them when the last handle closes.
/// Assignment is immediately after spawn; a process already exited needs no owner.
#[cfg(windows)]
struct ProcessTree(windows_sys::Win32::Foundation::HANDLE);
#[cfg(not(windows))]
struct ProcessTree;
impl ProcessTree {
    fn attach(child: &mut Child) -> Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    terminate(child);
                    return Err("Could not create process supervisor.".into());
                }
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    std::mem::size_of_val(&limits) as u32,
                ) == 0
                    || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0
                {
                    CloseHandle(job);
                    if child.try_wait().ok().flatten().is_some() {
                        return Ok(Self(std::ptr::null_mut()));
                    }
                    terminate(child);
                    return Err("Could not supervise media engine safely.".into());
                }
                Ok(Self(job))
            }
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Ok(Self)
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
}
fn terminate(child: &mut Child) {
    #[cfg(windows)]
    {
        let _ = command(Path::new("taskkill.exe"))
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .output();
    }
    let _ = child.kill();
    let _ = child.wait();
}
fn bounded_output(c: &mut Command, cancel: &AtomicBool, timeout: Duration) -> Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut output = tempfile::tempfile().map_err(err)?;
    c.stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone().map_err(err)?))
        .stderr(Stdio::null());
    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelled".into());
    }
    let mut child = c.spawn().map_err(err)?;
    let owner = ProcessTree::attach(&mut child)?;
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst)
            || start.elapsed() >= timeout
            || output.metadata().map(|m| m.len()).unwrap_or(u64::MAX) > 4_194_304
        {
            terminate(&mut child);
            return Err("Media inspection cancelled, timed out or exceeded output limit.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return Err("Media inspection failed.".into()),
            Ok(None) => {}
            Err(e) => {
                terminate(&mut child);
                return Err(err(e));
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    drop(owner);
    output.seek(SeekFrom::Start(0)).map_err(err)?;
    let mut bytes = Vec::new();
    output
        .take(4_194_305)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > 4_194_304 {
        return Err("Media inspection exceeded output limit.".into());
    }
    Ok(bytes)
}
fn prepare_publication(
    src: &Path,
    dest: &Path,
    cancel: &AtomicBool,
) -> Result<tempfile::NamedTempFile> {
    use std::io::Read;
    let mut tmp = tempfile::NamedTempFile::new_in(dest).map_err(err)?;
    let mut input = File::open(src).map_err(err)?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        let n = input.read(&mut buf).map_err(err)?;
        if n == 0 {
            break;
        }
        tmp.write_all(&buf[..n]).map_err(err)?;
    }
    tmp.flush().map_err(err)?;
    tmp.as_file().sync_all().map_err(err)?;
    Ok(tmp)
}
fn publish_group(
    stem: &str,
    sources: &[(PathBuf, String)],
    dest: &Path,
    cancel: &AtomicBool,
    mut record: impl FnMut(PathBuf) -> Result<()>,
) -> Result<()> {
    let mut suffixes = std::collections::HashSet::new();
    for (_, suffix) in sources {
        if !suffix.starts_with('.')
            || suffix.contains(['/', '\\'])
            || !suffixes.insert(suffix.to_ascii_lowercase())
        {
            return Err("Invalid or duplicate media sidecar suffix.".into());
        }
    }
    let prepared = sources
        .iter()
        .map(|(src, _)| prepare_publication(src, dest, cancel))
        .collect::<Result<Vec<_>>>()?;
    // Serialize only name allocation and publication, not large file copies.
    static PUBLISH_LOCK: Mutex<()> = Mutex::new(());
    let _lock = PUBLISH_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let targets = (0..10000)
        .find_map(|i| {
            let base = if i == 0 {
                stem.to_owned()
            } else {
                format!("{stem} ({i})")
            };
            let paths: Vec<_> = sources
                .iter()
                .map(|(_, suffix)| dest.join(format!("{base}{suffix}")))
                .collect();
            paths.iter().all(|p| !p.exists()).then_some(paths)
        })
        .ok_or("Too many conflicting filenames.")?;
    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelled".into());
    }
    // A late external collision cannot overwrite user data. Previously committed
    // paths are recorded immediately and remaining temporaries clean themselves up.
    for (tmp, target) in prepared.into_iter().zip(targets) {
        tmp.persist_noclobber(&target).map_err(err)?;
        record(target)?;
    }
    Ok(())
}
#[cfg(test)]
fn publish(src: &Path, dest: &Path) -> Result<PathBuf> {
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid filename")?;
    let suffix = format!(
        ".{}",
        src.extension().and_then(|s| s.to_str()).unwrap_or("media")
    );
    let mut files = vec![];
    publish_group(
        stem,
        &[(src.to_owned(), suffix)],
        dest,
        &AtomicBool::new(false),
        |p| {
            files.push(p);
            Ok(())
        },
    )?;
    files.pop().ok_or("No media published".into())
}
fn ensure_editor(
    path: &Path,
    ffmpeg: &Path,
    quality: &str,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    let height_cap = quality.parse::<u64>().ok();
    let probe = ffmpeg.with_file_name(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    if !probe.is_file() {
        return Err("Editor MP4 requires ffprobe beside ffmpeg.".into());
    }
    let compatible = |file: &Path| -> Result<bool> {
        let mut cmd = command(&probe);
        cmd.args(["-v", "error", "-show_streams", "-of", "json"])
            .arg(file);
        let result = bounded_output(&mut cmd, cancel, Duration::from_secs(30))?;
        let info: serde_json::Value = serde_json::from_slice(&result).map_err(err)?;
        let streams = info["streams"].as_array().ok_or("No media streams")?;
        Ok(file.extension().and_then(|v| v.to_str()) == Some("mp4")
            && streams.iter().any(|s| {
                s["codec_type"] == "video"
                    && s["codec_name"] == "h264"
                    && s["pix_fmt"] == "yuv420p"
                    && height_cap
                        .is_none_or(|cap| s["height"].as_u64().is_some_and(|h| h > 0 && h <= cap))
            })
            && streams
                .iter()
                .filter(|s| s["codec_type"] == "audio")
                .all(|s| s["codec_name"] == "aac"))
    };
    if compatible(path)? {
        return Ok(path.to_owned());
    }
    let out = path.with_file_name(format!(
        "{} [editor].mp4",
        path.file_stem().and_then(|v| v.to_str()).unwrap_or("media")
    ));
    let mut cmd = command(ffmpeg);
    cmd.args(["-nostdin", "-v", "error", "-n", "-i"]).arg(path);
    if let Some(cap) = height_cap {
        cmd.args(["-vf", &format!("scale=-2:trunc(min(ih\\,{cap})/2)*2")]);
    }
    let mut child = cmd
        .args([
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
        ])
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(err)?;
    let _owner = ProcessTree::attach(&mut child)?;
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(4 * 3600) {
            terminate(&mut child);
            return Err("Conversion cancelled or timed out.".into());
        }
        match child.try_wait() {
            Ok(Some(s)) if s.success() => break,
            Ok(Some(s)) => return Err(format!("Conversion failed: {s}")),
            Ok(None) => {}
            Err(e) => {
                terminate(&mut child);
                return Err(err(e));
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    if !compatible(&out)? {
        return Err("Editor compatibility verification failed.".into());
    }
    Ok(out)
}
fn classify_error(lower: &str) -> &'static str {
    if lower.contains("removed for violating")
        || lower.contains("video has been removed")
        || lower.contains("video was removed")
    {
        "The video was removed by the source."
    } else if lower.contains("private video") || lower.contains("video is private") {
        "This video is private."
    } else if lower.contains("video unavailable") || lower.contains("video is unavailable") {
        "The video is unavailable at the source."
    } else if lower.contains("403")
        || lower.contains("401")
        || lower.contains("sign in")
        || lower.contains("login")
    {
        "The source refused access. Check whether the media is accessible."
    } else if lower.contains("timed out")
        || lower.contains("resolve")
        || lower.contains("connection")
    {
        "Network error. Check connectivity and retry."
    } else if lower.contains("requested format") || lower.contains("format is not available") {
        "Selected format is unavailable. Try Best or Original."
    } else if lower.contains("ffmpeg") || lower.contains("ffprobe") {
        "Media processing tool failed. Check FFmpeg and FFprobe."
    } else if lower.contains("unsupported url") || lower.contains("unsupported site") {
        "This source may not be supported by the current downloader."
    } else if lower.contains("permission denied")
        || lower.contains("access is denied")
        || lower.contains("no space")
    {
        "Check folder permissions and available storage."
    } else {
        "Media engine reported an error. Check the link and installed tool versions."
    }
}
#[cfg(test)]
fn download(
    j: &Job,
    s: &Settings,
    cancel: &AtomicBool,
    progress: impl FnMut(f64, String),
) -> Result<Vec<PathBuf>> {
    download_recording(j, s, cancel, progress, |_| Ok(()))
}
fn download_recording(
    j: &Job,
    s: &Settings,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64, String),
    mut record: impl FnMut(PathBuf) -> Result<()>,
) -> Result<Vec<PathBuf>> {
    // Only explicitly requested short video clips use the experimental section path.
    // All ordinary downloads, audio, playlists and sidecars retain the original path.
    if fast_section(j) && s.prefs.limit_kib_per_second == 0 {
        progress(0., "Trying fast clip download".into());
        match download_once(j, s, cancel, &mut progress, true, &mut record) {
            Err(e) if e.starts_with("Section unavailable: ") && !cancel.load(Ordering::SeqCst) => {
                progress(0., "Source needs full download; trimming afterward".into());
            }
            result => return result,
        }
    }
    for attempt in 0..=2 {
        match download_once(j, s, cancel, &mut progress, false, &mut record) {
            Err(e)
                if e.starts_with("Retryable network error: ")
                    && attempt < 2
                    && !cancel.load(Ordering::SeqCst) =>
            {
                progress(
                    0.,
                    format!("Connection interrupted. Retrying ({}/2)", attempt + 1),
                );
                for _ in 0..10 {
                    if cancel.load(Ordering::SeqCst) {
                        return Err("Cancelled".into());
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
            result => {
                return result.map_err(|e| {
                    e.strip_prefix("Retryable network error: ")
                        .unwrap_or(&e)
                        .to_owned()
                })
            }
        }
    }
    unreachable!()
}
fn fast_section(j: &Job) -> bool {
    !j.playlist
        && !j.options.archive
        && j.options.subtitle_languages.is_empty()
        && matches!(j.preset.as_str(), "editor" | "original")
        && j.options.clip_end.is_some_and(|end| {
            let start = j.options.clip_start.unwrap_or(0.);
            end.is_finite() && start.is_finite() && end > start && end - start <= 600.
        })
}
#[cfg(test)]
#[test]
fn completed_duplicate_exact_match() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("fixture.mp4");
    fs::write(&file, b"fixture").unwrap();
    let mut r = Request {
        url: "https://example.com/media".into(),
        preset: "editor".into(),
        quality: "1080".into(),
        playlist: false,
        organize: false,
        options: Default::default(),
        destination: dir.path().into(),
    };
    let j = Job {
        id: Uuid::new_v4().simple().to_string(),
        url: r.url.clone(),
        preset: r.preset.clone(),
        quality: r.quality.clone(),
        playlist: false,
        organize: false,
        options: r.options.clone(),
        destination: r.destination.clone(),
        status: Status::Completed,
        progress: 1.,
        message: "Done".into(),
        files: vec![file.clone()],
        resume_on_startup: false,
    };
    assert!(completed_duplicate(&j, &r, &r.url));
    r.options.subtitle_format = "srt".into();
    assert!(completed_duplicate(&j, &r, &r.url));
    r.quality = "720".into();
    assert!(!completed_duplicate(&j, &r, &r.url));
    r.quality = "1080".into();
    r.options.clip_end = Some(5.);
    assert!(!completed_duplicate(&j, &r, &r.url));
    r.options = Default::default();
    fs::remove_file(file).unwrap();
    assert!(!completed_duplicate(&j, &r, &r.url));
}
fn same_output_options(a: &MediaOptions, b: &MediaOptions) -> bool {
    let normalize = |p: &MediaOptions| {
        let mut p = p.clone();
        if p.subtitle_languages.is_empty() {
            p.subtitle_format.clear();
            p.subtitle_auto = false;
        }
        p
    };
    normalize(a) == normalize(b)
}
fn youtube_video_identity(raw: &str) -> Option<String> {
    let u = url::Url::parse(raw).ok()?;
    if u.port().is_some() || u.fragment().is_some() || !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    let pairs: Vec<_> = u.query_pairs().collect();
    let host = u.host_str()?;
    let id = if host == "youtu.be" {
        if pairs
            .iter()
            .any(|(k, _)| !matches!(k.as_ref(), "si" | "feature"))
        {
            return None;
        }
        u.path().strip_prefix('/')?.to_owned()
    } else if matches!(host, "youtube.com" | "www.youtube.com" | "m.youtube.com")
        && u.path() == "/watch"
    {
        if pairs
            .iter()
            .any(|(k, _)| !matches!(k.as_ref(), "v" | "si" | "feature"))
        {
            return None;
        }
        let ids = pairs.iter().filter(|(k, _)| k == "v").collect::<Vec<_>>();
        if ids.len() != 1 {
            return None;
        }
        ids[0].1.to_string()
    } else {
        return None;
    };
    (id.len() == 11
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'))
    .then_some(id)
}
fn same_media_url(a: &str, b: &str, playlist: bool) -> bool {
    a == b
        || (!playlist
            && youtube_video_identity(a)
                .is_some_and(|id| youtube_video_identity(b).as_ref() == Some(&id)))
}
fn completed_duplicate(j: &Job, r: &Request, url: &str) -> bool {
    j.status == Status::Completed
        && !j.playlist
        && !r.playlist
        && same_media_url(&j.url, url, false)
        && j.preset == r.preset
        && (j.quality == r.quality || matches!(r.preset.as_str(), "mp3" | "m4a" | "wav"))
        && j.organize == r.organize
        && same_output_options(&j.options, &r.options)
        && j.destination == r.destination
        && !j.files.is_empty()
        && j.files.iter().all(|f| f.is_file())
}
fn resumable(j: &Job) -> bool {
    matches!(
        j.preset.as_str(),
        "original" | "editor" | "mp3" | "m4a" | "wav"
    ) && !j.playlist
        && !j.options.clipping()
        && !j.options.archive
        && j.options.subtitle_languages.is_empty()
}
fn resume_path(j: &Job) -> Result<PathBuf> {
    if j.id.len() != 32 || !j.id.bytes().all(|v| v.is_ascii_hexdigit()) {
        return Err("Invalid resume job identifier.".into());
    }
    Ok(j.destination.join(format!(".quickclip-resume-{}", j.id)))
}
fn ordinary(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
fn inspect_stage(j: &Job) -> Result<bool> {
    let root = resume_path(j)?;
    let meta = fs::symlink_metadata(&root).map_err(err)?;
    if !meta.is_dir()
        || !ordinary(&meta)
        || root.canonicalize().map_err(err)?.parent()
            != Some(j.destination.canonicalize().map_err(err)?.as_path())
    {
        return Err("Unsafe resume directory.".into());
    }
    let owner = root.join(".owner");
    let meta = fs::symlink_metadata(&owner).map_err(err)?;
    if !meta.is_file()
        || !ordinary(&meta)
        || meta.len() != 32
        || fs::read_to_string(owner).map_err(err)? != j.id
    {
        return Err("Resume directory ownership mismatch.".into());
    }
    let mut partial = false;
    for entry in fs::read_dir(root).map_err(err)? {
        let entry = entry.map_err(err)?;
        let name = entry.file_name();
        if name == ".owner" {
            continue;
        }
        let name = name.to_string_lossy();
        let meta = fs::symlink_metadata(entry.path()).map_err(err)?;
        if !meta.is_file()
            || !ordinary(&meta)
            || !(name.ends_with(".part") || name.ends_with(".ytdl") || name.contains(".part-Frag"))
        {
            return Err("Resume directory contains completed or unexpected outputs; preserved for manual recovery.".into());
        }
        if name.ends_with(".part") && meta.len() > 0 {
            partial = true;
        }
    }
    Ok(partial)
}
fn cleanup_partial(j: &Job) -> Result<()> {
    if !resumable(j) {
        return Ok(());
    }
    let root = resume_path(j)?;
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(err(e)),
        Ok(_) => {}
    }
    // Preserve unrecognized or completed media and never recurse.
    inspect_stage(j).map_err(|e| format!("{e} Recovery folder: {}", root.display()))?;
    for entry in fs::read_dir(&root).map_err(err)? {
        let entry = entry.map_err(err)?;
        let name = entry.file_name();
        if name == ".owner" {
            continue;
        }
        let name = name.to_string_lossy();
        let meta = fs::symlink_metadata(entry.path()).map_err(err)?;
        if !meta.is_file()
            || !ordinary(&meta)
            || !(name.ends_with(".part") || name.ends_with(".ytdl") || name.contains(".part-Frag"))
        {
            return Err("Unexpected recovery file; preserved.".into());
        }
        fs::remove_file(entry.path()).map_err(err)?;
    }
    fs::remove_file(root.join(".owner")).map_err(err)?;
    fs::remove_dir(root).map_err(err)?;
    Ok(())
}
fn safe_partial(j: &Job) -> bool {
    resumable(j) && inspect_stage(j).unwrap_or(false)
}
enum Stage {
    Temp(tempfile::TempDir),
    Resume(PathBuf),
}
impl Stage {
    fn open(j: &Job) -> Result<Self> {
        if !resumable(j) {
            return Ok(Self::Temp(
                tempfile::Builder::new()
                    .prefix(".quickclip-v3-")
                    .tempdir_in(&j.destination)
                    .map_err(err)?,
            ));
        }
        let root = resume_path(j)?;
        match fs::create_dir(&root) {
            Ok(()) => {
                let mut owner = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root.join(".owner"))
                    .map_err(err)?;
                owner.write_all(j.id.as_bytes()).map_err(err)?;
                owner.sync_all().map_err(err)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                inspect_stage(j)?;
            }
            Err(e) => return Err(err(e)),
        }
        Ok(Self::Resume(root))
    }
    fn path(&self) -> &Path {
        match self {
            Self::Temp(t) => t.path(),
            Self::Resume(p) => p,
        }
    }
    fn keep(self) -> PathBuf {
        match self {
            Self::Temp(t) => t.keep(),
            Self::Resume(p) => p,
        }
    }
    fn empty(&self) -> bool {
        fs::read_dir(self.path()).ok().is_some_and(|entries| {
            entries
                .map(|entry| entry.is_ok_and(|e| e.file_name() == ".owner"))
                .all(|ok| ok)
        })
    }
    fn finish(self) {
        if let Self::Resume(root) = self {
            // Do not recurse or touch reparse points. Preserve unexpected folders.
            let owner = root.join(".owner");
            let expected = root
                .file_name()
                .and_then(|v| v.to_str())
                .and_then(|v| v.strip_prefix(".quickclip-resume-"));
            if !fs::symlink_metadata(&owner)
                .ok()
                .is_some_and(|m| m.is_file() && ordinary(&m) && m.len() == 32)
                || fs::read_to_string(&owner).ok().as_deref() != expected
            {
                return;
            }
            if fs::symlink_metadata(&root)
                .ok()
                .is_some_and(|m| m.is_dir() && ordinary(&m))
            {
                if let Ok(entries) = fs::read_dir(&root) {
                    for entry in entries.flatten() {
                        if fs::symlink_metadata(entry.path())
                            .ok()
                            .is_some_and(|m| m.is_file() && ordinary(&m))
                        {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
                let _ = fs::remove_dir(&root);
            }
        }
    }
}
fn preview_stages() -> &'static Mutex<std::collections::HashMap<String, PathBuf>> {
    static STAGES: std::sync::OnceLock<Mutex<std::collections::HashMap<String, PathBuf>>> =
        std::sync::OnceLock::new();
    STAGES.get_or_init(|| Mutex::new(Default::default()))
}
struct PreviewStage(String);
impl PreviewStage {
    fn new(id: String, path: PathBuf) -> Self {
        preview_stages().lock().unwrap().insert(id.clone(), path);
        Self(id)
    }
}
impl Drop for PreviewStage {
    fn drop(&mut self) {
        preview_stages().lock().unwrap().remove(&self.0);
    }
}
pub fn live_frame(id: &str, ffmpeg: &Path, seconds: f64) -> Option<String> {
    // No remote streams, no additional download. One bounded, low-resolution decode.
    static DECODE: Mutex<()> = Mutex::new(());
    let _guard = DECODE.try_lock().ok()?;
    if !seconds.is_finite() || seconds < 0. || seconds > 86400. * 30. {
        return None;
    }
    let root = preview_stages().lock().ok()?.get(id)?.clone();
    if !ordinary(&fs::symlink_metadata(&root).ok()?) {
        return None;
    }
    let canonical = root.canonicalize().ok()?;
    let mut candidates = vec![];
    for e in fs::read_dir(&root).ok()?.flatten().take(64) {
        let path = e.path();
        let m = fs::symlink_metadata(&path).ok()?;
        if !m.is_file()
            || !ordinary(&m)
            || m.len() < 32768
            || path.canonicalize().ok()?.parent() != Some(canonical.as_path())
        {
            continue;
        }
        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        let name = name.strip_suffix(".part").unwrap_or(&name);
        let format = if name.ends_with(".mp4") || name.ends_with(".m4v") {
            "mov"
        } else if name.ends_with(".webm") || name.ends_with(".mkv") {
            "matroska"
        } else {
            continue;
        };
        candidates.push((m.len(), path, format));
    }
    candidates.sort_by_key(|x| std::cmp::Reverse(x.0));
    let (_, path, format) = candidates.first()?;
    let mut output = tempfile::tempfile().ok()?;
    let mut c = command(ffmpeg);
    c.args([
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-protocol_whitelist",
        "file,pipe",
        "-threads",
        "1",
        "-ss",
        &format!("{seconds:.3}"),
        "-f",
        format,
        "-i",
    ])
    .arg(path)
    .args([
        "-an",
        "-sn",
        "-dn",
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-vf",
        "scale=160:-2",
        "-threads",
        "1",
        "-f",
        "image2pipe",
        "-vcodec",
        "mjpeg",
        "pipe:1",
    ])
    .stdout(Stdio::from(output.try_clone().ok()?))
    .stderr(Stdio::null());
    let mut child = c.spawn().ok()?;
    let _owner = ProcessTree::attach(&mut child).ok()?;
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_millis(700) || output.metadata().ok()?.len() > 160000 {
            terminate(&mut child);
            return None;
        }
        match child.try_wait() {
            Ok(Some(s)) if s.success() => break,
            Ok(Some(_)) => return None,
            Ok(None) => {}
            Err(_) => {
                terminate(&mut child);
                return None;
            }
        }
        thread::sleep(Duration::from_millis(40));
    }
    use std::io::{Read, Seek, SeekFrom};
    output.seek(SeekFrom::Start(0)).ok()?;
    let mut data = vec![];
    output.take(160001).read_to_end(&mut data).ok()?;
    if data.len() > 160000 || !data.starts_with(&[0xff, 0xd8]) || !data.ends_with(&[0xff, 0xd9]) {
        return None;
    }
    Some(format!("data:image/jpeg;base64,{}", encode_base64(&data)))
}
// yt-dlp --skip-download produces subtitle sidecars without a media file.
fn subtitle_only_groups(
    root: &Path,
    format: &str,
) -> Result<std::collections::BTreeMap<String, Vec<(PathBuf, String)>>> {
    if !matches!(format, "srt" | "vtt") {
        return Err("Choose SRT or VTT.".into());
    }
    let root = root.canonicalize().map_err(err)?;
    let mut groups = std::collections::BTreeMap::<String, Vec<(PathBuf, String)>>::new();
    for entry in fs::read_dir(&root).map_err(err)? {
        let entry = entry.map_err(err)?;
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some(format) {
            continue;
        }
        let meta = fs::symlink_metadata(&path).map_err(err)?;
        if !ordinary(&meta)
            || !meta.is_file()
            || path.canonicalize().map_err(err)?.parent() != Some(root.as_path())
        {
            return Err("Unsafe subtitle output.".into());
        }
        if meta.len() == 0 {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or("Invalid subtitle filename")?;
        let base = name
            .strip_suffix(&format!(".{format}"))
            .ok_or("Invalid subtitle extension")?;
        let (stem, lang) = base.rsplit_once('.').ok_or("Missing subtitle language")?;
        if stem.is_empty() || lang.is_empty() {
            return Err("Invalid subtitle filename".into());
        }
        groups
            .entry(stem.to_owned())
            .or_default()
            .push((path.clone(), format!(".{lang}.{format}")));
    }
    Ok(groups)
}
fn download_once(
    j: &Job,
    s: &Settings,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64, String),
    section: bool,
    mut record: impl FnMut(PathBuf) -> Result<()>,
) -> Result<Vec<PathBuf>> {
    if !executable_file(&s.yt_dlp) || !executable_file(&s.ffmpeg) {
        return Err("Set existing yt-dlp and FFmpeg executable paths in Settings.".into());
    }
    fs::create_dir_all(&j.destination).map_err(err)?;
    let staging = Stage::open(j)?;
    let _preview = PreviewStage::new(j.id.clone(), staging.path().to_owned());
    let mut c = command(&s.yt_dlp);
    c.args([
        "--ignore-config",
        "--no-plugin-dirs",
        "--no-cache-dir",
        "--newline",
        "--no-warnings",
        "--progress",
        "--progress-template",
        "download:QC:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s",
        "--progress-template",
        "postprocess:QC_POST:%(progress.status)s",
        "--windows-filenames",
        "--no-overwrites",
        "--continue",
        "--socket-timeout",
        "20",
        "--retries",
        "2",
        "--fragment-retries",
        "2",
        "--concurrent-fragments",
        &if s.prefs.limit_kib_per_second > 0 {
            "1".to_string()
        } else {
            s.prefs.fragment_concurrency.to_string()
        },
    ]);
    if s.prefs.limit_kib_per_second > 0 {
        c.args([
            "--limit-rate",
            &format!("{}K", s.prefs.limit_kib_per_second),
        ]);
    }
    if j.playlist {
        c.arg("--yes-playlist");
    } else {
        c.args(["--no-playlist", "--playlist-items", "1"]);
    }
    c.args(["--js-runtimes", &js_runtime(&s.yt_dlp)])
        .arg("--ffmpeg-location")
        .arg(&s.ffmpeg)
        .arg("-f")
        .arg(selector(&j.preset, &j.quality)?)
        .arg("-o")
        .arg(staging.path().join("%(title).90B [%(id)s].%(ext)s"))
        .args([
            "--no-simulate",
            "--print-to-file",
            "after_move:%(.{filepath,title,artist,album_artist,channel,uploader,series,tv_show,movie,movie_title,vcodec,track,genre,game,game_title})j",
        ])
        .arg(staging.path().join("outputs.jsonl"));
    if j.preset == "subtitles" {
        c.args(["--skip-download", "--ignore-no-formats-error"]);
    }
    if section {
        let start = j.options.clip_start.unwrap_or(0.);
        let end = j.options.clip_end.ok_or("Missing clip end")?;
        c.args([
            "--download-sections",
            &format!("*{start:.6}-{end:.6}"),
            "--force-keyframes-at-cuts",
        ]);
    }
    if !j.options.subtitle_languages.is_empty() {
        c.args([
            "--write-subs",
            "--sub-langs",
            &j.options.subtitle_languages.join(","),
            "--sub-format",
            "best",
            "--convert-subs",
            &j.options.subtitle_format,
        ]);
        if j.options.subtitle_auto {
            c.arg("--write-auto-subs");
        }
    }
    if j.options.archive {
        c.args([
            "--write-info-json",
            "--write-description",
            "--clean-info-json",
        ]);
    }
    match j.preset.as_str() {
        "editor" => {
            c.args(["--merge-output-format", "mp4", "--recode-video", "mp4"]);
        }
        "mp3" | "wav" | "m4a" => {
            c.args(["-x", "--audio-format", &j.preset]);
        }
        _ => {}
    }
    #[cfg(test)]
    if j.url.starts_with("http://127.0.0.1:") {
        c.args(["--proxy", ""]);
    }
    c.arg("--")
        .arg(&j.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().map_err(err)?;
    let owner = ProcessTree::attach(&mut child)?;
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let stderr = child.stderr.take().ok_or("Missing stderr")?;
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = thread::spawn(move || {
        for bytes in BufReader::new(stdout)
            .split(b'\n')
            .map_while(std::result::Result::ok)
        {
            let line = String::from_utf8_lossy(&bytes).into_owned();
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let errors = thread::spawn(move || {
        let mut hint = None;
        for bytes in BufReader::new(stderr)
            .split(b'\n')
            .map_while(std::result::Result::ok)
        {
            let v = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            #[cfg(test)]
            eprintln!("fixture stderr: {v}");
            if v.contains("error") {
                hint = Some(classify_error(&v));
            }
        }
        hint
    });
    let started = Instant::now();
    let exit = loop {
        if cancel.load(Ordering::SeqCst) {
            terminate(&mut child);
            break Err("Cancelled".into());
        }
        if started.elapsed()
            > Duration::from_secs(if section {
                180 + (j.options.clip_end.unwrap_or(0.) - j.options.clip_start.unwrap_or(0.)).ceil()
                    as u64
            } else {
                4 * 3600
            })
        {
            terminate(&mut child);
            break Err("Media operation timed out.".into());
        }
        while let Ok(line) = rx.try_recv() {
            if let Some(value) = line.strip_prefix("QC:") {
                let mut parts = value.split('|');
                if let Ok(p) = parts
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches('%')
                    .parse::<f64>()
                {
                    progress(
                        (p / 100.).clamp(0., 0.98),
                        format!(
                            "Downloading current stream {} | {} | ETA {}",
                            format!("{p:.1}%"),
                            parts.next().unwrap_or("unknown").trim(),
                            parts.next().unwrap_or("unknown").trim()
                        ),
                    );
                }
            } else if line.starts_with("QC_POST:") {
                progress(0.98, "Processing media".into());
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "Media engine exited with {status}. Check the link and tool versions."
                    ))
                }
            }
            Ok(None) => {}
            Err(e) => {
                terminate(&mut child);
                break Err(err(e));
            }
        }
        thread::sleep(Duration::from_millis(120));
    };
    drop(owner); // Close inherited pipes from any lingering descendant before joining readers.
    let _ = reader.join();
    let hint = errors.join().ok().flatten();
    if let Err(mut e) = exit {
        if section && !cancel.load(Ordering::SeqCst) {
            return Err(format!("Section unavailable: {e}"));
        }
        // Retry only before ANY staged file exists. Never redownload or duplicate published outputs.
        if !cancel.load(Ordering::SeqCst)
            && hint == Some("Network error. Check connectivity and retry.")
            && staging.empty()
        {
            return Err(format!(
                "Retryable network error: {e} Network error. Check connectivity and retry."
            ));
        }
        if !cancel.load(Ordering::SeqCst) {
            if let Some(h) = hint {
                e = h.to_string();
            }
        }
        if staging.empty() {
            staging.finish();
            return Err(e);
        }
        let retained = staging.keep();
        return Err(format!(
            "{e} Partial files retained at {}",
            retained.display()
        ));
    }
    let mut publication_started = false;
    let result = (|| -> Result<Vec<PathBuf>> {
        let mut files = vec![];
        if j.preset == "subtitles" {
            let target = if j.organize {
                j.destination.join("Subtitles")
            } else {
                j.destination.clone()
            };
            let groups = subtitle_only_groups(staging.path(), &j.options.subtitle_format)?;
            if groups.is_empty() {
                return Err("No matching subtitles found. Try another language or enable automatic subtitles.".into());
            }
            fs::create_dir_all(&target).map_err(err)?;
            progress(0.99, "Saving subtitles".into());
            for (stem, sources) in groups {
                publication_started = true;
                publish_group(&stem, &sources, &target, cancel, |path| {
                    files.push(path.clone());
                    record(path)
                })?;
            }
            return Ok(files);
        }
        let mut seen = std::collections::HashSet::new();
        let root = staging.path().canonicalize().map_err(err)?;
        let manifest = fs::read_to_string(staging.path().join("outputs.jsonl")).map_err(err)?;
        let mut ready_files = vec![];
        for line in manifest.lines() {
            let metadata: serde_json::Value = serde_json::from_str(line).map_err(err)?;
            let path = PathBuf::from(
                metadata["filepath"]
                    .as_str()
                    .ok_or("Missing media output path")?,
            );
            let path = path.canonicalize().map_err(err)?;
            if !path.starts_with(&root)
                || !path.is_file()
                || path.metadata().map_err(err)?.len() == 0
            {
                return Err("Invalid output from media engine.".into());
            }
            if !seen.insert(path.clone()) {
                continue;
            }
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelled".into());
            }
            let sidecars = media_options::subtitle_files(&path, &j.options)?;
            let archive_files = if j.options.archive {
                [
                    path.with_extension("info.json"),
                    path.with_extension("description"),
                ]
                .into_iter()
                .filter(|p| p.is_file())
                .collect::<Vec<_>>()
            } else {
                vec![]
            };
            let ready = if j.preset == "editor" || section {
                progress(0.98, "Verifying editor compatibility".into());
                ensure_editor(&path, &s.ffmpeg, &j.quality, cancel)?
            } else {
                path.clone()
            };
            let ready = if section {
                let duration = media_options::inspect_duration_cancel(&ready, &s.ffmpeg, cancel)?;
                let expected =
                    j.options.clip_end.unwrap_or(0.) - j.options.clip_start.unwrap_or(0.);
                if (duration - expected).abs() > 0.25_f64.max(expected * 0.02) {
                    return Err("Fast clip duration mismatch".into());
                }
                let stem = ready
                    .file_stem()
                    .and_then(|v| v.to_str())
                    .ok_or("Invalid clip name")?;
                let output = ready.with_file_name(format!(
                    "{stem} [clip {}-{}].mp4",
                    j.options.clip_start.unwrap_or(0.),
                    j.options.clip_end.unwrap_or(0.)
                ));
                fs::rename(&ready, &output).map_err(err)?;
                output
            } else if j.options.clipping() {
                progress(0.98, "Trimming clip".into());
                media_options::clip(&ready, &s.ffmpeg, &j.options, cancel)?
            } else {
                ready
            };
            let destination = if j.organize {
                organizer::configured_destination(&metadata, &j.preset, &j.destination, &s.prefs)
            } else {
                j.destination.clone()
            };
            ready_files.push((ready, destination, path, sidecars, archive_files));
        }
        progress(0.99, "Saving media files".into());
        for (_, destination, _, _, _) in &ready_files {
            fs::create_dir_all(destination).map_err(|e| {
                format!("Cannot create output folder {}: {e}", destination.display())
            })?;
        }
        for (ready, destination, source, sidecars, archive_files) in ready_files {
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelled".into());
            }
            let target = &destination;
            publication_started = true;
            let stem = ready
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid media name")?;
            let source_stem = source
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid source name")?;
            let extension = ready
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("media");
            let mut sources = vec![(ready.clone(), format!(".{extension}"))];
            let tmp = tempfile::tempdir_in(staging.path()).map_err(err)?;
            for sidecar in sidecars.into_iter().chain(archive_files) {
                let name = sidecar
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid sidecar name")?;
                let suffix = name
                    .strip_prefix(source_stem)
                    .ok_or("Unexpected sidecar name")?;
                let dest = tmp.path().join(name);
                if matches!(
                    sidecar.extension().and_then(|s| s.to_str()),
                    Some("srt" | "vtt")
                ) {
                    let text = fs::read_to_string(&sidecar).map_err(err)?;
                    fs::write(
                        &dest,
                        media_options::trim_subtitle(
                            &text,
                            &j.options,
                            sidecar.extension().and_then(|s| s.to_str()) == Some("srt"),
                        ),
                    )
                    .map_err(err)?;
                    sources.push((dest, suffix.to_owned()));
                } else {
                    sources.push((sidecar.clone(), suffix.to_owned()));
                }
            }
            publish_group(stem, &sources, target, cancel, |path| {
                files.push(path.clone());
                record(path)
            })?;
        }
        if files.is_empty() {
            return Err("No media file was produced.".into());
        }
        Ok(files)
    })();
    match result {
        Ok(files) => {
            staging.finish();
            Ok(files)
        }
        Err(e) => {
            if section && !publication_started && !cancel.load(Ordering::SeqCst) {
                return Err(format!("Section unavailable: {e}"));
            }
            let retained = staging.keep();
            Err(format!("{e} Staging retained at {}", retained.display()))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subtitle_only_groups_keep_languages_and_reject_empty() {
        let dir = tempfile::tempdir().unwrap();
        for (name, data) in [
            ("Title.with.dot [id].en.srt", "cue"),
            ("Title.with.dot [id].tr.srt", "cue"),
            ("Other [id].en.srt", "cue"),
            ("Empty [id].en.srt", ""),
            ("Other [id].mp4", "video"),
            ("Other [id].en.vtt", "vtt"),
        ] {
            fs::write(dir.path().join(name), data).unwrap();
        }
        let groups = subtitle_only_groups(dir.path(), "srt").unwrap();
        assert_eq!(groups.len(), 2);
        let langs = &groups["Title.with.dot [id]"];
        assert_eq!(langs.len(), 2);
        assert!(
            langs.iter().any(|(_, suffix)| suffix == ".en.srt")
                && langs.iter().any(|(_, suffix)| suffix == ".tr.srt")
        );
        assert!(subtitle_only_groups(dir.path(), "mp4").is_err());
        assert_eq!(subtitle_only_groups(dir.path(), "vtt").unwrap().len(), 1);
    }

    #[test]
    fn resume_stage_rejects_unsafe_reuse() {
        let d = tempfile::tempdir().unwrap();
        let j = Job {
            id: Uuid::new_v4().simple().to_string(),
            url: "https://example.com/media".into(),
            preset: "original".into(),
            quality: "best".into(),
            playlist: false,
            organize: false,
            options: Default::default(),
            destination: d.path().into(),
            status: Status::Paused,
            progress: 0.,
            message: String::new(),
            files: vec![],
            resume_on_startup: false,
        };
        let stage = Stage::open(&j).unwrap();
        fs::write(stage.path().join("file.mp4.part"), b"partial").unwrap();
        assert!(safe_partial(&j));
        fs::write(stage.path().join("outputs.jsonl"), b"completed manifest").unwrap();
        assert!(!safe_partial(&j));
        assert!(Stage::open(&j).is_err());
        assert_eq!(
            fs::read(stage.path().join("file.mp4.part")).unwrap(),
            b"partial"
        );
        fs::remove_file(stage.path().join("outputs.jsonl")).unwrap();
        fs::write(stage.path().join(".owner"), b"wrong").unwrap();
        assert!(Stage::open(&j).is_err());
        assert_eq!(
            fs::read(stage.path().join("file.mp4.part")).unwrap(),
            b"partial"
        );
    }
    #[cfg(windows)]
    #[test]
    fn process_tree_kills_owned_child() {
        let ps = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = command(&ps)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let owner = ProcessTree::attach(&mut child).unwrap();
        drop(owner);
        let start = Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if start.elapsed() > Duration::from_secs(5) {
                terminate(&mut child);
                panic!("Owned child survived supervisor close");
            }
            thread::sleep(Duration::from_millis(30));
        }
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Requires isolated verified managed binaries"]
    fn managed_setup_reuses_verified_tools() {
        let source = PathBuf::from(std::env::var("QC_TEST_MANAGED_ROOT").unwrap());
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("tools/managed-2026.11.23-9.1-24.22.3");
        fs::create_dir_all(&target).unwrap();
        for file in [
            "yt-dlp.exe",
            "ffmpeg.exe",
            "ffprobe.exe",
            "node.exe",
            "FFmpeg-LICENSE.txt",
            "Node-LICENSE.txt",
        ] {
            fs::copy(source.join(file), target.join(file)).unwrap();
        }
        let settings = Settings {
            destination: d.path().join("media"),
            yt_dlp: PathBuf::new(),
            ffmpeg: PathBuf::new(),
            prefs: Default::default(),
        };
        let engine = Engine::open(d.path().join("state.json"), settings).unwrap();
        let installed = engine.setup_managed_tools().unwrap();
        assert_eq!(installed.yt_dlp, target.join("yt-dlp.exe"));
        assert_eq!(installed.ffmpeg, target.join("ffmpeg.exe"));
        let updated = engine.install_recommended_ytdlp().unwrap();
        assert_eq!(
            updated.yt_dlp,
            d.path().join("tools/yt-dlp-2026.11.23/yt-dlp.exe")
        );
        assert_eq!(updated.ffmpeg, installed.ffmpeg);
        assert!(verified_ytdlp(&updated.yt_dlp));
        assert_eq!(engine.rollback_ytdlp().unwrap().yt_dlp, installed.yt_dlp);

        assert_eq!(engine.snapshot().settings.yt_dlp, installed.yt_dlp);
        assert!(js_runtime(&installed.yt_dlp).starts_with("node:"));
        fs::write(target.join("yt-dlp.exe"), b"tampered fixture").unwrap();
        engine.setup_managed_tools().unwrap();
        assert!(verified_ytdlp(&target.join("yt-dlp.exe")));
        let quarantines = fs::read_dir(d.path().join("tools"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".quarantine-"))
            .collect::<Vec<_>>();
        assert_eq!(quarantines.len(), 1);
        assert_eq!(
            fs::read(quarantines[0].path().join("yt-dlp.exe")).unwrap(),
            b"tampered fixture"
        );
        assert_eq!(engine.snapshot().settings.yt_dlp, installed.yt_dlp);
    }
    #[test]
    fn invalid_links() {
        for u in [
            "file:///x",
            "https://user:pass@host/a",
            "https://[",
            "https://host:99999",
            "https://host/a b",
        ] {
            assert!(validate_url(u).is_err(), "{u}");
        }
        assert!(validate_url("www.example.com/video").is_ok());
    }
    #[test]
    fn choices() {
        assert!(selector("editor", "1080").unwrap().contains("height<=1080"));
        assert!(selector("bogus", "best").is_err());
        assert!(selector("mp3", "best").is_ok());
    }
    #[test]
    fn no_overwrite() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("source.mp4");
        fs::write(&source, b"new").unwrap();
        let dest = d.path().join("out");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("source.mp4"), b"old").unwrap();
        let p = publish(&source, &dest).unwrap();
        assert_eq!(fs::read(p).unwrap(), b"new");
        assert_eq!(fs::read(dest.join("source.mp4")).unwrap(), b"old");
    }
    #[test]
    #[ignore = "Requires local FFmpeg fixture"]
    fn partial_preview_frames() {
        let dir = tempfile::tempdir().unwrap();
        let ffmpeg = PathBuf::from(std::env::var("QC_TEST_FFMPEG").unwrap());
        let source = dir.path().join("fixture.mp4");
        assert!(command(&ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=320x180:r=24:d=8",
                "-c:v",
                "libx264",
                "-g",
                "24",
                "-threads",
                "1",
                "-movflags",
                "+faststart"
            ])
            .arg(&source)
            .status()
            .unwrap()
            .success());
        let bytes = fs::read(&source).unwrap();
        let stage = tempfile::tempdir().unwrap();
        fs::write(
            stage.path().join("fixture.mp4.part"),
            &bytes[..bytes.len() * 3 / 4],
        )
        .unwrap();
        let id = Uuid::new_v4().simple().to_string();
        let _live = PreviewStage::new(id.clone(), stage.path().into());
        let first = live_frame(&id, &ffmpeg, 0.5).expect("first real partial frame");
        let second = live_frame(&id, &ffmpeg, 3.).expect("second real partial frame");
        assert_ne!(first, second);
        println!(
            "Two distinct JPEG frames from incomplete local MP4: {} and {} base64 characters",
            first.len(),
            second.len()
        );
    }
    #[test]
    #[ignore = "Requires installed yt-dlp and FFmpeg; no Internet media needed"]
    fn range_resume() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::atomic::AtomicU64,
        };
        let dir = tempfile::tempdir().unwrap();
        let ytdlp = PathBuf::from(std::env::var("QC_TEST_YTDLP").unwrap());
        let ffmpeg = PathBuf::from(std::env::var("QC_TEST_FFMPEG").unwrap());
        let source = dir.path().join("range.mp4");
        assert!(command(&ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=640x360:r=30:d=15",
                "-c:v",
                "mpeg4",
                "-q:v",
                "1",
                "-threads",
                "2",
                "-movflags",
                "+faststart"
            ])
            .arg(&source)
            .status()
            .unwrap()
            .success());
        let bytes = Arc::new(fs::read(&source).unwrap());
        assert!(bytes.len() > 512 * 1024);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let high = Arc::new(AtomicU64::new(u64::MAX));
        let transferred = Arc::new(AtomicU64::new(0));
        let server_bytes = transferred.clone();
        let server_stop = stop.clone();
        let server_high = high.clone();
        let payload = bytes.clone();
        let server = thread::spawn(move || {
            let mut workers = vec![];
            while !server_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut sock, _)) => {
                        let data = payload.clone();
                        let range_seen = server_high.clone();
                        let stopping = server_stop.clone();
                        let count = server_bytes.clone();
                        workers.push(thread::spawn(move||{
                            let _=sock.set_read_timeout(Some(Duration::from_secs(3)));let _=sock.set_write_timeout(Some(Duration::from_secs(30)));
                            let mut request=vec![];let mut byte=[0];
                            while request.len()<16384&&sock.read_exact(&mut byte).is_ok(){request.push(byte[0]);if request.ends_with(b"\r\n\r\n"){break}}
                            let text=String::from_utf8_lossy(&request).to_ascii_lowercase();
                            let range_header=text.lines().find_map(|line|line.strip_prefix("range: bytes="));
                            let (range,end)=range_header.and_then(|v|v.split_once('-')).map(|(a,b)|(a.parse::<usize>().unwrap_or(0),b.parse::<usize>().unwrap_or(data.len()-1).min(data.len()-1))).unwrap_or((0,data.len()-1));
                            if range>=data.len()||end<range{let _=sock.write_all(format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",data.len()).as_bytes());return}
                            if range>0{range_seen.fetch_min(range as u64,Ordering::SeqCst);}
                            let status=if range_header.is_some(){"206 Partial Content"}else{"200 OK"};
                            let extra=if range_header.is_some(){format!("Content-Range: bytes {}-{}/{}\r\n",range,end,data.len())}else{String::new()};
                            let h=format!("HTTP/1.1 {status}\r\nContent-Type: video/mp4\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",end-range+1);
                            if sock.write_all(h.as_bytes()).is_err(){return}
                            if !text.starts_with("head "){for block in data[range..=end].chunks(16*1024){if stopping.load(Ordering::SeqCst)||sock.write_all(block).is_err(){break;}count.fetch_add(block.len() as u64,Ordering::SeqCst);thread::sleep(Duration::from_millis(15));}}
                            let _=sock.flush();let _=sock.shutdown(std::net::Shutdown::Write);
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
            for worker in workers {
                let _ = worker.join();
            }
        });
        let out = dir.path().join("out");
        let state = dir.path().join("state.json");
        let settings = Settings {
            destination: out.clone(),
            yt_dlp: ytdlp,
            ffmpeg,
            prefs: Preferences::default(),
        };
        let engine = Engine::open(state.clone(), settings.clone()).unwrap();
        let _id = engine
            .enqueue(Request {
                url: format!("http://{addr}/range.mp4"),
                preset: "original".into(),
                quality: "best".into(),
                playlist: false,
                organize: false,
                options: Default::default(),
                destination: out.clone(),
            })
            .unwrap();
        let result = (|| {
            let started = Instant::now();
            loop {
                let j = engine.snapshot().jobs[0].clone();
                if j.status == Status::Running
                    && j.message.starts_with("Downloading")
                    && safe_partial(&j)
                {
                    engine.prepare_safe_exit().unwrap();
                    break;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(25)
                        && !matches!(
                            j.status,
                            Status::Completed | Status::Failed | Status::Cancelled
                        ),
                    "Could not pause: {:?}",
                    j
                );
                thread::sleep(Duration::from_millis(30));
            }
            let started = Instant::now();
            let partial = loop {
                let j = engine.snapshot().jobs[0].clone();
                if j.status == Status::Paused {
                    assert!(safe_partial(&j));
                    break fs::read_dir(resume_path(&j).unwrap())
                        .unwrap()
                        .flatten()
                        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
                        .map(|e| e.metadata().unwrap().len())
                        .sum::<u64>();
                }
                assert!(
                    started.elapsed() < Duration::from_secs(15) && j.status == Status::Running,
                    "Pause failed: {:?}",
                    j
                );
                thread::sleep(Duration::from_millis(40));
            };
            assert!(partial > 0 && partial < bytes.len() as u64);
            assert!(engine.safe_exit_ready());
            assert!(engine.snapshot().jobs[0].resume_on_startup);
            // Reopen persisted paused job, equivalent to an orderly app restart.
            drop(engine);
            high.store(u64::MAX, Ordering::SeqCst);
            let reopened = Engine::open(state, settings.clone()).unwrap();
            assert_eq!(reopened.snapshot().jobs[0].status, Status::Paused);
            assert_eq!(reopened.resume_saved_queue().unwrap(), 1);
            let started = Instant::now();
            let final_job = loop {
                let j = reopened.snapshot().jobs[0].clone();
                if j.status == Status::Completed {
                    break j;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(45)
                        && !matches!(j.status, Status::Failed | Status::Cancelled),
                    "Resume failed: {:?}",
                    j
                );
                thread::sleep(Duration::from_millis(50));
            };
            let requested = high.load(Ordering::SeqCst);
            assert!(
                requested > 0 && requested <= partial,
                "No actual byte reuse: range={requested}, partial={partial}"
            );
            assert_eq!(final_job.files.len(), 1);
            assert_eq!(fs::read(&final_job.files[0]).unwrap(), *bytes);
            assert!(!resume_path(&final_job).unwrap().exists());
            println!(
                "Range resume: retained {partial}, requested {requested}, exact output {} bytes",
                bytes.len()
            );
            // Test the section path separately, with real HTTP Range bytes and validated duration.
            transferred.store(0, Ordering::SeqCst);
            let mut clip = final_job.clone();
            clip.id = Uuid::new_v4().simple().to_string();
            clip.preset = "editor".into();
            clip.options = MediaOptions {
                clip_start: Some(8.),
                clip_end: Some(9.),
                ..Default::default()
            };
            let mut fallback = false;
            let files = download(&clip, &settings, &AtomicBool::new(false), |_, m| {
                if m.starts_with("Source needs full") {
                    fallback = true;
                }
            })
            .unwrap();
            let duration = media_options::inspect_duration(&files[0], &settings.ffmpeg).unwrap();
            assert!((duration - 1.).abs() < 0.25);
            println!("Section fixture: duration {duration}s, transmitted {} / {} bytes, fallback {fallback}",transferred.load(Ordering::SeqCst),bytes.len());
        })();
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
        result
    }
    #[test]
    #[ignore = "Requires local yt-dlp and FFmpeg fixtures"]
    fn synthetic_download() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let d = tempfile::tempdir().unwrap();
        let ytdlp = PathBuf::from(std::env::var("QC_TEST_YTDLP").unwrap());
        let ffmpeg = PathBuf::from(std::env::var("QC_TEST_FFMPEG").unwrap());
        let source = d.path().join("fixture.mp4");
        let status = command(&ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=purple:s=160x90:r=24:d=1",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-c:v",
                "mpeg4",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let bytes = fs::read(&source).unwrap();
        let thumb_path = d.path().join("thumbnail.png");
        assert!(command(&ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=purple:s=160x90",
                "-frames:v",
                "1",
                "-threads",
                "1"
            ])
            .arg(&thumb_path)
            .status()
            .unwrap()
            .success());
        let thumb = fs::read(thumb_path).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let server = thread::spawn(move || {
            let bytes = Arc::new(bytes);
            let thumb = Arc::new(thumb);
            let mut clients = vec![];
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        let bytes = bytes.clone();
                        let thumb = thumb.clone();
                        clients.push(thread::spawn(move||{
                        let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut request = vec![];
                        let mut one = [0u8; 1];
                        while request.len() < 16384 && socket.read_exact(&mut one).is_ok() {
                            request.push(one[0]);
                            if request.ends_with(b"\r\n\r\n") {
                                break;
                            }
                        }
                        let head = request.starts_with(b"HEAD ");
                        let line = String::from_utf8_lossy(&request);
                        let path = line.split_whitespace().nth(1).unwrap_or("");
                        let page=format!("<html><head><title>QuickClip fixture</title><meta property=\"og:title\" content=\"QuickClip fixture\"><meta property=\"og:image\" content=\"http://{address}/thumbnail.png\"><meta property=\"og:video\" content=\"http://{address}/fixture.mp4\"><meta property=\"og:video:type\" content=\"video/mp4\"></head><body><video poster=\"http://{address}/thumbnail.png\" src=\"http://{address}/fixture.mp4\"></video></body></html>");
                        let (mime, body) = match path {
                            "/page" => ("text/html", page.as_bytes()),
                            "/thumbnail.png" => ("image/png", thumb.as_slice()),
                            _ => ("video/mp4", bytes.as_slice()),
                        };
                        let header=format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
                        let _ = socket.write_all(header.as_bytes());
                        if !head {let _ = socket.write_all(body);}
                        let _=socket.flush();
                        let _=socket.shutdown(std::net::Shutdown::Write);
                        let _=socket.set_read_timeout(Some(Duration::from_millis(500)));
                        let mut tail=[0u8;256];
                        while matches!(socket.read(&mut tail),Ok(n) if n>0){}
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
            for client in clients {
                let _ = client.join();
            }
        });
        let out = d.path().join("out");
        let settings = Settings {
            destination: out.clone(),
            yt_dlp: ytdlp,
            ffmpeg,
            prefs: Preferences::default(),
        };
        let job = Job {
            id: Uuid::new_v4().simple().to_string(),
            url: format!("http://{address}/fixture.mp4"),
            preset: "editor".into(),
            quality: "best".into(),
            playlist: false,
            organize: true,
            options: MediaOptions {
                clip_start: Some(0.2),
                clip_end: Some(0.8),
                ..Default::default()
            },
            destination: out,
            status: Status::Running,
            progress: 0.,
            message: String::new(),
            files: vec![],
            resume_on_startup: false,
        };
        let metadata = preview(&job.url, &settings.yt_dlp);
        let thumb_preview = preview(&format!("http://{address}/page"), &settings.yt_dlp);
        let result = download(&job, &settings, &AtomicBool::new(false), |_, _| {});
        let queue_root = d.path().join("queue");
        let mut parallel_settings = settings.clone();
        parallel_settings.destination = queue_root.clone();
        parallel_settings.prefs.concurrent_downloads = 2;
        let engine = Engine::open(d.path().join("parallel.json"), parallel_settings).unwrap();
        for n in 0..2 {
            engine
                .enqueue(Request {
                    url: format!("http://{address}/fixture.mp4?job={n}"),
                    preset: "original".into(),
                    quality: "best".into(),
                    playlist: false,
                    organize: false,
                    options: MediaOptions::default(),
                    destination: queue_root.clone(),
                })
                .unwrap();
        }
        let simultaneous = engine
            .snapshot()
            .jobs
            .iter()
            .filter(|j| j.status == Status::Running)
            .count();
        let start = Instant::now();
        while engine
            .snapshot()
            .jobs
            .iter()
            .any(|j| matches!(j.status, Status::Queued | Status::Running))
            && start.elapsed() < Duration::from_secs(30)
        {
            thread::sleep(Duration::from_millis(50));
        }
        let parallel = engine.snapshot();
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
        assert_eq!(simultaneous, 2);
        assert!(
            parallel.jobs.iter().all(|j| j.status == Status::Completed),
            "{:?}",
            parallel.jobs
        );
        assert_ne!(parallel.jobs[0].files[0], parallel.jobs[1].files[0]);
        assert!(!metadata.unwrap().title.is_empty());
        let thumb_preview = thumb_preview.unwrap();
        assert!(
            thumb_preview
                .thumbnail
                .as_deref()
                .is_some_and(|s| s.starts_with("data:image/png;base64,")),
            "Fixture thumbnail was not obtained: {:?}",
            thumb_preview
        );
        let files = result.unwrap();
        assert!(files[0].parent().unwrap().ends_with("Video/Other"));
        assert_eq!(files.len(), 1);
        assert!(files[0].is_file());
        assert!(source.is_file());
    }
    #[test]
    fn bulk_cancel_preserves_completed_jobs() {
        let d = tempfile::tempdir().unwrap();
        let engine = Engine::open(
            d.path().join("state.json"),
            Settings {
                destination: d.path().into(),
                yt_dlp: PathBuf::new(),
                ffmpeg: PathBuf::new(),
                prefs: Preferences::default(),
            },
        )
        .unwrap();
        let active = Arc::new(AtomicBool::new(false));
        {
            let mut x = engine.inner.lock().unwrap();
            x.active.insert("active".into(), active.clone());
            for (id, status) in [
                ("active", Status::Running),
                ("pending", Status::Queued),
                ("done", Status::Completed),
            ] {
                x.state.jobs.push(Job {
                    id: id.into(),
                    url: "https://example.com/video".into(),
                    preset: "editor".into(),
                    quality: "1080".into(),
                    playlist: false,
                    organize: false,
                    options: MediaOptions::default(),
                    destination: d.path().into(),
                    status,
                    progress: 0.,
                    message: String::new(),
                    files: vec![],
                    resume_on_startup: false,
                });
            }
        }
        engine.bulk_action("clear_pending").unwrap();
        assert!(!active.load(Ordering::SeqCst));
        assert_eq!(engine.snapshot().jobs[1].status, Status::Cancelled);
        assert_eq!(engine.snapshot().jobs[2].status, Status::Completed);
        engine.bulk_action("cancel_all").unwrap();
        assert!(active.load(Ordering::SeqCst));
        engine.bulk_action("clear_completed").unwrap();
        assert_eq!(engine.snapshot().jobs.len(), 2);
    }
    #[test]
    fn thumbnail_encoding_and_rejection() {
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("bad.svg"), b"<svg/>").unwrap();
        assert!(read_thumbnail(d.path()).is_none());
        fs::write(d.path().join("preview.png"), b"\x89PNG\r\n\x1a\nfixture").unwrap();
        assert!(read_thumbnail(d.path())
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }
    #[test]
    fn held_jobs_and_concurrency_slots() {
        let d = tempfile::tempdir().unwrap();
        let engine = Engine::open(
            d.path().join("state.json"),
            Settings {
                destination: d.path().into(),
                yt_dlp: PathBuf::new(),
                ffmpeg: PathBuf::new(),
                prefs: Preferences::default(),
            },
        )
        .unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        {
            let mut x = engine.inner.lock().unwrap();
            x.active.insert("occupied".into(), flag.clone());
        }
        let request = Request {
            url: "https://example.com/media".into(),
            preset: "original".into(),
            quality: "best".into(),
            playlist: false,
            organize: false,
            options: MediaOptions::default(),
            destination: d.path().into(),
        };
        let id = engine.enqueue(request).unwrap();
        assert_eq!(engine.snapshot().jobs[0].status, Status::Queued);
        engine.hold(&id, true).unwrap();
        assert_eq!(engine.snapshot().jobs[0].status, Status::Paused);
        let reopened =
            Engine::open(d.path().join("state.json"), engine.snapshot().settings).unwrap();
        assert_eq!(reopened.snapshot().jobs[0].status, Status::Paused);
        engine.hold(&id, false).unwrap();
        assert_eq!(engine.snapshot().jobs[0].status, Status::Queued);
        assert!(engine.hold("missing", true).is_err());
        engine.cancel(&id).unwrap();
        assert_eq!(engine.snapshot().jobs[0].status, Status::Cancelled);
    }
    #[test]
    #[ignore = "Requires external network and a licensed test source"]
    fn public_metadata_preview() {
        let tool = PathBuf::from(std::env::var("QC_TEST_YTDLP").unwrap());
        let mut available = 0;
        let mut thumbnails = 0;
        for url in [
            "https://commons.wikimedia.org/wiki/File:Big_Buck_Bunny_Trailer_400p.ogv",
            "https://vimeo.com/1084537",
            "https://www.youtube.com/watch?v=YE7VzlLtp-4",
        ] {
            match preview(url, &tool) {
                Ok(p) => {
                    println!(
                        "Public preview {url}: title={}, thumbnail={}",
                        p.title,
                        p.thumbnail.is_some()
                    );
                    if !p.title.is_empty() {
                        available += 1;
                    }
                    if p.thumbnail.is_some() {
                        thumbnails += 1;
                    }
                }
                Err(e) => println!("Public preview {url}: {e}"),
            }
        }
        assert!(available > 0, "No external metadata was obtained");
        assert!(thumbnails > 0, "No external thumbnail was obtained");
    }
    #[test]
    fn state_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let settings = Settings {
            destination: d.path().into(),
            yt_dlp: PathBuf::new(),
            ffmpeg: PathBuf::new(),
            prefs: Preferences::default(),
        };
        let p = d.path().join("state.json");
        let engine = Engine::open(p.clone(), settings.clone()).unwrap();
        assert_eq!(engine.snapshot().settings.destination, settings.destination);
        drop(engine);
        assert!(Engine::open(p, settings).is_ok());
    }
}

#[cfg(test)]
#[test]
fn classified_media_errors() {
    assert_eq!(
        classify_error(
            "error: this video has been removed for violating youtube's community guidelines"
        ),
        "The video was removed by the source."
    );
    assert_eq!(
        classify_error("error: private video"),
        "This video is private."
    );
    assert_eq!(
        classify_error("error: connection timed out"),
        "Network error. Check connectivity and retry."
    );
    assert_eq!(
        classify_error("error: requested format is not available"),
        "Selected format is unavailable. Try Best or Original."
    );
    assert_eq!(
        classify_error("error: no space left"),
        "Check folder permissions and available storage."
    );
}

#[cfg(test)]
#[test]
fn crash_recovery_and_idle_update_gate() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("state.json");
    let settings = Settings {
        destination: d.path().to_owned(),
        yt_dlp: PathBuf::new(),
        ffmpeg: PathBuf::new(),
        prefs: Preferences::default(),
    };
    let make = |status: Status| Job {
        id: Uuid::new_v4().simple().to_string(),
        url: "https://example.com/fixture".into(),
        preset: "editor".into(),
        quality: "720".into(),
        playlist: false,
        organize: false,
        options: Default::default(),
        destination: d.path().to_owned(),
        status,
        progress: 0.2,
        message: "Downloading".into(),
        files: vec![],
        resume_on_startup: false,
    };
    let valid = make(Status::Running);
    let stage = Stage::open(&valid).unwrap();
    fs::write(stage.path().join("fixture.mp4.part"), b"partial data").unwrap();
    let unsafe_job = make(Status::Running);
    let unsafe_stage = Stage::open(&unsafe_job).unwrap();
    fs::write(
        unsafe_stage.path().join("unexpected.mp4"),
        b"completed or unknown output",
    )
    .unwrap();
    let queued = make(Status::Queued);
    let manual = make(Status::Paused);
    persist(
        &path,
        &Snapshot {
            settings: settings.clone(),
            jobs: vec![valid.clone(), unsafe_job.clone(), queued, manual],
        },
    )
    .unwrap();
    let e = Engine::open(path, settings).unwrap();
    let jobs = e.snapshot().jobs;
    assert_eq!(jobs[0].status, Status::Paused);
    assert!(jobs[0].resume_on_startup);
    assert_eq!(jobs[1].status, Status::Failed);
    assert!(!jobs[1].resume_on_startup);
    assert_eq!(jobs[2].status, Status::Paused);
    assert!(jobs[2].resume_on_startup);
    assert_eq!(jobs[3].status, Status::Paused);
    assert!(!jobs[3].resume_on_startup);
    assert_eq!(
        fs::read(unsafe_stage.path().join("unexpected.mp4")).unwrap(),
        b"completed or unknown output"
    );
    assert!(e.begin_idle_update().is_err());
    for j in jobs.iter().filter(|j| j.status == Status::Paused) {
        e.cancel(&j.id).unwrap();
    }
    assert!(e.begin_idle_update().is_ok());
    assert!(e.safe_exit_ready());
    e.abort_safe_exit();
    let guard = e.tool_maintenance().unwrap();
    assert!(e.begin_idle_update().is_err());
    drop(guard);
    assert!(e.begin_idle_update().is_ok());
}

#[cfg(test)]
mod stability_publication_tests {
    use super::*;
    #[test]
    fn group_names_match_and_existing_sidecar_is_preserved() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("source");
        let dest = d.path().join("dest");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&dest).unwrap();
        let a = source.join("video.mp4");
        let b = source.join("video.en.vtt");
        fs::write(&a, b"video").unwrap();
        fs::write(&b, b"subtitle").unwrap();
        fs::write(dest.join("video.en.vtt"), b"existing").unwrap();
        let mut saved = vec![];
        publish_group(
            "video",
            &[(a, ".mp4".into()), (b, ".en.vtt".into())],
            &dest,
            &AtomicBool::new(false),
            |p| {
                saved.push(p);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            saved,
            vec![dest.join("video (1).mp4"), dest.join("video (1).en.vtt")]
        );
        assert_eq!(fs::read(dest.join("video.en.vtt")).unwrap(), b"existing");
        assert_eq!(fs::read(&saved[1]).unwrap(), b"subtitle");
    }
    #[test]
    fn bad_sidecar_or_cancellation_leaves_no_published_outputs() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("source");
        let dest = d.path().join("dest");
        fs::write(&source, b"video").unwrap();
        fs::create_dir(&dest).unwrap();
        let mut count = 0;
        assert!(publish_group(
            "video",
            &[
                (source.clone(), ".mp4".into()),
                (d.path().join("missing"), ".vtt".into())
            ],
            &dest,
            &AtomicBool::new(false),
            |_| {
                count += 1;
                Ok(())
            }
        )
        .is_err());
        assert!(publish_group(
            "video",
            &[(source, ".mp4".into())],
            &dest,
            &AtomicBool::new(true),
            |_| {
                count += 1;
                Ok(())
            }
        )
        .is_err());
        assert_eq!(count, 0);
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
    }
    #[test]
    fn late_commit_error_records_actual_outputs_and_no_clobber() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        let b = d.path().join("b");
        let dest = d.path().join("dest");
        fs::write(&a, b"media").unwrap();
        fs::write(&b, b"sidecar").unwrap();
        fs::create_dir(&dest).unwrap();
        let mut saved = vec![];
        let result = publish_group(
            "video",
            &[(a, ".mp4".into()), (b, ".vtt".into())],
            &dest,
            &AtomicBool::new(false),
            |p| {
                saved.push(p);
                fs::write(dest.join("video.vtt"), b"external").unwrap();
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(saved, vec![dest.join("video.mp4")]);
        assert_eq!(fs::read(dest.join("video.vtt")).unwrap(), b"external");
    }
    #[cfg(windows)]
    #[test]
    fn bounded_process_times_out_and_cancels() {
        let cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/ping.exe");
        let mut c = command(&cmd);
        c.args(["-t", "127.0.0.1"]);
        let start = Instant::now();
        assert!(
            bounded_output(&mut c, &AtomicBool::new(false), Duration::from_millis(200)).is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(5));
        let mut c = command(&cmd);
        c.args(["-t", "127.0.0.1"]);
        assert!(bounded_output(&mut c, &AtomicBool::new(true), Duration::from_secs(1)).is_err());
    }
}

#[cfg(test)]
#[test]
fn safe_url_identity_does_not_merge_signed_or_playlist_links() {
    let base = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let share = "https://youtu.be/dQw4w9WgXcQ?si=abc";
    assert!(same_media_url(base, share, false));
    assert!(!same_media_url(base, share, true));
    assert!(!same_media_url(
        base,
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=5",
        false
    ));
    assert!(!same_media_url(
        "https://example.com/a?sig=one",
        "https://example.com/a?sig=two",
        false
    ));
}
