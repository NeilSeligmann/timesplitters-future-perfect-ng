//! Update check against GitHub Releases (T4333). Pure logic: the GUI fetches the JSON, downloads the chosen
//! asset and its `.minisig`, verifies the signature against the embedded public key and swaps the binary.
//! Stable follows full releases, Nightly also accepts pre-releases. Drafts are never offered.
use crate::Result;
use crate::config::Channel;
use serde_json::Value;

pub const RELEASES_API: &str = "https://api.github.com/repos/NeilSeligmann/timesplitters-future-perfect-ng/releases?per_page=20";

/// Release asset base name for the running platform, matching the CI matrix names.
pub fn platform_asset() -> Option<String> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return None,
    };
    Some(format!(
        "tsfp-launcher-{os}-{arch}{}",
        std::env::consts::EXE_SUFFIX
    ))
}

/// `major.minor.patch` with an optional `-pre` suffix; a leading `v` is accepted.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub core: (u64, u64, u64),
    /// Sorts pre-releases before the release (`None` is greater than any `Some`).
    pub release: bool,
    pub pre: String,
}

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let t = text.trim().trim_start_matches('v');
        let (core, pre) = t.split_once('-').unwrap_or((t, ""));
        let mut n = core.split('.').map(|p| p.parse::<u64>().ok());
        let v = (n.next()??, n.next()??, n.next()??);
        if n.next().is_some() {
            return None;
        }
        Some(Version {
            core: v,
            release: pre.is_empty(),
            pre: pre.to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub version: String,
    pub notes: String,
    pub asset_url: String,
    pub signature_url: String,
}

/// Newest release on `channel` that is newer than `current` and ships both the platform asset and its
/// `.minisig`. Releases without a signature are skipped, never offered.
pub fn newest(json: &str, channel: Channel, current: &str, asset: &str) -> Result<Option<Offer>> {
    let current = Version::parse(current).ok_or(format!("bad current version {current:?}"))?;
    let releases: Value = serde_json::from_str(json).map_err(|e| format!("releases: {e}"))?;
    let list = releases.as_array().ok_or("releases: expected an array")?;
    let mut best: Option<(Version, Offer)> = None;
    for r in list {
        let flag = |k: &str| r.get(k).and_then(Value::as_bool).unwrap_or(false);
        if flag("draft") || (flag("prerelease") && channel == Channel::Stable) {
            continue;
        }
        let Some(tag) = r.get("tag_name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = Version::parse(tag) else {
            continue;
        };
        if version <= current || best.as_ref().is_some_and(|(b, _)| version <= *b) {
            continue;
        }
        let url = |name: &str| {
            r.get("assets")?.as_array()?.iter().find_map(|a| {
                (a.get("name")?.as_str()? == name)
                    .then(|| a.get("browser_download_url")?.as_str().map(String::from))?
            })
        };
        let (Some(asset_url), Some(signature_url)) = (url(asset), url(&format!("{asset}.minisig")))
        else {
            continue;
        };
        let notes = r
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        best = Some((
            version,
            Offer {
                version: tag.to_string(),
                notes,
                asset_url,
                signature_url,
            },
        ));
    }
    Ok(best.map(|(_, o)| o))
}
