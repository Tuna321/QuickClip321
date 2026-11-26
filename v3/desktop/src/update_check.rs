// Discovery only. Release metadata is not an authentication mechanism.
// Never execute or download a release executable from this module.
use semver::Version;
use serde::Serialize;
const API: &str = "https://api.github.com/repos/Tuna321/QuickClip321/releases?per_page=30";
#[derive(Clone, Debug, Serialize)]
pub struct UpdateInfo {
    pub installed: String,
    pub channel: String,
    pub available: Option<String>,
    pub latest_in_channel: Option<String>,
    pub public_beta: Option<String>,
    pub switch_available: bool,
    pub signed_available: bool,
    pub state: String,
}
pub fn select(
    data: &serde_json::Value,
    current: &str,
    channel: &str,
) -> Result<UpdateInfo, String> {
    let installed = Version::parse(current).map_err(|_| "Invalid local version")?;
    let list = data.as_array().ok_or("Invalid release metadata")?;
    let mut versions = Vec::new();
    let mut betas = Vec::new();
    let mut signed = std::collections::HashSet::new();
    for release in list {
        if release["draft"].as_bool() != Some(false) {
            continue;
        }
        let Some(tag) = release["tag_name"]
            .as_str()
            .and_then(|s| s.strip_prefix('v'))
        else {
            continue;
        };
        let Ok(v) = Version::parse(tag) else { continue };
        let pre = v.pre.as_str();
        let nightly = pre.split('.').any(|p| p == "nightly");
        // Stable never selects nightly or other prereleases. Nightly can select
        // a final stable release, but only if strictly newer than the installed build.
        let allowed =
            channel == "stable" && pre.is_empty() && release["prerelease"].as_bool() == Some(false)
                || channel == "nightly" && nightly && release["prerelease"].as_bool() == Some(true);
        let public_beta =
            pre.starts_with("beta.") && !nightly && release["prerelease"].as_bool() == Some(true);
        let expected = format!("QuickClip321-{v}-win-x64.exe");
        if !release["assets"].as_array().is_some_and(|a| {
            a.iter().any(|x| {
                x["name"].as_str() == Some(expected.as_str())
                    && x["state"].as_str() == Some("uploaded")
                    && x["size"].as_u64().is_some_and(|n| n > 0 && n < 100_000_000)
            })
        }) {
            continue;
        }
        if release["assets"].as_array().is_some_and(|a| {
            ["update-manifest.json", "update-manifest.sig"]
                .iter()
                .all(|name| {
                    a.iter().any(|x| {
                        x["name"].as_str() == Some(*name) && x["state"].as_str() == Some("uploaded")
                    })
                })
        }) {
            signed.insert(v.to_string());
        }
        if allowed {
            versions.push(v.clone());
        }
        if public_beta {
            betas.push(v);
        }
    }
    let latest = versions.into_iter().max();
    let installed_nightly = installed.pre.as_str().split('.').any(|p| p == "nightly");
    let switching = if channel == "stable" {
        installed_nightly
    } else {
        !installed_nightly
    };
    let switch_available = switching && latest.as_ref().is_some_and(|v| v != &installed);
    let available = latest
        .as_ref()
        .filter(|v| **v > installed)
        .map(|v| v.to_string());
    let latest_in_channel = latest.map(|v| v.to_string());
    let public_beta = betas.into_iter().max().map(|v| v.to_string());
    let signed_available = latest_in_channel
        .as_ref()
        .is_some_and(|v| signed.contains(v))
        && (available.is_some() || switch_available);
    Ok(UpdateInfo {
        installed: current.into(),
        channel: channel.into(),
        state: if signed_available {
            "Signed update available"
        } else if switch_available {
            "Selected channel has a different release; signed installation not enabled"
        } else if latest_in_channel.is_none() {
            "No published release on selected channel"
        } else if available.is_some() {
            "Release available; signed installation not enabled"
        } else {
            "No newer release on selected channel"
        }
        .into(),
        available,
        latest_in_channel,
        public_beta,
        switch_available,
        signed_available,
    })
}
pub fn check(channel: &str) -> Result<UpdateInfo, String> {
    if !matches!(channel, "stable" | "nightly") {
        return Err("Invalid update channel".into());
    }
    // System curl; fixed HTTPS endpoint, no user data, credentials or redirects.
    let path = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").ok_or("Windows directory unavailable")?,
    )
    .join("System32")
    .join("curl.exe");
    let mut cmd = std::process::Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd
        .args([
            "-q",
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--connect-timeout",
            "5",
            "--max-time",
            "15",
            "--max-filesize",
            "2097152",
            "--header",
            "Accept: application/vnd.github+json",
            "--user-agent",
            "QuickClip321-update-check",
            API,
        ])
        .output()
        .map_err(|_| "Update check unavailable")?;
    if !output.status.success() || output.stdout.len() > 2_097_152 {
        return Err("Could not check releases; retry later".into());
    }
    let data = serde_json::from_slice(&output.stdout).map_err(|_| "Invalid release response")?;
    select(&data, env!("CARGO_PKG_VERSION"), channel)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(v: &str, pre: bool, draft: bool) -> serde_json::Value {
        serde_json::json!({"tag_name":format!("v{v}"),"draft":draft,"prerelease":pre,"assets":[{"name":format!("QuickClip321-{v}-win-x64.exe"),"state":"uploaded","size":1234}]})
    }
    #[test]
    fn channels_and_downgrade() {
        let data = serde_json::json!([
            release("3.0.0", false, false),
            release("3.1.0-nightly.10", true, false),
            release("3.1.0-nightly.2", true, false),
            release("9.0.0", false, true),
            release("4.0.0-beta.1", true, false)
        ]);
        assert_eq!(
            select(&data, "3.0.0-beta.5", "stable")
                .unwrap()
                .available
                .as_deref(),
            Some("3.0.0")
        );
        assert_eq!(
            select(&data, "3.0.0-beta.5", "nightly")
                .unwrap()
                .available
                .as_deref(),
            Some("3.1.0-nightly.10")
        );
        assert!(select(&data, "4.0.0", "nightly")
            .unwrap()
            .available
            .is_none());
    }
    #[test]
    fn explicit_channel_switch_and_missing_channels() {
        let data = serde_json::json!([
            release("3.0.0", false, false),
            release("3.1.0-nightly.2", true, false)
        ]);
        let i = select(&data, "3.1.0-nightly.2", "stable").unwrap();
        assert!(i.switch_available);
        assert_eq!(i.latest_in_channel.as_deref(), Some("3.0.0"));
        assert!(i.available.is_none());
        let i = select(
            &serde_json::json!([release("3.0.0-beta.5", true, false)]),
            "3.1.0-nightly.2",
            "stable",
        )
        .unwrap();
        assert!(i.latest_in_channel.is_none());
        assert_eq!(i.public_beta.as_deref(), Some("3.0.0-beta.5"));
    }
    #[test]
    fn rejects_wrong_asset() {
        let mut r = release("3.0.0", false, false);
        r["assets"][0]["name"] = serde_json::json!("unrelated.exe");
        assert!(select(&serde_json::json!([r]), "2.0.0", "stable")
            .unwrap()
            .available
            .is_none());
    }
}
