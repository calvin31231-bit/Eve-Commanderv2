//! Update check (Phase 7): compare the running version against the latest
//! published release and tell the user when a newer one exists.
//!
//! This is **notify-only** — it never downloads or installs (signed auto-update
//! is an ops/release concern, and silent self-update on a tool that reads your
//! ESI data is a trust risk). The version comparison is pure and unit-tested;
//! the release lookup is a single GET against a configurable releases endpoint
//! (GitHub-releases-shaped), best-effort.

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Parse a `MAJOR.MINOR.PATCH` version (ignoring a leading `v` and any
/// pre-release/build suffix) into a comparable tuple. Unparseable parts are 0.
fn parse_semver(v: &str) -> (u64, u64, u64) {
    let core = v.trim().trim_start_matches('v');
    // Drop pre-release / build metadata (`-rc1`, `+build`) before splitting.
    let core = core.split(['-', '+']).next().unwrap_or("");
    let mut it = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// True when `latest` is a strictly newer version than `current`. Pure.
pub fn is_newer(current: &str, latest: &str) -> bool {
    parse_semver(latest) > parse_semver(current)
}

/// The outcome of an update check, surfaced to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
    /// Where to get it (release page URL), when known.
    pub url: String,
}

/// One GitHub-releases-shaped record (only the fields we read).
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: Option<String>,
    name: Option<String>,
    html_url: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// Check `releases_url` (a GitHub `/releases` API URL or any endpoint returning
/// that JSON shape) for the newest non-draft, non-prerelease version and compare
/// it to `current`. Best-effort: a network/parse failure yields "no update"
/// rather than an error so a launch check never disrupts startup.
pub async fn check(
    http: &reqwest::Client,
    user_agent: &str,
    releases_url: &str,
    current: &str,
) -> Result<UpdateStatus> {
    let none = || UpdateStatus {
        current: current.to_string(),
        latest: current.to_string(),
        update_available: false,
        url: String::new(),
    };
    let resp = match http
        .get(releases_url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => return Ok(none()),
    };
    let releases: Vec<Release> = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Ok(none()),
    };
    let Some(latest) = pick_latest(&releases) else {
        return Ok(none());
    };
    let tag = latest.tag_name.clone().or_else(|| latest.name.clone()).unwrap_or_default();
    Ok(UpdateStatus {
        current: current.to_string(),
        update_available: is_newer(current, &tag),
        latest: tag,
        url: latest.html_url.clone().unwrap_or_default(),
    })
}

/// Pick the highest-version stable release from a list. Pure.
fn pick_latest(releases: &[Release]) -> Option<&Release> {
    releases
        .iter()
        .filter(|r| !r.draft && !r.prerelease)
        .max_by(|a, b| {
            let va = parse_semver(a.tag_name.as_deref().or(a.name.as_deref()).unwrap_or(""));
            let vb = parse_semver(b.tag_name.as_deref().or(b.name.as_deref()).unwrap_or(""));
            va.cmp(&vb)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare_handles_prefix_and_suffix() {
        assert!(is_newer("1.2.3", "1.2.4"));
        assert!(is_newer("1.2.3", "v1.3.0"));
        assert!(is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("2.0.0", "1.9.9"));
        // Pre-release/build metadata is ignored for the core comparison.
        assert!(!is_newer("1.2.3", "1.2.3-rc1"));
        assert!(is_newer("1.2.3", "1.2.4-rc1"));
    }

    #[test]
    fn picks_highest_stable_release() {
        let rel = |tag: &str, pre: bool, draft: bool| Release {
            tag_name: Some(tag.into()),
            name: None,
            html_url: Some(format!("https://x/{tag}")),
            prerelease: pre,
            draft,
        };
        let releases = vec![
            rel("v1.0.0", false, false),
            rel("v1.2.0", false, false),
            rel("v1.3.0", true, false),  // prerelease — skipped
            rel("v1.4.0", false, true),  // draft — skipped
        ];
        let latest = pick_latest(&releases).unwrap();
        assert_eq!(latest.tag_name.as_deref(), Some("v1.2.0"));
    }
}
