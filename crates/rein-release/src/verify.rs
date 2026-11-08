//! The fail-closed verifier (blueprint §Components 2, order fixed by rein D-153 r1–r2):
//! (a) manifest schema → (b) `key_id` → (c) minisign over the tarball → (d) require the
//! in-tarball manifest to equal the beside one → (e) extract with sha256 + size of every
//! listed file, no unlisted/missing/unsafe/out-of-layout path → (f) apply `mode`.
//!
//! Hardening from the E-3 security review: only `schema`/`version`/`key_id` are read from
//! the unsigned beside manifest before the signature check (P1-1); the version must equal
//! the requested one (P1-2); the full manifest is parsed only after it is proven equal to
//! the signed in-tarball copy; the tarball's size and mtime must not change while it is
//! read (P2-1); a drop guard removes the extraction directory on any failure, including a
//! panic (P2-3); directory entries must carry no data (P2-4).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use flate2::read::GzDecoder;
use semver::Version;
use sha2::{Digest, Sha256};
use tar::EntryType;

use crate::error::VerifyError;
use crate::manifest::{FileEntry, MAX_MANIFEST_BYTES, Manifest, ManifestHeader};
use crate::paths::check_rel_path;
use crate::signature::{MAX_SIGNATURE_BYTES, SignatureVerifier};

/// The three downloaded assets of one release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAssets {
    /// `rein-X.Y.Z.tar.gz`
    pub tarball: PathBuf,
    /// `rein-X.Y.Z.tar.gz.minisig`
    pub signature: PathBuf,
    /// `manifest.json` (the copy beside the tarball)
    pub manifest: PathBuf,
}

/// A release whose every byte has been verified, extracted at `tree`.
#[derive(Debug)]
pub struct VerifiedRelease {
    pub manifest: Manifest,
    /// Directory holding exactly the manifest's files (plus `manifest.json`).
    pub tree: PathBuf,
}

/// Verify `assets` (which must be release `requested`) with `verifier` and extract into
/// `dest`, which must not exist yet.
pub fn verify_release(
    assets: &ReleaseAssets,
    verifier: &dyn SignatureVerifier,
    requested: &Version,
    dest: &Path,
) -> Result<VerifiedRelease, VerifyError> {
    // (a) beside manifest: schema, version, key id only — it is not signed.
    let beside = read_capped(&assets.manifest, MAX_MANIFEST_BYTES, "manifest.json")?;
    let header = ManifestHeader::peek(&beside)?;
    if &header.version != requested {
        return Err(VerifyError::VersionMismatch {
            requested: requested.to_string(),
            found: header.version.to_string(),
        });
    }

    // (b) key id before any signature work: an older app names the fix, not a bare error.
    let app_key = verifier.key_id().map_err(VerifyError::NoTrustedKey)?;
    if app_key != header.key_id {
        return Err(VerifyError::NeedsNewerApp {
            release_key: header.key_id,
            app_key,
            release_version: header.version.to_string(),
        });
    }

    // (c) signature over the tarball, read through one handle that is re-read below.
    let sig_bytes = read_capped(&assets.signature, MAX_SIGNATURE_BYTES, "signature")?;
    let sig = String::from_utf8(sig_bytes)
        .map_err(|_| VerifyError::BadSignature("signature file is not UTF-8".into()))?;
    let mut tarball = File::open(&assets.tarball)?;
    let stamp = file_stamp(&tarball)?;
    verifier
        .verify(&mut tarball, &sig)
        .map_err(VerifyError::BadSignature)?;

    // (d) the beside manifest must equal the signed in-tarball copy; only then parse it.
    tarball.seek(SeekFrom::Start(0))?;
    if read_inner_manifest(&mut tarball)? != beside {
        return Err(VerifyError::ManifestMismatch);
    }
    let manifest = Manifest::parse(&beside)?;

    // (e)–(f) extract into a fresh directory, removed on any failure.
    if fs::symlink_metadata(dest).is_ok() {
        return Err(VerifyError::DestinationExists(dest.display().to_string()));
    }
    fs::create_dir(dest)?;
    let guard = RemoveOnDrop(Some(dest.to_path_buf()));
    tarball.seek(SeekFrom::Start(0))?;
    extract(&manifest, &beside, &mut tarball, dest)?;
    if file_stamp(&tarball)? != stamp {
        return Err(VerifyError::TarballChanged);
    }
    guard.disarm();
    Ok(VerifiedRelease {
        manifest,
        tree: dest.to_path_buf(),
    })
}

/// Removes the directory when dropped, unless disarmed.
struct RemoveOnDrop(Option<PathBuf>);

impl RemoveOnDrop {
    fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = fs::remove_dir_all(p);
        }
    }
}

fn file_stamp(f: &File) -> Result<(u64, Option<SystemTime>), VerifyError> {
    let meta = f.metadata()?;
    Ok((meta.len(), meta.modified().ok()))
}

fn unreadable(e: std::io::Error) -> VerifyError {
    VerifyError::TarballUnreadable(e.to_string())
}

