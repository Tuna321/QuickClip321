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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    let normalized = if raw.starts_with("www.") {
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
fn js_runtime(tool: &Path) -> String {
    if let Some(dir) = tool.parent() {
        if dir.file_name().and_then(|s| s.to_str()) == Some("managed-2026.11.23-9.1-24.22.3")
            && dir.join("node.exe").is_file()
        {
            return format!("node:{}", dir.join("node.exe").display());
        }
    }
    "node".into()
}
pub fn preview(raw: &str, tool: &Path) -> Result<Preview> {
    use std::io::{Read, Seek, SeekFrom};
    let url = validate_url(raw.trim())?;
    if !tool.is_absolute() || !tool.is_file() {
        return Err("Media engine is not configured.".into());
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
        if started.elapsed() > Duration::from_secs(22)
            || output.metadata().map_err(err)?.len() > 8_000_000
        {
            terminate(&mut child);
            return Err("Preview timed out or exceeded its size limit.".into());
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
        "mp3" | "wav" | "m4a" => Ok("bestaudio/best".into()),
        _ => Err("Unknown preset.".into()),
    }
}
fn persist(path: &Path, state: &Snapshot) -> Result<()> {
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
}
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Mutex<Inner>>,
    file: PathBuf,
}
impl Engine {
    pub fn open(file: PathBuf, defaults: Settings) -> Result<Self> {
        let mut state = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice::<Snapshot>(&bytes)
                .map_err(|_| "State file is damaged; preserved for recovery.")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Snapshot {
                settings: defaults.clone(),
                jobs: vec![],
            },
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
            if matches!(j.status, Status::Running | Status::Queued) {
                j.status = Status::Failed;
                j.message = "Interrupted. Retry explicitly.".into();
            }
        }
        persist(&file, &state)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                state,
                active: Default::default(),
                pause_requests: Default::default(),
                notices: Default::default(),
            })),
            file,
        })
    }
    /// Install version-pinned tools in the app-owned directory, not on PATH.
    #[cfg(windows)]
    pub fn setup_managed_tools(&self) -> Result<Settings> {
        static INSTALL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = INSTALL
            .try_lock()
            .map_err(|_| "Tool setup is already running.")?;
        if self
            .snapshot()
            .jobs
            .iter()
            .any(|j| matches!(j.status, Status::Running | Status::Queued | Status::Paused))
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
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        let mut child = c.spawn().map_err(err)?;
        let _owner = ProcessTree::attach(&mut child)?;
        let start = Instant::now();
        loop {
            match child.try_wait(){Ok(Some(status)) if status.success()=>break,Ok(Some(_))=>return Err("Tool setup failed. Check connectivity or an existing managed-tool integrity problem, then retry.".into()),Ok(None)=>{},Err(e)=>{terminate(&mut child);return Err(err(e));}}
            if start.elapsed() > Duration::from_secs(780) {
                terminate(&mut child);
                return Err("Tool setup timed out.".into());
            }
            thread::sleep(Duration::from_millis(200));
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
    pub fn snapshot(&self) -> Snapshot {
        self.inner.lock().unwrap().state.clone()
    }
    pub fn settings(&self, mut settings: Settings) -> Result<()> {
        settings.prefs.normalize();
        if !settings.destination.is_absolute() {
            return Err("Choose an absolute destination folder.".into());
        }
        for tool in [&settings.yt_dlp, &settings.ffmpeg] {
            if !tool.as_os_str().is_empty() && (!tool.is_absolute() || !tool.is_file()) {
                return Err("Tool paths must identify existing executable files.".into());
            }
        }
        let mut x = self.inner.lock().unwrap();
        let mut next = x.state.clone();
        next.settings = settings;
        persist(&self.file, &next)?;
        x.state = next;
        drop(x);
        self.kick();
        Ok(())
    }
    pub fn enqueue(&self, r: Request) -> Result<String> {
        let url = validate_url(r.url.trim())?;
        selector(&r.preset, &r.quality)?;
        r.options.validate()?;
        if !r.destination.is_absolute() {
            return Err("Choose an absolute destination folder.".into());
        }
        let mut x = self.inner.lock().unwrap();
        if x.state.jobs.iter().any(|j| {
            matches!(j.status, Status::Queued | Status::Running | Status::Paused)
                && j.url == url
                && j.preset == r.preset
                && j.quality == r.quality
                && j.playlist == r.playlist
                && j.organize == r.organize
                && j.options == r.options
                && j.destination == r.destination
        }) {
            return Err("Already in the queue.".into());
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
        });
        persist(&self.file, &next)?;
        x.state = next;
        drop(x);
        self.kick();
        Ok(id)
    }
    pub fn bulk_action(&self, action: &str) -> Result<()> {
        if !matches!(action, "cancel_all" | "clear_pending" | "clear_completed") {
            return Err("Unknown queue action.".into());
        }
        let mut x = self.inner.lock().unwrap();
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
        let mut next = x.state.clone();
        let j = next
            .jobs
            .iter_mut()
            .find(|j| j.id == id)
            .ok_or("Unknown job")?;
        if paused && j.status == Status::Running {
            if !resumable(j) || !j.message.starts_with("Downloading") || !safe_partial(j) {
                return Err("Pause is available during a single Original download with a reusable partial file.".into());
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
        next.jobs.retain(|j| {
            j.id != id || matches!(j.status, Status::Running | Status::Queued | Status::Paused)
        });
        persist(&self.file, &next)?;
        x.state = next;
        Ok(())
    }
    fn kick(&self) {
        let mut x = self.inner.lock().unwrap();
        if x.active.len() >= usize::from(x.state.settings.prefs.concurrent_downloads) {
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
            let result = download(&j, &settings, &cancel, |p, m| {
                let mut x = engine.inner.lock().unwrap();
                if x.pause_requests.contains(&j.id) {
                    return;
                }
                if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                    job.progress = p;
                    job.message = m;
                }
            });
            let mut x = engine.inner.lock().unwrap();
            let pause_requested = x.pause_requests.remove(&j.id);
            if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                match result {
                    Ok(files) => {
                        job.files = files;
                        job.status = Status::Completed;
                        job.progress = 1.;
                        job.message = "Download completed".into();
                    }
                    Err(e) => {
                        if pause_requested && safe_partial(job) {
                            job.status = Status::Paused;
                            job.message="Paused. Partial data retained; Resume continues when supported by the source.".into();
                        } else {
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
fn publish(src: &Path, dest: &Path) -> Result<PathBuf> {
    let name = src
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid filename")?;
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("media");
    for i in 0..10000 {
        let target = dest.join(if i == 0 {
            format!("{name}.{ext}")
        } else {
            format!("{name} ({i}).{ext}")
        });
        if target.exists() {
            continue;
        }
        let mut tmp = tempfile::NamedTempFile::new_in(dest).map_err(err)?;
        std::io::copy(&mut File::open(src).map_err(err)?, &mut tmp).map_err(err)?;
        tmp.flush().map_err(err)?;
        tmp.as_file().sync_all().map_err(err)?;
        match tmp.persist_noclobber(&target) {
            Ok(_) => return Ok(target),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(err(e)),
        }
    }
    Err("Too many conflicting filenames.".into())
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
        let result = command(&probe)
            .args(["-v", "error", "-show_streams", "-of", "json"])
            .arg(file)
            .output()
            .map_err(err)?;
        if !result.status.success() {
            return Err("FFprobe could not inspect the output.".into());
        }
        let info: serde_json::Value = serde_json::from_slice(&result.stdout).map_err(err)?;
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
    if lower.contains("403")
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
fn download(
    j: &Job,
    s: &Settings,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64, String),
) -> Result<Vec<PathBuf>> {
    for attempt in 0..=2 {
        match download_once(j, s, cancel, &mut progress) {
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
fn resumable(j: &Job) -> bool {
    j.preset == "original"
        && !j.playlist
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
fn download_once(
    j: &Job,
    s: &Settings,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64, String),
) -> Result<Vec<PathBuf>> {
    if !s.yt_dlp.is_absolute()
        || !s.yt_dlp.is_file()
        || !s.ffmpeg.is_absolute()
        || !s.ffmpeg.is_file()
    {
        return Err("Set existing yt-dlp and FFmpeg executable paths in Settings.".into());
    }
    fs::create_dir_all(&j.destination).map_err(err)?;
    let staging = Stage::open(j)?;
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
        &s.prefs.fragment_concurrency.to_string(),
    ]);
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
        if started.elapsed() > Duration::from_secs(4 * 3600) {
            terminate(&mut child);
            break Err("Operation exceeded four hours.".into());
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
                            "Downloading {} | {} | ETA {}",
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
                e.push_str(" ");
                e.push_str(h);
            }
        }
        let retained = staging.keep();
        return Err(format!(
            "{e} Partial files retained at {}",
            retained.display()
        ));
    }
    let result = (|| -> Result<Vec<PathBuf>> {
        let mut files = vec![];
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
            let ready = if j.preset == "editor" {
                progress(0.98, "Verifying editor compatibility".into());
                ensure_editor(&path, &s.ffmpeg, &j.quality, cancel)?
            } else {
                path.clone()
            };
            let ready = if j.options.clipping() {
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
        for (ready, destination, source, sidecars, archive_files) in ready_files {
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelled".into());
            }
            let target = if fs::create_dir_all(&destination).is_ok() {
                &destination
            } else {
                &j.destination
            };
            let published = publish(&ready, target)?;
            let stem = published
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid published media name")?;
            let source_stem = source
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid source name")?;
            files.push(published.clone());
            for sidecar in sidecars.into_iter().chain(archive_files) {
                let name = sidecar
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid sidecar name")?;
                let suffix = name
                    .strip_prefix(source_stem)
                    .ok_or("Unexpected sidecar name")?;
                let new_name = format!("{stem}{suffix}");
                // Write a new sidecar in a job-owned temporary folder; never overwrite originals.
                let tmp = tempfile::tempdir_in(staging.path()).map_err(err)?;
                let dest = tmp.path().join(new_name);
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
                } else {
                    fs::copy(&sidecar, &dest).map_err(err)?;
                }
                files.push(publish(&dest, target)?);
            }
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
            let retained = staging.keep();
            Err(format!("{e} Staging retained at {}", retained.display()))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
        for file in ["yt-dlp.exe", "ffmpeg.exe", "ffprobe.exe", "node.exe"] {
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
        assert_eq!(engine.snapshot().settings.yt_dlp, installed.yt_dlp);
        assert!(js_runtime(&installed.yt_dlp).starts_with("node:"));
        fs::write(target.join("yt-dlp.exe"), b"tampered fixture").unwrap();
        assert!(engine.setup_managed_tools().is_err());
        assert_eq!(
            fs::read(target.join("yt-dlp.exe")).unwrap(),
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
                "2"
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
        let high = Arc::new(AtomicU64::new(0));
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
                        workers.push(thread::spawn(move||{
                            let _=sock.set_read_timeout(Some(Duration::from_secs(3)));let _=sock.set_write_timeout(Some(Duration::from_secs(3)));
                            let mut request=vec![];let mut byte=[0];
                            while request.len()<16384&&sock.read_exact(&mut byte).is_ok(){request.push(byte[0]);if request.ends_with(b"\r\n\r\n"){break}}
                            let text=String::from_utf8_lossy(&request).to_ascii_lowercase();
                            let range=text.lines().find_map(|line|line.strip_prefix("range: bytes=")).and_then(|v|v.split('-').next()).and_then(|v|v.parse::<usize>().ok()).unwrap_or(0);
                            if range>=data.len(){let _=sock.write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");return}
                            if range>0{range_seen.fetch_max(range as u64,Ordering::SeqCst);}
                            let status=if range>0{"206 Partial Content"}else{"200 OK"};
                            let extra=if range>0{format!("Content-Range: bytes {}-{}/{}\r\n",range,data.len()-1,data.len())}else{String::new()};
                            let h=format!("HTTP/1.1 {status}\r\nContent-Type: video/mp4\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",data.len()-range);
                            if sock.write_all(h.as_bytes()).is_err(){return}
                            if !text.starts_with("head "){for block in data[range..].chunks(16*1024){if stopping.load(Ordering::SeqCst)||sock.write_all(block).is_err(){break;}thread::sleep(Duration::from_millis(15));}}
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
        let id = engine
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
                    engine.hold(&id, true).unwrap();
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
            // Reopen persisted paused job, equivalent to an orderly app restart.
            drop(engine);
            let reopened = Engine::open(state, settings).unwrap();
            assert_eq!(reopened.snapshot().jobs[0].status, Status::Paused);
            reopened.hold(&id, false).unwrap();
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
        assert_eq!(files[0].parent().unwrap().file_name().unwrap(), "Video");
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
