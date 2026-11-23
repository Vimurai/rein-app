//! The window's own update (updates.md §Components 3, §Data Model, §API, §Security; D-066 and
//! its addendum 1; E-86).
//!
//! Trust is a baked rein-app minisign key — not rein's. Two signatures, both by that key:
//! `desktop-latest.json.minisig` over the document's raw bytes, checked before the document
//! is parsed (security review of E-86, Architect ruling 2026-11-23: an unsigned document
//! could name an older signed installer as a newer version), and the installer's own
//! signature, checked before it starts, with its sha256 compared to the signed document.
//! TLS alone is never the proof.
//!
//! In place on Windows only (the per-user NSIS installer); Linux and macOS are told the
//! package-manager line and download nothing. The order of an install is fixed by D-066
//! addendum 1: download → verify the signature → compare sha256 → everything an ordinary quit
//! does → start the installer detached with the constant argv `/P /UPDATE /R` → exit with no
//! work after the start. Any failed step deletes the file, starts nothing and is one [`Step`].

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rein_release::{FetchError, HostPolicy, Http, MinisignVerifier, Rejecting, SignatureVerifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The one place the window looks for a newer window (updates.md §Components 3 "Check").
pub const LATEST_URL: &str =
    "https://github.com/Vimurai/rein-app/releases/latest/download/desktop-latest.json";
pub const MAX_DOC_BYTES: u64 = 64 * 1024;
pub const MAX_NOTES_BYTES: usize = 4 * 1024;
pub const MAX_SIG_BYTES: usize = 4 * 1024;
pub const MAX_URL_BYTES: usize = 2048;
pub const MAX_INSTALLER_BYTES: u64 = 200 * 1024 * 1024;
/// The check's limit (D-066 addendum 1 r4).
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
/// The installer's download: the CLI's limit for a whole release.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// The installer's argv (D-066 addendum 1 r1): passive, update, relaunch. Nothing from the
/// document reaches a command line.
#[cfg_attr(not(any(windows, feature = "update-test")), allow(dead_code))] // tests read it
pub const INSTALLER_ARGS: [&str; 3] = ["/P", "/UPDATE", "/R"];
/// Every downloaded installer is `<cache>/update/rein-app-update-<run>.exe`.
pub const FILE_PREFIX: &str = "rein-app-update-";
/// The document's signature is `<document url>` + this.
pub const DOC_SIG_SUFFIX: &str = ".minisig";

/// Where the document's installer may live (updates.md §Data Model). Redirects are checked
/// separately by the download's host policy.
pub const ASSET_PREFIXES: &[&str] = &[
    "https://github.com/Vimurai/rein-app/releases/download/",
    "https://objects.githubusercontent.com/",
];

/// The rein-app update key (`minisign.pub` text). None until the release-side task (E-88)
/// bakes the operator's key: until then nothing verifies and no update is offered in place.
pub const BAKED_APP_KEY: Option<&str> = None;

/// The package-manager line for a window installed by Linux / macOS packages (component 3:
/// "the sheet shows the one command to run").
pub const COMMAND_MACOS: &str = "brew upgrade rein-app";
pub const COMMAND_LINUX: &str = "sudo apt install --only-upgrade rein-app";

/// A step of the install that failed (updates.md §Data Model `Step`); one fixed sentence each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Fetch,
    TooLarge,
    Signature,
    Checksum,
    Start,
}

impl Step {
    pub fn sentence(self) -> &'static str {
        match self {
            Step::Fetch => "The update could not be downloaded.",
            Step::TooLarge => "The update is larger than its release says. Nothing was installed.",
            Step::Signature => "The update's signature did not verify. Nothing was installed.",
            Step::Checksum => "The update's checksum did not match. Nothing was installed.",
            Step::Start => "The installer could not be started.",
        }
    }

    pub fn kind(self) -> &'static str {
        match self {
            Step::Fetch => "fetch",
            Step::TooLarge => "too_large",
            Step::Signature => "signature",
            Step::Checksum => "checksum",
            Step::Start => "start",
        }
    }
}

/// `job` of `UpdatesState`: `"idle"`, `{"downloading":{"done","total"}}`, `"verifying"`,
/// `"restarting"` or `{"failed":"<step>"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Job {
    Idle,
    Downloading { done: u64, total: u64 },
    Verifying,
    Restarting,
    Failed(Step),
}

