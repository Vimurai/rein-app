//! Newest `rein` release lookup for `rein-app update` (blueprint §Components 7).
//!
//! The answer is unsigned metadata: it only chooses which version to fetch. The fetched
//! release is then verified and bound to that version (`verify_release(requested)`), and
//! the updater offers only versions newer than the installed one, so a forged answer can
//! at worst name a version that fails verification — never install an older one.

use semver::Version;

use crate::error::FetchError;
use crate::fetch::Http;

/// GitHub's "latest release" endpoint for rein (excludes drafts and prereleases).
pub const REIN_LATEST_API: &str = "https://api.github.com/repos/Vimurai/rein/releases/latest";

/// Upper bound on the API response.
pub const MAX_LATEST_BYTES: u64 = 1024 * 1024;

/// The newest published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestRelease {
    pub version: Version,
    /// Release notes as published (untrusted text; sanitise before printing).
    pub notes: String,
}

/// Ask `api_url` for the newest release.
pub fn latest_release(http: &dyn Http, api_url: &str) -> Result<LatestRelease, FetchError> {
    let mut body = Vec::new();
    http.download(api_url, MAX_LATEST_BYTES, &mut body)?;
    parse_latest(&body)
}

/// Parse the GitHub release JSON: `tag_name` (`vX.Y.Z`) and `body`.
pub fn parse_latest(json: &[u8]) -> Result<LatestRelease, FetchError> {
    let bad = |m: String| FetchError::Metadata(m);
    let v: serde_json::Value = serde_json::from_slice(json).map_err(|e| bad(e.to_string()))?;
    let tag = v
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| bad("no tag_name".into()))?;
    let raw = tag.strip_prefix('v').unwrap_or(tag);
    let version = Version::parse(raw).map_err(|e| bad(format!("tag '{tag}': {e}")))?;
    let notes = v
        .get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(LatestRelease { version, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_and_notes_and_rejects_non_semver_tags() {
        let r = parse_latest(br#"{"tag_name":"v0.6.0","body":"- fix\n"}"#).unwrap();
        assert_eq!(r.version, Version::new(0, 6, 0));
        assert_eq!(r.notes, "- fix\n");
        assert_eq!(
            parse_latest(br#"{"tag_name":"0.6.1"}"#).unwrap().version,
            Version::new(0, 6, 1)
        );
        for bad in [&br#"{"tag_name":"latest"}"#[..], br#"{}"#, b"not json"] {
            assert!(matches!(parse_latest(bad), Err(FetchError::Metadata(_))));
        }
    }
}
