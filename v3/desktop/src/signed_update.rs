use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
const PUBLIC: &str = include_str!("update-public.xml");
const VERIFY: &str = include_str!("update-verify.ps1");
const INSTALL: &str = include_str!("update-install.ps1");
const ROOT: &str = "https://github.com/Tuna321/QuickClip321/releases/download/";
#[derive(Deserialize)]
struct Manifest {
    schema: u32,
    version: String,
    channel: String,
    file: String,
    size: u64,
    sha256: String,
}
#[derive(Serialize, Clone)]
pub struct ReadyInfo {
    pub version: String,
    pub channel: String,
}
pub struct Staged {
    dir: tempfile::TempDir,
    manifest: Manifest,
}
impl Staged {
    pub fn info(&self) -> ReadyInfo {
        ReadyInfo {
            version: self.manifest.version.clone(),
            channel: self.manifest.channel.clone(),
        }
    }
}
fn command(relative: &str) -> Result<Command, String> {
    let system = std::env::var_os("SystemRoot").ok_or("Windows directory unavailable")?;
    let mut c = Command::new(PathBuf::from(system).join("System32").join(relative));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c.stdin(Stdio::null());
    Ok(c)
}
fn base64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (c.get(1).copied().unwrap_or(0) as u32) << 8
            | c.get(2).copied().unwrap_or(0) as u32;
        s.push(T[(n >> 18) as usize] as char);
        s.push(T[((n >> 12) & 63) as usize] as char);
        s.push(if c.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}
