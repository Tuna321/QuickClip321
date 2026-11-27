use quickclip_core::media_options::{trim_subtitle, MediaOptions};
use quickclip_core::preferences::Preferences;
use quickclip_core::{validate_url, Engine, Job, Request, Settings, Snapshot, Status};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
fn tmp() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap()
}
fn settings(root: &Path) -> Settings {
    Settings {
        destination: root.join("media"),
        yt_dlp: PathBuf::new(),
        ffmpeg: PathBuf::new(),
        prefs: Preferences::default(),
    }
}
fn job(s: &Settings, preset: &str, quality: &str, status: Status) -> Job {
    Job {
        id: "a".repeat(32),
        url: "http://127.0.0.1:9/sample.mp4".into(),
        preset: preset.into(),
        quality: quality.into(),
        playlist: false,
        organize: false,
        options: MediaOptions::default(),
        destination: s.destination.clone(),
        status,
        progress: 0.1,
        message: "Paused".into(),
        files: vec![],
        resume_on_startup: false,
    }
}
fn seed(root: &Path, s: &Settings, jobs: Vec<Job>) -> Engine {
    let file = root.join("state.json");
    fs::write(
        &file,
        serde_json::to_vec(&Snapshot {
            settings: s.clone(),
            jobs,
        })
        .unwrap(),
    )
    .unwrap();
    Engine::open(file, s.clone()).unwrap()
}
#[test]
fn fixed_audio_active_duplicate_diff_quality() {
    let d = tmp();
    let s = settings(d.path());
    let j = job(&s, "mp3", "1080", Status::Paused);
    let e = seed(d.path(), &s, vec![j.clone()]);
    let r = Request {
        url: j.url,
        preset: "mp3".into(),
        quality: "best".into(),
        playlist: false,
        organize: false,
        options: MediaOptions::default(),
        destination: s.destination.clone(),
    };
    assert!(e
        .enqueue_checked(r, false)
        .unwrap_err()
        .contains("Already in the queue"));
    assert_eq!(e.snapshot().jobs.len(), 1);
}
#[test]
fn fixed_cancel_remove_cleans_owned_partial() {
    let d = tmp();
    let s = settings(d.path());
    let j = job(&s, "original", "best", Status::Paused);
    let path = s.destination.join(format!(".quickclip-resume-{}", j.id));
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join(".owner"), &j.id).unwrap();
    fs::write(path.join("sample.mp4.part"), b"fixture").unwrap();
    let e = seed(d.path(), &s, vec![j.clone()]);
    e.cancel(&j.id).unwrap();
    e.remove(&j.id).unwrap();
    assert!(e.snapshot().jobs.is_empty());
    assert!(!path.exists());
}
#[test]
fn fixed_import_normalization_resolves_conflicting_shortcuts() {
    let mut p = Preferences::default();
    p.shortcuts.insert("download".into(), "Ctrl+Shift+3".into());
    p.normalize();
    let s = p.effective_shortcuts();
    assert_eq!(s["download"], "Ctrl+Shift+3");
    assert_eq!(s["mp3"], "Off");
}
#[test]
fn fixed_import_fragment_value_has_ui_choice() {
    let mut p = Preferences::default();
    p.fragment_concurrency = 3;
    p.normalize();
    assert_eq!(p.fragment_concurrency, 3);
    let html = include_str!("../../ui/index.html");
    let options = html
        .split(r#"id="fragments""#)
        .nth(1)
        .unwrap()
        .split("</select>")
        .next()
        .unwrap();
    assert!(options.contains("<option>3</option>"));
}
#[test]
fn fixed_non_executable_tool_and_file_destination_rejected() {
    let d = tmp();
    let mut s = settings(d.path());
    let file = d.path().join("not-an-executable.exe");
    fs::write(&file, b"not an executable").unwrap();
    let e = seed(d.path(), &s, vec![]);
    let mut bad = s.clone();
    bad.yt_dlp = file.clone();
    assert!(e.settings(bad).is_err());
    let mut bad = s.clone();
    bad.ffmpeg = file.clone();
    assert!(e.settings(bad).is_err());
    s.destination = file.clone();
    assert!(e.settings(s.clone()).is_err());
    s.destination = file.join("child");
    assert!(e.settings(s).is_err());
}
#[test]
fn fixed_uppercase_www_accepted() {
    assert!(validate_url("www.example.com/video").is_ok());
    assert!(validate_url("WWW.example.com/video").is_ok());
}
#[test]
fn fixed_vtt_style_and_cue_identifier_preserved() {
    let s =
        "WEBVTT\n\nSTYLE\n::cue { color: lime; }\n\nmy-cue\n00:00:01.000 --> 00:00:03.000\nhello\n";
    let out = trim_subtitle(
        s,
        &MediaOptions {
            clip_start: Some(1.),
            clip_end: Some(3.),
            ..Default::default()
        },
        false,
    );
    assert!(out.contains("hello"));
    assert!(out.contains("STYLE"));
    assert!(out.contains("my-cue"));
}
#[test]
fn documented_retry_playlist_only_retries_one_item() {
    let d = tmp();
    let s = settings(d.path());
    let mut j = job(&s, "original", "best", Status::Failed);
    j.playlist = true;
    let e = seed(d.path(), &s, vec![j.clone()]);
    let id = e.retry(&j.id).unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5)
        && e.snapshot()
            .jobs
            .iter()
            .any(|x| x.id == id && x.status == Status::Running)
    {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !e.snapshot()
            .jobs
            .iter()
            .find(|x| x.id == id)
            .unwrap()
            .playlist
    );
}