impl Job {
    /// A job holds the installer: another start is refused.
    pub fn running(self) -> bool {
        matches!(
            self,
            Job::Downloading { .. } | Job::Verifying | Job::Restarting
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDoc {
    schema: u32,
    version: String,
    key_id: String,
    notes: String,
    windows_x64: RawAsset,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAsset {
    url: String,
    size: u64,
    sha256: String,
    sig: String,
}

/// `desktop-latest.json`, schema 1, after the strict parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    pub version: semver::Version,
    pub key_id: String,
    pub notes: String,
    pub url: String,
    pub size: u64,
    pub sha256: [u8; 32],
    /// The installer's `.minisig` text.
    pub sig: String,
}

/// Parse `bytes` strictly: schema 1, no unknown field, every value bounded. `prefixes` are
/// the allowed installer URL prefixes ([`ASSET_PREFIXES`] outside the test build).
pub fn parse_doc(bytes: &[u8], prefixes: &[&str]) -> Result<Doc, String> {
    if bytes.len() as u64 > MAX_DOC_BYTES {
        return Err("document over 64 KiB".into());
    }
    let raw: RawDoc = serde_json::from_slice(bytes).map_err(|e| format!("document: {e}"))?;
    if raw.schema != 1 {
        return Err(format!("schema {} is not 1", raw.schema));
    }
    let version = crate::updates::parse_version(raw.version.as_bytes())
        .filter(|v| *v == raw.version)
        .and_then(|v| semver::Version::parse(&v).ok())
        .ok_or("version is not a semantic version")?;
    let key_id_ok = raw.key_id.len() == 16
        && raw
            .key_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b));
    if !key_id_ok {
        return Err("key_id is not 16 upper-case hex digits".into());
    }
    if raw.notes.len() > MAX_NOTES_BYTES {
        return Err("notes over 4 KiB".into());
    }
    if raw
        .notes
        .chars()
        .any(|c| (c.is_control() && c != '\n') || is_direction_mark(c))
    {
        return Err("notes hold a control or direction character".into());
    }
    let a = raw.windows_x64;
    let url_ok = a.url.len() <= MAX_URL_BYTES
        && a.url.bytes().all(|b| b.is_ascii_graphic())
        && prefixes
            .iter()
            .any(|p| a.url.strip_prefix(p).is_some_and(|rest| !rest.is_empty()));
    if !url_ok {
        return Err("installer url is not on an allowed host".into());
    }
    if !(1..=MAX_INSTALLER_BYTES).contains(&a.size) {
        return Err("installer size is outside 1 B – 200 MiB".into());
    }
    let sha256 = hex32(&a.sha256).ok_or("sha256 is not 64 lower-case hex digits")?;
    let sig_ok = (1..=MAX_SIG_BYTES).contains(&a.sig.len())
        && a.sig
            .bytes()
            .all(|b| b == b'\n' || b.is_ascii_graphic() || b == b' ');
    if !sig_ok {
        return Err("sig is not a minisign signature text".into());
    }
    Ok(Doc {
        version,
        key_id: raw.key_id,
        notes: raw.notes,
        url: a.url,
        size: a.size,
        sha256,
        sig: a.sig,
    })
}

/// Characters that change how text around them is ordered or broken: plain text only.
fn is_direction_mark(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{2028}' | '\u{2029}')
}

/// The document after its signature: `sig` (the `.minisig` text) must verify over the raw
/// `bytes` with `verifier` — the baked key — before anything in them is read. A missing or
/// bad signature is no offer and no download.
pub fn verified_doc(
    bytes: &[u8],
    sig: &[u8],
    verifier: &dyn SignatureVerifier,
    prefixes: &[&str],
) -> Result<Doc, String> {
    if bytes.len() as u64 > MAX_DOC_BYTES || sig.len() > MAX_SIG_BYTES {
        return Err("document or signature too large".into());
    }
    let sig = std::str::from_utf8(sig).map_err(|_| "signature is not text")?;
    verifier
        .verify(&mut &bytes[..], sig)
        .map_err(|e| format!("document signature: {e}"))?;
    parse_doc(bytes, prefixes)
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let nib = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 32];
    for (i, pair) in b.chunks(2).enumerate() {
        out[i] = nib(pair[0])? << 4 | nib(pair[1])?;
    }
    Some(out)
}

