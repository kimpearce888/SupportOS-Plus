//! Version transparency (R7): compare the running build against the
//! latest published GitHub release.
//!
//! The check is strictly user-initiated — the app never phones home on
//! its own. The repository is private, so an unauthenticated probe may
//! legitimately answer 404; that is surfaced honestly instead of silently
//! reported as "no updates".

use serde::Deserialize;

use crate::error::{Error, Result};

/// The project's public release feed. Unauthenticated access works only
/// while the repository is public; a private repository answers 404 and
/// the UI explains that.
pub const RELEASES_API: &str =
    "https://api.github.com/repos/kimpearce888/SupportOS-Plus/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReleaseInfo {
    /// Tag name as pushed, e.g. "v1.4.0".
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOrdering {
    RunningIsNewer,
    Same,
    RunningIsOlder,
}

/// Same numeric core: a pre-release sorts below its own release, and two
/// pre-releases compare by their dot-separated identifiers (numeric
/// identifiers numerically and lower than alphanumeric ones; a larger
/// identifier set outranks its prefix) — audit item 124: `beta.1` vs
/// `beta.2` (or `-rc.1` vs `-beta.2`) used to compare as Same, so a
/// running pre-release never learned a newer pre-release existed.
fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_ids = a.split('.');
    let mut b_ids = b.split('.');
    loop {
        match (a_ids.next(), b_ids.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            // A larger identifier set outranks its prefix.
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => {
                let xn = x.parse::<i64>().ok();
                let yn = y.parse::<i64>().ok();
                let ord = match (xn, yn) {
                    (Some(xi), Some(yi)) => xi.cmp(&yi),
                    // Numeric identifiers sort below alphanumeric ones.
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => x.cmp(y),
                };
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

#[must_use]
pub fn compare_versions(running: &str, latest: &str) -> VersionOrdering {
    let (a_core, a_pre) = split_version(running);
    let (b_core, b_pre) = split_version(latest);
    for i in 0..a_core.len().max(b_core.len()) {
        let x = a_core.get(i).copied().unwrap_or(0);
        let y = b_core.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Less => return VersionOrdering::RunningIsOlder,
            std::cmp::Ordering::Greater => return VersionOrdering::RunningIsNewer,
            std::cmp::Ordering::Equal => continue,
        }
    }
    match (a_pre, b_pre) {
        (Some(_), None) => VersionOrdering::RunningIsOlder,
        (None, Some(_)) => VersionOrdering::RunningIsNewer,
        (Some(a), Some(b)) => match compare_prerelease(&a, &b) {
            std::cmp::Ordering::Less => VersionOrdering::RunningIsOlder,
            std::cmp::Ordering::Greater => VersionOrdering::RunningIsNewer,
            std::cmp::Ordering::Equal => VersionOrdering::Same,
        },
        (None, None) => VersionOrdering::Same,
    }
}

/// Split `v1.4.0-beta.1+build` into `([1, 4, 0], Some("beta.1"))`.
fn split_version(s: &str) -> (Vec<i64>, Option<String>) {
    let trimmed = s.trim().trim_start_matches(['v', 'V']);
    let no_build = trimmed.split('+').next().unwrap_or(trimmed);
    let (core_str, pre) = match no_build.split_once('-') {
        Some((core, pre)) => (core, Some(pre.to_string())),
        None => (no_build, None),
    };
    let core = core_str
        .split('.')
        .map(|p| p.parse::<i64>().unwrap_or(0))
        .collect::<Vec<_>>();
    (core, pre)
}

impl VersionOrdering {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::RunningIsNewer => "running ahead of the latest published release",
            Self::Same => "up to date",
            Self::RunningIsOlder => "an update is available",
        }
    }
}

/// Probe the release feed once. Pure network call; the caller decides
/// where the result is rendered. A private or unreachable repository
/// surfaces as `Err` with the status in the message — never as "latest =
/// running" (which would be a silent lie).
pub async fn latest_release(client: reqwest::Client) -> Result<ReleaseInfo> {
    let response = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "SupportOS-Plus-version-check")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| Error::Config(format!("release check failed: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        let hint = if status.as_u16() == 404 {
            "the repository is private or has no published release"
        } else {
            "GitHub answered with an error"
        };
        return Err(Error::Config(format!(
            "release check failed: HTTP {status} ({hint})"
        )));
    }
    response
        .json::<ReleaseInfo>()
        .await
        .map_err(|e| Error::Config(format!("release check returned an unreadable body: {e}")))
}

/// Blocking wrapper for UI worker threads: builds its own short-lived
/// runtime and client, so callers stay sync (the settings page spawns a
/// detached thread and polls a shared slot).
pub fn latest_release_blocking() -> Result<ReleaseInfo> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::Config(format!("release check runtime: {e}")))?;
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| Error::Config(format!("release check client: {e}")))?;
    runtime.block_on(latest_release(client))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_handles_v_prefixes_and_suffixes() {
        assert_eq!(compare_versions("1.4.0", "v1.4.0"), VersionOrdering::Same);
        assert_eq!(
            compare_versions("v1.4.0", "v1.4.1"),
            VersionOrdering::RunningIsOlder
        );
        assert_eq!(
            compare_versions("1.5.0", "v1.4.9"),
            VersionOrdering::RunningIsNewer
        );
        // Pre-release sorts before its release.
        assert_eq!(
            compare_versions("1.4.0-beta.1", "1.4.0"),
            VersionOrdering::RunningIsOlder
        );
        // Missing segments read as zero: 1.4 == 1.4.0.
        assert_eq!(compare_versions("1.4", "v1.4.0"), VersionOrdering::Same);
    }
}

#[cfg(test)]
mod batch31_tests {
    use super::*;

    /// Audit item 124: two pre-releases of the same core compared as Same
    /// — a running `1.4.0-beta.1` never learned `1.4.0-beta.2` existed.
    /// Pre-release identifiers compare per semver now.
    #[test]
    fn pre_release_versions_order_against_each_other() {
        use VersionOrdering::*;
        assert_eq!(
            compare_versions("1.4.0-beta.1", "1.4.0-beta.2"),
            RunningIsOlder
        );
        assert_eq!(
            compare_versions("1.4.0-beta.2", "1.4.0-beta.1"),
            RunningIsNewer
        );
        assert_eq!(compare_versions("1.4.0-beta.1", "1.4.0-beta.1"), Same);
        // A larger identifier set outranks its prefix.
        assert_eq!(
            compare_versions("1.4.0-beta", "1.4.0-beta.1"),
            RunningIsOlder
        );
        // Numeric identifiers sort below alphanumeric ones.
        assert_eq!(compare_versions("1.4.0-1", "1.4.0-alpha"), RunningIsOlder);
        // Alphanumeric identifiers compare lexically.
        assert_eq!(
            compare_versions("1.4.0-alpha.2", "1.4.0-beta.1"),
            RunningIsOlder
        );
        // A pre-release still sorts below its own release (unchanged).
        assert_eq!(compare_versions("1.4.0-beta.1", "1.4.0"), RunningIsOlder);
        assert_eq!(compare_versions("1.4.0", "1.4.0-beta.1"), RunningIsNewer);
        assert_eq!(compare_versions("1.4.0", "1.4.0"), Same);
        assert_eq!(compare_versions("1.5.0", "1.4.0-beta.9"), RunningIsNewer);
    }
}