#[test]
fn corrupt_state_recovers_exact_backup_without_replay() {
    let d = tmp();
    let s = settings(d.path());
    let p = d.path().join("state.json");
    let mut j = job(&s, "mp3", "best", Status::Paused);
    j.resume_on_startup = true;
    let e = seed(d.path(), &s, vec![j]);
    drop(e);
    let e = Engine::open(p.clone(), s.clone()).unwrap();
    drop(e);
    fs::write(&p, b"{broken").unwrap();
    let e = Engine::open(p.clone(), s.clone()).unwrap();
    assert!(e
        .recovery_notice()
        .unwrap()
        .contains("recovered from backup"));
    assert_eq!(e.snapshot().jobs.len(), 1);
    assert!(!e.snapshot().jobs[0].resume_on_startup);
    assert!(fs::read_dir(d.path()).unwrap().flatten().any(|f| f
        .file_name()
        .to_string_lossy()
        .contains("json.corrupt-")
        && fs::read(f.path()).unwrap() == b"{broken"));
    drop(e);
    assert!(Engine::open(p, s).unwrap().recovery_notice().is_none());
}
#[test]
fn corrupt_state_without_backup_preserves_original_bytes() {
    let d = tmp();
    let s = settings(d.path());
    let p = d.path().join("state.json");
    fs::write(&p, b"{broken").unwrap();
    let e = Engine::open(p.clone(), s.clone()).unwrap();
    assert!(e.recovery_notice().unwrap().contains("default settings"));
    assert!(e.snapshot().jobs.is_empty());
    assert!(fs::read_dir(d.path()).unwrap().flatten().any(|f| f
        .file_name()
        .to_string_lossy()
        .contains("json.corrupt-")
        && fs::read(f.path()).unwrap() == b"{broken"));
    assert!(serde_json::from_slice::<Snapshot>(&fs::read(p).unwrap()).is_ok());
}
#[test]
fn cleanup_preserves_unrecognized_recovery_media_and_history() {
    let d = tmp();
    let s = settings(d.path());
    let j = job(&s, "original", "best", Status::Cancelled);
    let path = s.destination.join(format!(".quickclip-resume-{}", j.id));
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join(".owner"), &j.id).unwrap();
    fs::write(path.join("completed.mp4"), b"valuable").unwrap();
    let e = seed(d.path(), &s, vec![j.clone()]);
    assert!(e.remove(&j.id).is_err());
    assert_eq!(e.snapshot().jobs.len(), 1);
    assert_eq!(fs::read(path.join("completed.mp4")).unwrap(), b"valuable");
}
#[test]
fn concurrent_preference_updates_do_not_overwrite_each_other() {
    let d = tmp();
    let s = settings(d.path());
    let e = seed(d.path(), &s, vec![]);
    let a = e.clone();
    let b = e.clone();
    let x = thread::spawn(move || {
        a.modify_settings(false, |s| {
            s.prefs.limit_kib_per_second = 123;
            thread::sleep(Duration::from_millis(10));
            Ok(())
        })
        .unwrap()
    });
    let y = thread::spawn(move || {
        b.modify_settings(false, |s| {
            s.prefs.language = "tr".into();
            Ok(())
        })
        .unwrap()
    });
    x.join().unwrap();
    y.join().unwrap();
    let actual = e.snapshot().settings.prefs;
    assert_eq!(actual.limit_kib_per_second, 123);
    assert_eq!(actual.language, "tr");
}
#[test]
fn category_path_file_is_rejected_when_saving() {
    let d = tmp();
    let mut s = settings(d.path());
    let e = seed(d.path(), &s, vec![]);
    let bad = d.path().join("not-a-folder");
    fs::write(&bad, b"file").unwrap();
    s.prefs.category_paths.insert("video".into(), bad);
    assert!(e.settings(s).is_err());
}
#[test]
fn partially_published_job_never_retries_implicitly() {
    let d = tmp();
    let s = settings(d.path());
    let mut j = job(&s, "original", "best", Status::Failed);
    fs::create_dir_all(&s.destination).unwrap();
    let file = s.destination.join("output.mp4");
    fs::write(&file, b"media").unwrap();
    j.files.push(file);
    let e = seed(d.path(), &s, vec![j.clone()]);
    assert!(e.retry(&j.id).unwrap_err().contains("already saved"));
}

#[test]
fn history_snapshot_does_not_copy_completed_history_into_active_queue() {
    let d = tmp();
    let s = settings(d.path());
    let mut jobs: Vec<_> = (0..5000)
        .map(|i| {
            let mut j = job(&s, "original", "best", Status::Completed);
            j.id = format!("{i:032x}");
            j
        })
        .collect();
    let mut active = job(&s, "original", "best", Status::Paused);
    active.id = "f".repeat(32);
    jobs.push(active);
    let e = seed(d.path(), &s, jobs);
    assert_eq!(e.active_snapshot().jobs.len(), 1);
    assert_eq!(e.snapshot().jobs.len(), 5001);
}
#[test]
fn youtube_share_and_watch_duplicate_but_signed_urls_remain_distinct() {
    let d = tmp();
    let s = settings(d.path());
    let mut j = job(&s, "original", "best", Status::Paused);
    j.url = "https://youtu.be/dQw4w9WgXcQ?si=abc".into();
    let e = seed(d.path(), &s, vec![j]);
    let r = Request {
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(),
        preset: "original".into(),
        quality: "best".into(),
        playlist: false,
        organize: false,
        options: MediaOptions::default(),
        destination: s.destination.clone(),
    };
    assert!(e
        .enqueue_checked(r, false)
        .unwrap_err()
        .contains("Already in the queue"));
}