fn entry_path(entry: &tar::Entry<'_, impl Read>) -> Result<String, VerifyError> {
    let raw = entry.path_bytes();
    std::str::from_utf8(&raw)
        .map(str::to_owned)
        .map_err(|_| VerifyError::UnsafePath(String::from_utf8_lossy(&raw).into_owned()))
}

/// Pass 1: the bytes of the single root `manifest.json` entry. Writes nothing.
fn read_inner_manifest(tarball: &mut File) -> Result<Vec<u8>, VerifyError> {
    let mut archive = tar::Archive::new(GzDecoder::new(BufReader::new(tarball)));
    let mut found: Option<Vec<u8>> = None;
    for entry in archive.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        if entry_path(&entry)? != "manifest.json" {
            continue;
        }
        if found.is_some() {
            return Err(VerifyError::Duplicate("manifest.json".into()));
        }
        let mut buf = Vec::new();
        (&mut entry)
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(unreadable)?;
        if buf.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(VerifyError::ManifestInvalid(
                "in-tarball manifest too large".into(),
            ));
        }
        found = Some(buf);
    }
    found.ok_or(VerifyError::ManifestNotInTarball)
}

/// Pass 2: check and write every entry.
fn extract(
    manifest: &Manifest,
    manifest_bytes: &[u8],
    tarball: &mut File,
    dest: &Path,
) -> Result<(), VerifyError> {
    let listed: HashMap<&str, &FileEntry> = manifest
        .files
        .iter()
        .map(|f| (f.path.as_str(), f))
        .collect();
    let mut seen: HashSet<String> = HashSet::new();

    let mut archive = tar::Archive::new(GzDecoder::new(BufReader::new(tarball)));
    for entry in archive.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        let path = entry_path(&entry)?;
        let header_size = entry.header().size().map_err(unreadable)?;
        match entry.header().entry_type() {
            EntryType::Regular | EntryType::Continuous => {}
            EntryType::Directory => {
                let dir = path.strip_suffix('/').unwrap_or(&path);
                check_rel_path(dir).map_err(|_| VerifyError::UnsafePath(path.clone()))?;
                if header_size != 0 {
                    return Err(VerifyError::UnsupportedEntry {
                        path,
                        kind: "Directory with data".into(),
                    });
                }
                if !manifest.in_layout(dir, true) {
                    return Err(VerifyError::OutsideLayout(path));
                }
                continue; // directories are created on demand for listed files
            }
            // Metadata records the iterator has already applied.
            EntryType::XGlobalHeader | EntryType::XHeader => continue,
            other => {
                return Err(VerifyError::UnsupportedEntry {
                    path,
                    kind: format!("{other:?}"),
                });
            }
        }
        check_rel_path(&path).map_err(|_| VerifyError::UnsafePath(path.clone()))?;
        if !seen.insert(path.clone()) {
            return Err(VerifyError::Duplicate(path));
        }
        if path == "manifest.json" {
            continue; // proven equal to the beside copy in pass 1
        }
        if !manifest.in_layout(&path, false) {
            return Err(VerifyError::OutsideLayout(path));
        }
        let Some(file) = listed.get(path.as_str()) else {
            return Err(VerifyError::Unlisted(path));
        };
        if header_size != file.size {
            return Err(VerifyError::SizeMismatch {
                path,
                expected: file.size,
                actual: header_size,
            });
        }
        write_verified(dest, file, &mut entry)?;
    }

    if let Some(f) = manifest.files.iter().find(|f| !seen.contains(&f.path)) {
        return Err(VerifyError::Missing(f.path.clone()));
    }
    fs::write(dest.join("manifest.json"), manifest_bytes)?;
    Ok(())
}

/// Stream one entry to `dest/<path>`, hashing as it goes; check size and sha256; set mode.
fn write_verified(dest: &Path, file: &FileEntry, data: &mut dyn Read) -> Result<(), VerifyError> {
    let target = dest.join(&file.path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)?;
    let mut hasher = Sha256::new();
    let mut limited = data.take(file.size + 1);
    let mut buf = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let n = limited.read(&mut buf).map_err(unreadable)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        written += n as u64;
    }
    if written != file.size {
        return Err(VerifyError::SizeMismatch {
            path: file.path.clone(),
            expected: file.size,
            actual: written,
        });
    }
    if hex(&hasher.finalize()) != file.sha256 {
        return Err(VerifyError::HashMismatch(file.path.clone()));
    }
    set_mode(&out, file.mode.bits())?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(f: &File, bits: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    f.set_permissions(fs::Permissions::from_mode(bits))
}

#[cfg(not(unix))]
fn set_mode(_f: &File, _bits: u32) -> std::io::Result<()> {
    Ok(()) // Windows runs rein inside WSL (Component 8); modes apply there.
}

fn read_capped(path: &Path, cap: u64, what: &str) -> Result<Vec<u8>, VerifyError> {
    let mut buf = Vec::new();
    File::open(path)
        .map_err(|e| VerifyError::Io(format!("{what}: {e}")))?
        .take(cap + 1)
        .read_to_end(&mut buf)?;
    if buf.len() as u64 > cap {
        return Err(VerifyError::Io(format!("{what} exceeds {cap} bytes")));
    }
    Ok(buf)
}

/// Lower-case hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
