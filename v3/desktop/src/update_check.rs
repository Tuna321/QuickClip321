// Discovery only. Release metadata is not an authentication mechanism.
// Never execute or download a release executable from this module.
use semver::Version;
use serde::Serialize;
const API: &str = "https://api.github.com/repos/Tuna321/QuickClip321/releases?per_page=30";
const PAGE_SIZE: usize = 30;
const MAX_PAGES: usize = 10;
fn collect_releases(
    mut fetch: impl FnMut(usize) -> Result<Vec<serde_json::Value>, String>,
) -> Result<serde_json::Value, String> {
    let mut all = Vec::new();
    for page in 1..=MAX_PAGES {
        let entries = fetch(page)?;
        let last = entries.len() < PAGE_SIZE;
        all.extend(entries);
        if last {
            return Ok(serde_json::Value::Array(all));
        }
    }
    Err("Release history exceeds discovery limit; check releases manually".into())
}
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
    if !matches!(channel, "stable" | "nightly") {
        return Err("Invalid update channel".into());
    }
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
        if !v.build.is_empty() {
            continue;
        }
        let pre = v.pre.as_str();
        let nightly = pre.split('.').any(|p| p == "nightly");
        // Each channel only selects its own releases. Cross-channel downgrades
        // require an explicit switch.
        let public_beta = pre
            .strip_prefix("beta.")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            && release["prerelease"].as_bool() == Some(true);
        // The stable *track* accepts explicitly signed public betas until GA.
        // Legacy unsigned beta assets remain manual-only.
        let allowed =
            channel == "stable" && pre.is_empty() && release["prerelease"].as_bool() == Some(false)
                || channel == "nightly" && nightly && release["prerelease"].as_bool() == Some(true);
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
        if allowed || (channel == "stable" && public_beta && signed.contains(&v.to_string())) {
            versions.push(v.clone());
        }
        if public_beta {
            betas.push(v);
        }
    }
    let latest = versions
        .iter()
        .filter(|v| signed.contains(&v.to_string()))
        .max()
        .cloned()
        .or_else(|| versions.into_iter().max());
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
    let data = collect_releases(|page| {
        let endpoint = format!("{API}&page={page}");
        let mut cmd = std::process::Command::new(&path);
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
                &endpoint,
            ])
            .output()
            .map_err(|_| "Update check unavailable")?;
        if !output.status.success() || output.stdout.len() > 2_097_152 {
            return Err("Could not check releases; retry later".into());
        }
        serde_json::from_slice(&output.stdout).map_err(|_| "Invalid release response".into())
    })?;
    select(&data, env!("CARGO_PKG_VERSION"), channel)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(v: &str, pre: bool, draft: bool) -> serde_json::Value {
        serde_json::json!({"tag_name":format!("v{v}"),"draft":draft,"prerelease":pre,"assets":[{"name":format!("QuickClip321-{v}-win-x64.exe"),"state":"uploaded","size":1234}]})
    }
    #[test]
    fn stable_release_after_more_than_thirty_nightlies_is_discovered() {
        let stable = release("3.0.0", false, false);
        let nightly = (0..30)
            .map(|n| release(&format!("3.1.0-nightly.{n}"), true, false))
            .collect::<Vec<_>>();
        let data = collect_releases(|page| {
            Ok(if page == 1 {
                nightly.clone()
            } else {
                vec![stable.clone()]
            })
        })
        .unwrap();
        let info = select(&data, "2.9.0", "stable").unwrap();
        assert_eq!(info.latest_in_channel.as_deref(), Some("3.0.0"));
        assert!(collect_releases(|_| Ok(nightly.clone())).is_err());
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
    fn signed_release(v: &str, pre: bool) -> serde_json::Value {
        let mut r = release(v, pre, false);
        let a = r["assets"].as_array_mut().unwrap();
        for name in ["update-manifest.json", "update-manifest.sig"] {
            a.push(serde_json::json!({"name":name,"state":"uploaded"}));
        }
        r
    }
    #[test]
    fn signed_channels_switch_both_directions() {
        let data = serde_json::json!([
            signed_release("3.0.0", false),
            signed_release("3.1.0-nightly.2", true),
            release("9.0.0", false, false),
            release("9.0.0-nightly.1", true, false)
        ]);
        let a = select(&data, "3.1.0-nightly.2", "stable").unwrap();
        assert!(a.switch_available && a.signed_available);
        assert_eq!(a.latest_in_channel.as_deref(), Some("3.0.0"));
        let b = select(&data, "3.0.0", "nightly").unwrap();
        assert!(b.switch_available && b.signed_available);
        assert_eq!(b.latest_in_channel.as_deref(), Some("3.1.0-nightly.2"));
        let c = select(&data, "3.0.0", "stable").unwrap();
        assert!(!c.signed_available && !c.switch_available);
        assert!(select(&data, "3.0.0", "invalid").is_err());
    }
    #[test]
    fn signed_beta_is_stable_track_not_legacy_beta() {
        let data = serde_json::json!([
            release("3.0.0-beta.5", true, false),
            signed_release("3.0.0-beta.6", true),
            signed_release("3.0.0-beta.6.nightly.3", true)
        ]);
        let a = select(&data, "3.0.0-beta.6.nightly.3", "stable").unwrap();
        assert!(a.signed_available && a.switch_available);
        assert_eq!(a.latest_in_channel.as_deref(), Some("3.0.0-beta.6"));
        let b = select(&data, "3.0.0-beta.6", "nightly").unwrap();
        assert!(b.signed_available && b.switch_available);
        assert_eq!(
            b.latest_in_channel.as_deref(),
            Some("3.0.0-beta.6.nightly.3")
        );
    }
    #[test]
    fn same_channel_update_and_downgrade() {
        let data = serde_json::json!([signed_release("3.0.1", false)]);
        let a = select(&data, "3.0.0", "stable").unwrap();
        assert!(a.signed_available && !a.switch_available);
        assert_eq!(a.available.as_deref(), Some("3.0.1"));
        let b = select(&data, "3.0.2", "stable").unwrap();
        assert!(!b.signed_available && b.available.is_none());
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