fn ps(body: &str) -> Result<Command, String> {
    let encoded = base64(
        &body
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let mut c = command("WindowsPowerShell/v1.0/powershell.exe")?;
    c.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        &encoded,
    ])
    .env("QC_UPD_PUBLIC", PUBLIC);
    Ok(c)
}
fn fetch(url: &str, path: &Path, max: u64) -> Result<(), String> {
    let status = command("curl.exe")?
        .args([
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
            &max.to_string(),
            "--output",
        ])
        .arg(path)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| "Update download failed")?;
    if !status.success()
        || !std::fs::metadata(path)
            .ok()
            .is_some_and(|m| m.len() > 0 && m.len() <= max)
    {
        return Err("Update asset missing or invalid".into());
    }
    Ok(())
}
fn valid_version(version: &str, channel: &str) -> bool {
    semver::Version::parse(version).ok().is_some_and(|v| {
        v.to_string() == version
            && v.build.is_empty()
            && match channel {
                "stable" => v.pre.is_empty(),
                "nightly" => v.pre.as_str().split('.').any(|p| p == "nightly"),
                _ => false,
            }
    })
}
fn verify(dir: &Path, version: &str, channel: &str) -> Result<Manifest, String> {
    if !valid_version(version, channel) {
        return Err("Invalid signed update version".into());
    }
    let result=ps(&format!("{VERIFY}\n$null=Assert-UpdateManifest $env:QC_UPD_TX $env:QC_UPD_VERSION $env:QC_UPD_CHANNEL"))?.env("QC_UPD_TX",dir).env("QC_UPD_VERSION",version).env("QC_UPD_CHANNEL",channel).stdout(Stdio::null()).stderr(Stdio::null()).status().map_err(|_|"Signature verification unavailable")?;
    if !result.success() {
        return Err("Release signature rejected".into());
    }
    let bytes =
        std::fs::read(dir.join("update-manifest.json")).map_err(|_| "Manifest unavailable")?;
    let m: Manifest = serde_json::from_slice(&bytes).map_err(|_| "Invalid manifest")?;
    if m.schema != 1
        || m.version != version
        || m.channel != channel
        || m.file != format!("QuickClip321-{version}-win-x64.exe")
        || m.size == 0
        || m.size > 100_000_000
        || m.sha256.len() != 64
        || !m.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Manifest identity mismatch".into());
    }
    Ok(m)
}
fn valid_artifact(path: &Path, m: &Manifest) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != m.size {
        return false;
    }
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    h.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        == m.sha256
}
pub fn stage(version: &str, channel: &str) -> Result<Staged, String> {
    if !valid_version(version, channel) {
        return Err("Invalid update version".into());
    }
    let dir = tempfile::Builder::new()
        .prefix("quickclip-signed-")
        .tempdir()
        .map_err(|_| "Cannot stage update")?;
    let base = format!("{ROOT}v{version}/");
    for (name, max) in [
        ("update-manifest.json", 16384),
        ("update-manifest.sig", 2048),
    ] {
        fetch(&(base.clone() + name), &dir.path().join(name), max)?;
    }
    let manifest = verify(dir.path(), version, channel)?;
    let file = dir.path().join(&manifest.file);
    fetch(&(base + &manifest.file), &file, manifest.size)?;
    if !valid_artifact(&file, &manifest) {
        return Err("Release artifact integrity failure".into());
    }
    Ok(Staged { dir, manifest })
}
pub fn install(staged: &Staged, state: &Path) -> Result<(), String> {
    let current = std::env::current_exe().map_err(|_| "Current executable unavailable")?;
    let parent = current.parent().ok_or("Invalid executable location")?;
    if !valid_artifact(
        &staged.dir.path().join(&staged.manifest.file),
        &staged.manifest,
    ) {
        return Err("Staged artifact changed".into());
    }
    let tx = tempfile::Builder::new()
        .prefix(".quickclip-update-")
        .tempdir_in(parent)
        .map_err(|_| "Cannot write beside portable EXE")?;
    for file in [
        "update-manifest.json",
        "update-manifest.sig",
        staged.manifest.file.as_str(),
    ] {
        std::fs::copy(staged.dir.path().join(file), tx.path().join(file))
            .map_err(|_| "Cannot prepare portable replacement")?;
    }
    let _ = verify(
        tx.path(),
        &staged.manifest.version,
        &staged.manifest.channel,
    )?;
    if !valid_artifact(&tx.path().join(&staged.manifest.file), &staged.manifest) {
        return Err("Copied artifact verification failed".into());
    }
    let token = tx
        .path()
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or("Invalid transaction")?
        .to_string();
    let body = format!("{VERIFY}\n{INSTALL}");
    let mut c = ps(&body)?;
    c.env("QC_UPD_TX", tx.path())
        .env("QC_UPD_TARGET", &current)
        .env("QC_UPD_STATE", state)
        .env("QC_UPD_PID", std::process::id().to_string())
        .env("QC_UPD_TOKEN", token)
        .env("QC_UPD_VERSION", &staged.manifest.version)
        .env("QC_UPD_CHANNEL", &staged.manifest.channel)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    c.spawn().map_err(|_| "Cannot start signed update helper")?;
    let _ = tx.keep();
    Ok(())
}
pub fn report_health() -> Result<(), String> {
    let Some(path) = std::env::var_os("QC_UPDATE_HEALTH_PATH").map(PathBuf::from) else {
        return Ok(());
    };
    let token = std::env::var("QC_UPDATE_HEALTH_TOKEN").map_err(|_| "Invalid health token")?;
    let version =
        std::env::var("QC_UPDATE_HEALTH_VERSION").map_err(|_| "Invalid health version")?;
    let exe = std::env::current_exe().map_err(|_| "Invalid executable")?;
    if version != env!("CARGO_PKG_VERSION")
        || path.file_name().and_then(|x| x.to_str()) != Some("healthy.txt")
        || path.parent().and_then(Path::parent) != exe.parent()
        || path
            .parent()
            .and_then(Path::file_name)
            .and_then(|x| x.to_str())
            != Some(token.as_str())
        || !token.starts_with(".quickclip-update-")
    {
        return Err("Invalid update health transaction".into());
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| "Could not report update health")?;
    f.write_all(format!("{version}|{token}").as_bytes())
        .map_err(|_| "Could not report update health")?;
    f.sync_all()
        .map_err(|_| "Could not save update health".to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_identity() {
        assert!(valid_version("3.0.0", "stable"));
        assert!(valid_version("3.1.0-nightly.2", "nightly"));
        assert!(!valid_version("3.1.0-nightly.2", "stable"));
        assert!(!valid_version("../evil", "nightly"));
        assert_eq!(base64(b"abc"), "YWJj");
    }
}
