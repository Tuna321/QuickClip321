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
        || ["/playlist", "/sets/", "/albums/", "/album/"]
            .iter()
            .any(|p| u.path().to_lowercase().contains(p))
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
        "node",
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
    c.arg("--")
        .arg(&url)
        .stdout(Stdio::from(output.try_clone().map_err(err)?))
        .stderr(Stdio::null());
    let mut child = c.spawn().map_err(err)?;
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
        "editor" => Ok(format!(
            "bestvideo{cap}[vcodec^=avc1]+bestaudio[acodec^=mp4a]/{video}"
        )),
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
    active: Option<(String, Arc<AtomicBool>)>,
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
                active: None,
            })),
            file,
        })
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
            matches!(j.status, Status::Queued | Status::Running)
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
                    if j.status == Status::Queued {
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
            if let Some((_, flag)) = &x.active {
                flag.store(true, Ordering::SeqCst);
            }
        }
        Ok(())
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let mut x = self.inner.lock().unwrap();
        if let Some((active, flag)) = &x.active {
            if active == id {
                flag.store(true, Ordering::SeqCst);
                return Ok(());
            }
        }
        let mut next = x.state.clone();
        let j = next
            .jobs
            .iter_mut()
            .find(|j| j.id == id)
            .ok_or("Unknown job")?;
        if j.status != Status::Queued {
            return Err("Job is not queued.".into());
        }
        j.status = Status::Cancelled;
        j.message = "Cancelled".into();
        persist(&self.file, &next)?;
        x.state = next;
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
        next.jobs
            .retain(|j| j.id != id || matches!(j.status, Status::Running | Status::Queued));
        persist(&self.file, &next)?;
        x.state = next;
        Ok(())
    }
    fn kick(&self) {
        let mut x = self.inner.lock().unwrap();
        if x.active.is_some() {
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
        x.active = Some((j.id.clone(), cancel.clone()));
        if let Err(e) = persist(&self.file, &x.state) {
            x.state.jobs[i].status = Status::Failed;
            x.state.jobs[i].message = format!("Cannot save state: {e}");
            x.active = None;
            return;
        }
        drop(x);
        let engine = self.clone();
        thread::spawn(move || {
            let result = download(&j, &settings, &cancel, |p, m| {
                let mut x = engine.inner.lock().unwrap();
                if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                    job.progress = p;
                    job.message = m;
                }
            });
            let mut x = engine.inner.lock().unwrap();
            if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                match result {
                    Ok(files) => {
                        job.files = files;
                        job.status = Status::Completed;
                        job.progress = 1.;
                        job.message = "Ready".into();
                    }
                    Err(e) => {
                        job.status = if cancel.load(Ordering::SeqCst) {
                            Status::Cancelled
                        } else {
                            Status::Failed
                        };
                        job.message = e;
                    }
                }
            }
            if let Err(e) = persist(&engine.file, &x.state) {
                if let Some(job) = x.state.jobs.iter_mut().find(|job| job.id == j.id) {
                    job.message = format!("Result not saved: {e}");
                }
            }
            x.active = None;
            drop(x);
            engine.kick();
        });
    }
}
fn command(tool: &Path) -> Command {
    let mut c = Command::new(tool);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c
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
fn ensure_editor(path: &Path, ffmpeg: &Path, cancel: &AtomicBool) -> Result<PathBuf> {
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
                s["codec_type"] == "video" && s["codec_name"] == "h264" && s["pix_fmt"] == "yuv420p"
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
    let mut child = command(ffmpeg)
        .args(["-nostdin", "-v", "error", "-n", "-i"])
        .arg(path)
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
fn download(
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
    let staging = tempfile::Builder::new()
        .prefix(".quickclip-v3-")
        .tempdir_in(&j.destination)
        .map_err(err)?;
    let mut c = command(&s.yt_dlp);
    c.args([
        "--ignore-config",
        "--no-plugin-dirs",
        "--newline",
        "--no-warnings",
        "--progress",
        "--progress-template",
        "download:QC:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s",
        "--progress-template",
        "postprocess:QC_POST:%(progress.status)s",
        "--windows-filenames",
        "--no-overwrites",
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
    c.args(["--js-runtimes", "node"])
        .arg("--ffmpeg-location")
        .arg(&s.ffmpeg)
        .arg("-f")
        .arg(selector(&j.preset, &j.quality)?)
        .arg("-o")
        .arg(staging.path().join("%(title).90B [%(id)s].%(ext)s"))
        .args([
            "--no-simulate",
            "--print-to-file",
            "after_move:%(.{filepath,title,artist,album_artist,channel,uploader,series,tv_show,movie,movie_title,vcodec,track,genre})j",
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
    c.arg("--")
        .arg(&j.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().map_err(err)?;
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let stderr = child.stderr.take().ok_or("Missing stderr")?;
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let errors = thread::spawn(move || {
        for _ in BufReader::new(stderr)
            .lines()
            .map_while(std::result::Result::ok)
        {}
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
    let _ = reader.join();
    let _ = errors.join();
    if let Err(e) = exit {
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
                ensure_editor(&path, &s.ffmpeg, cancel)?
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
                let (category, _) = organizer::category(&metadata, &j.preset);
                j.destination.join(organizer::folder(category))
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
        Ok(files) => Ok(files),
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
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
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
                        if !head {
                            let _ = socket.write_all(body);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
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
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
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
            x.active = Some(("active".into(), active.clone()));
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