/// What a parsed document offers this window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// Not newer than the running window (no downgrade, no same version).
    Nothing,
    /// Newer, and signed for the key this build trusts: installable in place.
    InPlace,
    /// Newer, but for another key (or none is baked): a newer window must be installed by hand.
    ByHand,
}

pub fn offer(doc: &Doc, running: &str, baked_key_id: Option<&str>) -> Offer {
    let Ok(running) = semver::Version::parse(running) else {
        return Offer::Nothing;
    };
    if doc.version <= running {
        Offer::Nothing
    } else if baked_key_id == Some(doc.key_id.as_str()) {
        Offer::InPlace
    } else {
        Offer::ByHand
    }
}

/// Where the window can install itself: Windows, or the test build on any system.
pub fn in_place_here() -> bool {
    cfg!(windows) || cfg!(feature = "update-test")
}

/// `app.mode` and `app.command` for an offer on this system.
pub fn mode(offer: Offer) -> (&'static str, Option<&'static str>) {
    match offer {
        Offer::Nothing => ("none", None),
        _ if !in_place_here() => (
            "command",
            Some(if cfg!(target_os = "macos") {
                COMMAND_MACOS
            } else {
                COMMAND_LINUX
            }),
        ),
        Offer::InPlace => ("in_place", None),
        Offer::ByHand => ("by_hand", None),
    }
}

/// The verifier this build trusts for the window's update.
pub fn verifier() -> Box<dyn SignatureVerifier + Send + Sync> {
    #[cfg(feature = "update-test")]
    if let Some(path) = crate::dirs::process_env("REIN_DESKTOP_UPDATE_KEY") {
        return match fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| MinisignVerifier::from_pub_file(&t))
        {
            Ok(v) => Box::new(v),
            Err(e) => Box::new(Rejecting::new(e)),
        };
    }
    match BAKED_APP_KEY.map(MinisignVerifier::from_pub_file) {
        Some(Ok(v)) => Box::new(v),
        Some(Err(e)) => Box::new(Rejecting::new(format!(
            "baked rein-app key is invalid: {e}"
        ))),
        None => Box::new(Rejecting::new(
            "no rein-app update key is baked into this build",
        )),
    }
}

/// The document's URL, its host policy and the installer URL prefixes. The test build reads
/// `REIN_DESKTOP_UPDATE_URL` (loopback HTTP only); every other build uses the constants.
pub fn endpoint() -> (String, HostPolicy, &'static [&'static str]) {
    #[cfg(feature = "update-test")]
    if let Some(u) = crate::dirs::process_env("REIN_DESKTOP_UPDATE_URL") {
        return (
            u,
            HostPolicy::loopback_only(),
            &["http://127.0.0.1:", "http://localhost:"],
        );
    }
    (
        LATEST_URL.into(),
        HostPolicy::github_assets(),
        ASSET_PREFIXES,
    )
}

/// Fetch the document (at most 64 KiB) and its signature (at most 4 KiB), each in at most
/// 20 s, through the same host policy.
pub fn fetch_doc(http: &dyn Http, url: &str) -> Result<(Vec<u8>, Vec<u8>), FetchError> {
    let mut doc = Vec::new();
    http.download(url, MAX_DOC_BYTES, &mut doc)?;
    let mut sig = Vec::new();
    http.download(
        &format!("{url}{DOC_SIG_SUFFIX}"),
        MAX_SIG_BYTES as u64,
        &mut sig,
    )?;
    Ok((doc, sig))
}

/// The download directory under the app's cache directory.
pub fn update_dir(cache: &Path) -> PathBuf {
    cache.join("update")
}

/// Delete every installer a previous run left (at start, and after every failure).
pub fn clean(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let is_ours = e.file_name().to_string_lossy().starts_with(FILE_PREFIX);
        if is_ours && e.file_type().is_ok_and(|t| t.is_file()) {
            let _ = fs::remove_file(e.path());
        }
    }
}

/// How an install ended without an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The installer is running; the window must exit now.
    Started,
    /// Cancel during the download: the file is gone, nothing started.
    Cancelled,
}

