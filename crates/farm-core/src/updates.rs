//! Update checks against the project's own GitHub Releases.
//!
//! This is deliberately not a mock: it calls the public GitHub API, compares the
//! published tag with the running version, downloads the real installer asset,
//! verifies its SHA-256 digest when GitHub publishes one, and only then hands the
//! file to the shell for execution.

use crate::error::{CoreError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

const USER_AGENT: &str = "BT-Farm-Updater";
const RELEASES_API: &str = "https://api.github.com/repos";

/// The repository whose Releases page publishes this app's installers.
pub const UPDATE_REPO: &str = "itriedcoding/BT-Farm";

/// Result of asking GitHub for the newest release.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_name: String,
    pub notes: String,
    pub published_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_size: Option<u64>,
    pub digest: Option<String>,
    pub checked_at: DateTime<Utc>,
    /// Human-readable explanation when nothing can be installed automatically.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateDownload {
    pub path: String,
    pub bytes: u64,
    pub verified: bool,
    pub note: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<DateTime<Utc>>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| CoreError::Transport {
            provider: "updater".into(),
            message: e.to_string(),
        })
}

/// `v1.2.3-beta+meta` → (1, 2, 3).
pub fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let trimmed = value.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split(['-', '+']).next()?.trim();
    if core.is_empty() {
        return None;
    }
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn pick_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let uploaded = |a: &&ReleaseAsset| a.state.as_deref().map(|s| s == "uploaded").unwrap_or(true);
    assets
        .iter()
        .filter(uploaded)
        .find(|a| {
            let name = a.name.to_ascii_lowercase();
            name.ends_with(".exe") && name.contains("setup")
        })
        .or_else(|| {
            assets
                .iter()
                .filter(uploaded)
                .find(|a| a.name.to_ascii_lowercase().ends_with(".exe"))
        })
}

