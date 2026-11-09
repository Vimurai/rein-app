//! `Fetcher` (blueprint §Components 1, §Security trust boundary 1): download the three
//! assets of a tagged `rein` release. HTTPS only to an allow-listed host set, size caps,
//! timeouts. Everything downloaded is untrusted until `verify_release` passes.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;
use semver::Version;

use crate::error::FetchError;
use crate::manifest::MAX_MANIFEST_BYTES;
use crate::signature::MAX_SIGNATURE_BYTES;
use crate::verify::ReleaseAssets;

/// Default cap on the tarball (blueprint: 200 MB).
pub const MAX_TARBALL_BYTES: u64 = 200 * 1024 * 1024;

/// Downloads one URL into `out`, refusing more than `cap` bytes.
pub trait Http {
    fn download(&self, url: &str, cap: u64, out: &mut dyn Write) -> Result<u64, FetchError>;
}

/// Which URLs may be contacted, including redirect targets.
#[derive(Debug, Clone)]
pub struct HostPolicy {
    https_hosts: Vec<String>,
    allow_loopback_http: bool,
}

impl HostPolicy {
    /// GitHub releases: `github.com`, which redirects asset downloads to its two
    /// asset CDNs, and `api.github.com` for the latest-release lookup (metadata only,
    /// see `latest.rs`). Nothing else, HTTPS only.
    pub fn github() -> Self {
        Self {
            https_hosts: [
                "github.com",
                "api.github.com",
                "objects.githubusercontent.com",
                "release-assets.githubusercontent.com",
            ]
            .map(String::from)
            .to_vec(),
            allow_loopback_http: false,
        }
    }

    /// Plain HTTP to 127.0.0.1/localhost only — for the local fixture server in tests.
    pub fn loopback_only() -> Self {
        Self {
            https_hosts: Vec::new(),
            allow_loopback_http: true,
        }
    }

    /// Is `url` allowed?
    pub fn allows(&self, url: &Url) -> bool {
        let host = url.host_str().unwrap_or_default();
        match url.scheme() {
            "https" => self.https_hosts.iter().any(|h| h == host),
            "http" => {
                self.allow_loopback_http && matches!(host, "127.0.0.1" | "localhost" | "[::1]")
            }
            _ => false,
        }
    }
}

/// Production `Http`: blocking reqwest over rustls, redirects checked against the policy.
pub struct ReqwestHttp {
    client: reqwest::blocking::Client,
    policy: Arc<HostPolicy>,
}

impl ReqwestHttp {
    pub fn new(policy: HostPolicy) -> Result<Self, FetchError> {
        let policy = Arc::new(policy);
        let redirect_policy = Arc::clone(&policy);
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("too many redirects")
                } else if redirect_policy.allows(attempt.url()) {
                    attempt.follow()
                } else {
                    let msg = format!("redirect to disallowed host {}", attempt.url());
                    attempt.error(msg)
                }
            }))
            // GitHub's API refuses requests without a User-Agent; a generic client name.
            .user_agent(concat!("rein-app/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(15 * 60))
            .build()
            .map_err(|e| FetchError::Network(error_chain(&e)))?;
        Ok(Self { client, policy })
    }
}

impl Http for ReqwestHttp {
    fn download(&self, url: &str, cap: u64, out: &mut dyn Write) -> Result<u64, FetchError> {
        let parsed = Url::parse(url).map_err(|_| FetchError::HostNotAllowed(url.to_owned()))?;
        if !self.policy.allows(&parsed) {
            return Err(FetchError::HostNotAllowed(url.to_owned()));
        }
        let mut resp = self
            .client
            .get(parsed)
            .send()
            .map_err(|e| FetchError::Network(error_chain(&e)))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(FetchError::Status {
                url: url.to_owned(),
                status: status.as_u16(),
            });
        }
        if resp.content_length().is_some_and(|len| len > cap) {
            return Err(FetchError::TooLarge {
                url: url.to_owned(),
                cap,
            });
        }
        copy_capped(&mut resp, out, cap, url)
    }
}

/// `e` and its sources, joined — reqwest's top-level message omits the cause.
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut msg = e.to_string();
    let mut src = e.source();
    while let Some(s) = src {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        src = s.source();
    }
    msg
}