/// What an install needs from the window.
pub struct Ctx<'a> {
    pub http: &'a dyn Http,
    pub verifier: &'a dyn SignatureVerifier,
    pub dir: &'a Path,
    /// Unique per run: the file is created exclusively under this name.
    pub run: &'a str,
    pub cancel: &'a AtomicBool,
}

/// Install `doc`: download → signature → sha256 → `quit` (the ordinary quit's work) → `start`.
/// `progress` sees every job change. Every error deletes the file and starts nothing.
pub fn install(
    doc: &Doc,
    ctx: &Ctx<'_>,
    progress: &mut dyn FnMut(Job),
    quit: &mut dyn FnMut(),
    start: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<Outcome, Step> {
    let path = ctx.dir.join(format!("{FILE_PREFIX}{}.exe", ctx.run));
    let r = install_at(&path, doc, ctx, progress, quit, start);
    if !matches!(r, Ok(Outcome::Started)) {
        let _ = fs::remove_file(&path);
    }
    r
}

fn install_at(
    path: &Path,
    doc: &Doc,
    ctx: &Ctx<'_>,
    progress: &mut dyn FnMut(Job),
    quit: &mut dyn FnMut(),
    start: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<Outcome, Step> {
    fs::create_dir_all(ctx.dir).map_err(|_| Step::Fetch)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| Step::Fetch)?;
    progress(Job::Downloading {
        done: 0,
        total: doc.size,
    });
    let mut w = Counting {
        file,
        done: 0,
        total: doc.size,
        shown: 0,
        cancel: ctx.cancel,
        progress,
    };
    let got = ctx.http.download(&doc.url, doc.size, &mut w);
    if ctx.cancel.load(Ordering::Acquire) {
        return Ok(Outcome::Cancelled);
    }
    match got {
        Err(FetchError::TooLarge { .. }) => return Err(Step::TooLarge),
        Err(_) => return Err(Step::Fetch),
        Ok(n) if n != doc.size => return Err(Step::Fetch),
        Ok(_) => {}
    }
    w.file.sync_all().map_err(|_| Step::Fetch)?;
    drop(w);
    progress(Job::Verifying);
    let mut f = File::open(path).map_err(|_| Step::Signature)?;
    ctx.verifier
        .verify(&mut f, &doc.sig)
        .map_err(|_| Step::Signature)?;
    if sha256_file(path).ok() != Some(doc.sha256) {
        return Err(Step::Checksum);
    }
    progress(Job::Restarting);
    quit();
    start(path).map_err(|_| Step::Start)?;
    Ok(Outcome::Started)
}

fn sha256_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(h.finalize().into());
        }
        h.update(&buf[..n]);
    }
}

/// The download's `Write`: counts bytes for progress (at most one report per whole percent)
/// and stops at Cancel. No change to the `Http` trait (D-066 addendum 1 r4).
struct Counting<'a, 'p> {
    file: File,
    done: u64,
    total: u64,
    shown: u64,
    cancel: &'a AtomicBool,
    progress: &'p mut dyn FnMut(Job),
}

impl Write for Counting<'_, '_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Acquire) {
            // Not `Interrupted`: `write_all` retries that kind forever.
            return Err(io::Error::other("cancelled"));
        }
        let n = self.file.write(buf)?;
        self.done += n as u64;
        let pct = self.done * 100 / self.total.max(1);
        if pct > self.shown || self.done == self.total {
            self.shown = pct;
            (self.progress)(Job::Downloading {
                done: self.done,
                total: self.total,
            });
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Start the installer with the constant argv; the window exits right after. Windows only; the test build on other
/// systems prints the argv it would run and starts nothing.
pub fn start_installer(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        // A process Windows starts outlives the window that started it; no console (V5).
        let mut cmd = std::process::Command::new(path);
        cmd.args(INSTALLER_ARGS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        rein_platform::process::hide_console(&mut cmd);
        cmd.spawn().map(drop)
    }
    #[cfg(all(not(windows), feature = "update-test"))]
    {
        eprintln!(
            "rein-desktop: update-test: would start {} {}",
            path.display(),
            INSTALLER_ARGS.join(" ")
        );
        Ok(())
    }
    #[cfg(all(not(windows), not(feature = "update-test")))]
    {
        let _ = path;
        Err(io::Error::other(
            "the window updates itself on Windows only",
        ))
    }
}

#[cfg(test)]
#[path = "app_update_tests.rs"]
mod tests;