/// Asks GitHub for the newest published release of `repo` ("owner/name").
pub async fn check(repo: &str, current_version: &str) -> Result<UpdateInfo> {
    let url = format!("{RELEASES_API}/{repo}/releases/latest");
    let response = client()?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| CoreError::Transport {
            provider: "updater".into(),
            message: format!("could not reach GitHub: {e}"),
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateInfo {
            available: false,
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            release_name: String::new(),
            notes: String::new(),
            published_at: None,
            html_url: format!("https://github.com/{repo}/releases"),
            asset_name: None,
            asset_url: None,
            asset_size: None,
            digest: None,
            checked_at: Utc::now(),
            note: Some("No published release was found for this repository yet.".into()),
        });
    }
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(CoreError::Transport {
            provider: "updater".into(),
            message: "GitHub's unauthenticated rate limit is exhausted. Open the release page to check manually, or try again later.".into(),
        });
    }
    if !status.is_success() {
        return Err(CoreError::Transport {
            provider: "updater".into(),
            message: format!("GitHub answered {status} for {url}"),
        });
    }

    let release: Release = response.json().await.map_err(|e| CoreError::Transport {
        provider: "updater".into(),
        message: format!("unexpected release payload: {e}"),
    })?;

    if release.draft || release.prerelease {
        return Ok(UpdateInfo {
            available: false,
            current_version: current_version.to_string(),
            latest_version: release.tag_name.clone(),
            release_name: release.name.unwrap_or_else(|| release.tag_name.clone()),
            notes: release.body.unwrap_or_default(),
            published_at: release.published_at,
            html_url: release.html_url,
            asset_name: None,
            asset_url: None,
            asset_size: None,
            digest: None,
            checked_at: Utc::now(),
            note: Some("The newest release is a draft or pre-release, so it is not offered as an update.".into()),
        });
    }

    let asset = pick_asset(&release.assets);
    let available = is_newer(&release.tag_name, current_version);
    let note = match (&asset, available) {
        (None, true) => Some("A newer release exists but it has no Windows installer attached.".into()),
        _ => None,
    };
    Ok(UpdateInfo {
        available: available && asset.is_some(),
        current_version: current_version.to_string(),
        latest_version: release.tag_name.clone(),
        release_name: release.name.unwrap_or_else(|| release.tag_name.clone()),
        notes: release.body.unwrap_or_default(),
        published_at: release.published_at,
        html_url: release.html_url,
        asset_name: asset.map(|a| a.name.clone()),
        asset_url: asset.map(|a| a.browser_download_url.clone()),
        asset_size: asset.map(|a| a.size),
        digest: asset.and_then(|a| a.digest.clone()),
        checked_at: Utc::now(),
        note,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Downloads the installer into `dir` and verifies it when a digest is known.
pub async fn download(info: &UpdateInfo, dir: &Path) -> Result<UpdateDownload> {
    let url = info
        .asset_url
        .as_deref()
        .ok_or_else(|| CoreError::invalid("this release has no installer to download"))?;
    let name = info
        .asset_name
        .clone()
        .unwrap_or_else(|| "bt-farm-setup.exe".to_string());
    // Keep only a filename, never a path, from the remote metadata.
    let file_name = std::path::Path::new(&name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "bt-farm-setup.exe".to_string());
    if !file_name.to_ascii_lowercase().ends_with(".exe") {
        return Err(CoreError::invalid(
            "the release asset is not a Windows installer",
        ));
    }

    std::fs::create_dir_all(dir)?;
    let target = dir.join(&file_name);

    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| CoreError::Transport {
            provider: "updater".into(),
            message: format!("download failed: {e}"),
        })?;
    if !response.status().is_success() {
        return Err(CoreError::Transport {
            provider: "updater".into(),
            message: format!("download failed with HTTP {}", response.status()),
        });
    }
    let bytes = response.bytes().await.map_err(|e| CoreError::Transport {
        provider: "updater".into(),
        message: format!("download was interrupted: {e}"),
    })?;
    if bytes.len() < 100_000 {
        return Err(CoreError::Transport {
            provider: "updater".into(),
            message: format!("the downloaded installer is implausibly small ({} bytes)", bytes.len()),
        });
    }

    let mut verified = false;
    let mut note = "Downloaded. The installer was not signed with a published digest.".to_string();
    if let Some(digest) = info.digest.as_deref() {
        let expected = digest
            .strip_prefix("sha256:")
            .unwrap_or(digest)
            .to_ascii_lowercase();
        let actual = sha256_hex(&bytes);
        if expected != actual {
            let _ = std::fs::remove_file(&target);
            return Err(CoreError::invalid(format!(
                "installer checksum mismatch (expected {expected}, got {actual}); the download was discarded"
            )));
        }
        verified = true;
        note = "Downloaded and verified against the release's published SHA-256 digest.".into();
    }

    std::fs::write(&target, &bytes)?;
    Ok(UpdateDownload {
        path: target.to_string_lossy().to_string(),
        bytes: bytes.len() as u64,
        verified,
        note,
    })
}

/// Launches a downloaded installer. Only files inside `allowed_dir` are accepted
/// so a compromised frontend cannot ask the shell to run anything it likes.
pub fn launch_installer(path: &Path, allowed_dir: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|e| CoreError::invalid(format!("installer not found: {e}")))?;
    let root = allowed_dir
        .canonicalize()
        .map_err(|e| CoreError::invalid(format!("update directory unavailable: {e}")))?;
    if !canonical.starts_with(&root) {
        return Err(CoreError::invalid(
            "refusing to run an installer outside the update directory",
        ));
    }
    if !canonical
        .extension()
        .map(|e| e.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
    {
        return Err(CoreError::invalid("only .exe installers can be launched"));
    }

    #[cfg(windows)]
    {
        std::process::Command::new(&canonical)
            .spawn()
            .map_err(|e| CoreError::invalid(format!("could not start the installer: {e}")))?;
    }
    #[cfg(not(windows))]
    {
        return Err(CoreError::unsupported(
            "updater",
            "running the installer",
            "the installer can only be launched on Windows",
        ));
    }
    #[allow(unreachable_code)]
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared_numerically() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.9"), Some((0, 9, 0)));
        assert_eq!(parse_version("2.0.0-beta.1"), Some((2, 0, 0)));
        assert_eq!(parse_version("nightly"), None);

        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(!is_newer("v0.9.9", "1.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
    }

    #[test]
    fn picks_the_setup_asset_over_other_exes() {
        let assets = vec![
            ReleaseAsset {
                name: "portable.exe".into(),
                browser_download_url: "https://example.com/portable.exe".into(),
                size: 10,
                digest: None,
                state: Some("uploaded".into()),
            },
            ReleaseAsset {
                name: "BT Farm_1.0.0_x64-setup.exe".into(),
                browser_download_url: "https://example.com/setup.exe".into(),
                size: 20,
                digest: Some("sha256:abc".into()),
                state: Some("uploaded".into()),
            },
        ];
        let picked = pick_asset(&assets).expect("an exe");
        assert_eq!(picked.name, "BT Farm_1.0.0_x64-setup.exe");
    }

    #[test]
    fn installer_must_stay_inside_the_update_dir() {
        let dir = tempfile::tempdir().unwrap();
        let inside = dir.path().join("setup.exe");
        std::fs::write(&inside, b"x").unwrap();
        assert!(launch_installer(&inside, dir.path()).is_ok() || !cfg!(windows));

        let other = tempfile::tempdir().unwrap();
        let outside = other.path().join("evil.exe");
        std::fs::write(&outside, b"x").unwrap();
        assert!(launch_installer(&outside, dir.path()).is_err());
    }
}