/// Copy at most `cap` bytes; error if the source has more.
pub fn copy_capped(
    src: &mut dyn Read,
    out: &mut dyn Write,
    cap: u64,
    url: &str,
) -> Result<u64, FetchError> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = src
            .read(&mut buf)
            .map_err(|e| FetchError::Network(e.to_string()))?;
        if n == 0 {
            return Ok(total);
        }
        total += n as u64;
        if total > cap {
            return Err(FetchError::TooLarge {
                url: url.to_owned(),
                cap,
            });
        }
        out.write_all(&buf[..n])?;
    }
}

/// Where releases live.
#[derive(Debug, Clone)]
pub struct ReleaseSource {
    /// Base of `<base>/v<version>/<asset>`.
    pub base_url: String,
}

impl ReleaseSource {
    /// `https://github.com/Vimurai/rein/releases/download`
    pub fn rein_github() -> Self {
        Self {
            base_url: "https://github.com/Vimurai/rein/releases/download".into(),
        }
    }

    fn asset_url(&self, version: &Version, name: &str) -> String {
        format!("{}/v{version}/{name}", self.base_url.trim_end_matches('/'))
    }
}

/// Asset file names of one release: tarball, signature, manifest.
pub fn asset_names(version: &Version) -> [String; 3] {
    let tarball = format!("rein-{version}.tar.gz");
    let signature = format!("{tarball}.minisig");
    [tarball, signature, "manifest.json".into()]
}

/// Download the three assets of `version` into `dir` (which must exist and be private to
/// this run). On failure every file written so far is removed.
pub fn fetch_release(
    http: &dyn Http,
    source: &ReleaseSource,
    version: &Version,
    dir: &Path,
) -> Result<ReleaseAssets, FetchError> {
    let [tarball, signature, manifest] = asset_names(version);
    let plan = [
        (&tarball, MAX_TARBALL_BYTES),
        (&signature, MAX_SIGNATURE_BYTES),
        (&manifest, MAX_MANIFEST_BYTES),
    ];
    let mut written = Vec::new();
    for (name, cap) in plan {
        let path = dir.join(name);
        // Only files this run created are ever cleaned up (security review E-3 P2-2).
        let result = File::create_new(&path)
            .map_err(FetchError::from)
            .and_then(|mut f| {
                written.push(path.clone());
                http.download(&source.asset_url(version, name), cap, &mut f)
            });
        if let Err(e) = result {
            for p in &written {
                let _ = fs::remove_file(p);
            }
            return Err(e);
        }
    }
    Ok(ReleaseAssets {
        tarball: dir.join(&tarball),
        signature: dir.join(&signature),
        manifest: dir.join(&manifest),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_policy_allows_only_https_release_hosts() {
        let p = HostPolicy::github();
        let ok = |u: &str| p.allows(&Url::parse(u).unwrap());
        assert!(ok(
            "https://github.com/Vimurai/rein/releases/download/v1.0.0/x"
        ));
        assert!(ok("https://objects.githubusercontent.com/x"));
        assert!(ok(crate::latest::REIN_LATEST_API));
        assert!(!ok("http://github.com/x"));
        assert!(!ok("https://evil.example/x"));
        assert!(!ok("https://github.com.evil.example/x"));
        assert!(!ok("http://127.0.0.1:8080/x"));
    }

    #[test]
    fn loopback_policy_allows_only_local_http() {
        let p = HostPolicy::loopback_only();
        assert!(p.allows(&Url::parse("http://127.0.0.1:1234/x").unwrap()));
        assert!(!p.allows(&Url::parse("http://10.0.0.1/x").unwrap()));
        assert!(!p.allows(&Url::parse("https://github.com/x").unwrap()));
    }

    #[test]
    fn copy_capped_refuses_oversize_bodies() {
        let mut out = Vec::new();
        assert_eq!(copy_capped(&mut &b"abc"[..], &mut out, 3, "u").unwrap(), 3);
        assert!(matches!(
            copy_capped(&mut &b"abcd"[..], &mut Vec::new(), 3, "u"),
            Err(FetchError::TooLarge { .. })
        ));
    }

    #[test]
    fn asset_urls_follow_the_release_layout() {
        let v = Version::new(0, 5, 0);
        let names = asset_names(&v);
        assert_eq!(
            names,
            [
                "rein-0.5.0.tar.gz",
                "rein-0.5.0.tar.gz.minisig",
                "manifest.json"
            ]
        );
        assert_eq!(
            ReleaseSource::rein_github().asset_url(&v, &names[0]),
            "https://github.com/Vimurai/rein/releases/download/v0.5.0/rein-0.5.0.tar.gz"
        );
    }
}
