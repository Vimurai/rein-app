//! Error types. Every variant names the check that failed (blueprint §Components 2).

use std::fmt;
use std::io;

/// A release failed verification. Nothing outside the extraction directory was written,
/// and the extraction directory itself has been removed.
#[derive(Debug)]
pub enum VerifyError {
    /// `manifest.json` is not valid JSON or does not match schema 1.
    ManifestInvalid(String),
    /// `manifest.json` declares a schema this build does not understand.
    UnsupportedSchema(u64),
    /// This build carries no usable trust root (kill switch or no baked key).
    NoTrustedKey(String),
    /// The manifest's version is not the version that was requested (no downgrade by
    /// serving an older signed release under a newer tag).
    VersionMismatch { requested: String, found: String },
    /// The tarball file changed while it was being verified and extracted.
    TarballChanged,
    /// The release is signed with a key this build does not trust (rein D-153 r1).
    NeedsNewerApp {
        release_key: String,
        app_key: String,
        release_version: String,
    },
    /// The minisign signature over the tarball is invalid.
    BadSignature(String),
    /// The tarball is not a readable gzip'd tar.
    TarballUnreadable(String),
    /// A tarball entry has an unsafe path (`..`, absolute, backslash, ...).
    UnsafePath(String),
    /// A tarball entry is a symlink, hard link, device or other non-regular type.
    UnsupportedEntry { path: String, kind: String },
    /// A `files` path is not under a `layout.mirrored` directory (rein D-155 r2).
    OutsideLayout(String),
    /// The tarball contains a file the manifest does not list.
    Unlisted(String),
    /// The manifest lists a file the tarball does not contain.
    Missing(String),
    /// The tarball contains the same path twice.
    Duplicate(String),
    /// A file's size differs from the manifest.
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    /// A file's sha256 differs from the manifest.
    HashMismatch(String),
    /// The tarball has no `manifest.json`.
    ManifestNotInTarball,
    /// `manifest.json` is not the tarball's first member (rein D-155 r1).
    ManifestNotFirst,
    /// A member the release format never ships (`VERSION`).
    ForbiddenMember(String),
    /// The in-tarball `manifest.json` differs from the one beside the tarball.
    ManifestMismatch,
    /// The extraction directory already exists (refusing to write into it).
    DestinationExists(String),
    /// Local I/O failed while staging.
    Io(String),
}

/// Every string a manifest or tarball supplies is escaped on display: its control bytes
/// never reach the terminal (D-023 r4).
impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::paths::printable(&Unescaped(self).to_string()))
    }
}

struct Unescaped<'a>(&'a VerifyError);

impl fmt::Display for Unescaped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use VerifyError::*;
        match self.0 {
            ManifestInvalid(m) => write!(f, "manifest: invalid ({m})"),
            VersionMismatch { requested, found } => write!(
                f,
                "manifest: version {found} does not match the requested {requested}"
            ),
            TarballChanged => write!(f, "tarball: file changed during verification"),
            UnsupportedSchema(s) => write!(
                f,
                "manifest: schema {s} is not supported by this rein-app (supports 1) — install a newer rein-app"
            ),
            NoTrustedKey(m) => write!(f, "signature: no trusted release key in this build ({m})"),
            NeedsNewerApp {
                release_key,
                app_key,
                release_version,
            } => write!(
                f,
                "rein {release_version} is signed with key {release_key}; this rein-app {} trusts key {app_key} — install a newer rein-app",
                env!("CARGO_PKG_VERSION")
            ),
            BadSignature(m) => write!(f, "signature: invalid ({m})"),
            TarballUnreadable(m) => write!(f, "tarball: unreadable ({m})"),
            UnsafePath(p) => write!(f, "tarball: unsafe path '{p}'"),
            UnsupportedEntry { path, kind } => {
                write!(f, "tarball: unsupported entry type {kind} at '{path}'")
            }
            OutsideLayout(p) => write!(f, "layout: '{p}' is not under a layout.mirrored directory"),
            Unlisted(p) => write!(f, "files: '{p}' is in the tarball but not in the manifest"),
            Missing(p) => write!(f, "files: '{p}' is in the manifest but not in the tarball"),
            Duplicate(p) => write!(f, "tarball: duplicate entry '{p}'"),
            SizeMismatch {
                path,
                expected,
                actual,
            } => {
                write!(f, "files: '{path}' size {actual} != manifest {expected}")
            }
            HashMismatch(p) => write!(f, "files: '{p}' sha256 does not match the manifest"),
            ManifestNotInTarball => write!(f, "manifest: missing from the tarball"),
            ManifestNotFirst => write!(f, "tarball: manifest.json is not the first member"),
            ForbiddenMember(p) => write!(f, "tarball: '{p}' must not be shipped"),
            ManifestMismatch => write!(
                f,
                "manifest: the copy inside the tarball differs from the one beside it"
            ),
            DestinationExists(p) => write!(f, "staging: '{p}' already exists"),
            Io(m) => write!(f, "staging: i/o error ({m})"),
        }
    }
}

impl std::error::Error for VerifyError {}

impl From<io::Error> for VerifyError {
    fn from(e: io::Error) -> Self {
        VerifyError::Io(e.to_string())
    }
}

/// Downloading a release asset failed.
#[derive(Debug)]
pub enum FetchError {
    /// The URL's scheme or host is not on the allow-list.
    HostNotAllowed(String),
    /// The server answered with a non-success status.
    Status { url: String, status: u16 },
    /// The asset exceeds its size cap.
    TooLarge { url: String, cap: u64 },
    /// Network or TLS failure.
    Network(String),
    /// Release metadata (the latest-release answer) is malformed.
    Metadata(String),
    /// Local I/O failed while writing the download.
    Io(String),
}

/// Release metadata from the network reaches this text: escaped like VerifyError (E-29 P3-1).
impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::paths::printable(&UnescapedFetch(self).to_string()))
    }
}

struct UnescapedFetch<'a>(&'a FetchError);

impl fmt::Display for UnescapedFetch<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FetchError::*;
        match self.0 {
            HostNotAllowed(u) => write!(f, "fetch: host not allowed: {u}"),
            Status { url, status } => write!(f, "fetch: {url} answered HTTP {status}"),
            TooLarge { url, cap } => write!(f, "fetch: {url} exceeds the {cap}-byte cap"),
            Network(m) => write!(f, "fetch: network error ({m})"),
            Metadata(m) => write!(f, "fetch: release metadata invalid ({m})"),
            Io(m) => write!(f, "fetch: i/o error ({m})"),
        }
    }
}

impl std::error::Error for FetchError {}

impl From<io::Error> for FetchError {
    fn from(e: io::Error) -> Self {
        FetchError::Io(e.to_string())
    }
}
