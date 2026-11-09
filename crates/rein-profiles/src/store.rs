//! Reading and writing one profile's `settings.json` (blueprint §Components 6,
//! §Rollback Plan): read → parse → plan → validate → backup → temp file + rename.
//!
//! Backup: `settings.json.rein-bak` holds the bytes of settings.json from before the FIRST
//! Rein write and is never overwritten while it exists (E-4 security review P1). A
//! zero-byte backup means "there was no settings.json".
//!
//! Undo restores ONLY the three allowed keys from the backup onto the current file — a key
//! absent from the backup is removed — and never copies any other key (E-4 security review
//! P0: restoring the whole file could re-grant `permissions`/`defaultMode`, rein D-132).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use crate::patch::{AllowedKey, KeyChange, Patch, PlanError, plan};
use crate::profile::Profile;

/// Why a profile could not be applied or undone. Other profiles are unaffected.
#[derive(Debug)]
pub enum ApplyError {
    /// `settings.json` (or its backup) is not valid JSON (CLI exit 5).
    Parse(String),
    /// The merge could not be planned (CLI exit 5).
    Plan(PlanError),
    /// No backup to restore.
    NoBackup,
    /// settings.json is a symlink resolving outside the profile's config dir.
    SymlinkOutsideProfile(PathBuf),
    /// settings.json changed while we were working on it (e.g. Claude Code wrote it).
    ChangedConcurrently,
    Io(std::io::Error),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::Parse(m) => write!(f, "not valid JSON ({m})"),
            ApplyError::Plan(e) => write!(f, "{e}"),
            ApplyError::NoBackup => write!(f, "no settings.json.rein-bak to restore"),
            ApplyError::SymlinkOutsideProfile(t) => write!(
                f,
                "settings.json links outside the profile to {}; refusing to write through it",
                t.display()
            ),
            ApplyError::ChangedConcurrently => {
                write!(
                    f,
                    "settings.json changed while it was being updated; nothing written, retry"
                )
            }
            ApplyError::Io(e) => write!(f, "i/o error ({e})"),
        }
    }
}

impl std::error::Error for ApplyError {}

impl From<std::io::Error> for ApplyError {
    fn from(e: std::io::Error) -> Self {
        ApplyError::Io(e)
    }
}

/// Options for [`apply`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyOptions {
    /// Plan and report only.
    pub dry_run: bool,
}

/// What applying a patch to one profile would do / did.
#[derive(Debug)]
pub struct Outcome {
    pub changes: Vec<KeyChange>,
    /// True when the file was (or, for a dry run, would be) written.
    pub written: bool,
}

/// The file actually written. A symlink is followed only when it stays inside the
/// (resolved) config dir; one escaping it is refused (E-4 security review P2, D-008).
pub fn resolve_settings_path(profile: &Profile) -> Result<PathBuf, ApplyError> {
    let Ok(real) = fs::canonicalize(&profile.settings_path) else {
        return Ok(profile.settings_path.clone()); // absent: written in place
    };
    let dir = fs::canonicalize(&profile.config_dir).unwrap_or_else(|_| profile.config_dir.clone());
    if real.parent() != Some(dir.as_path()) {
        return Err(ApplyError::SymlinkOutsideProfile(real));
    }
    Ok(real)
}

fn backup_path(settings: &Path) -> PathBuf {
    let mut name = settings.file_name().unwrap_or_default().to_os_string();
    name.push(".rein-bak");
    settings.with_file_name(name)
}

/// Read and parse a settings file; `None` when it does not exist.
pub fn read_settings(path: &Path) -> Result<Option<(Vec<u8>, Value)>, ApplyError> {
    match fs::read(path) {
        Ok(bytes) => {
            let value =
                serde_json::from_slice(&bytes).map_err(|e| ApplyError::Parse(e.to_string()))?;
            Ok(Some((bytes, value)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Snapshot settings.json into the backup unless a backup already exists. Called before
/// the first mutation of a run — ours or `claude plugin install`'s. The backup appears
/// atomically and complete (temp file, fsync, hard link that fails if one exists), so a
/// crash can never leave a truncated backup that undo would trust (security re-review P2).
pub fn ensure_backup(profile: &Profile) -> Result<(), ApplyError> {
    let target = resolve_settings_path(profile)?;
    let backup = backup_path(&target);
    if fs::symlink_metadata(&backup).is_ok() {
        return Ok(()); // keep the pre-Rein state
    }
    let bytes = match fs::read(&target) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let tmp = temp_path(&backup);
    let result = (|| -> Result<(), ApplyError> {
        let mut f = create_private(&tmp)?;
        f.write_all(&bytes)?;
        copy_permissions(&target, &f)?;
        f.sync_all()?;
        match fs::hard_link(&tmp, &backup) {
            Ok(()) => Ok(()),
            // Another run made the backup first: keep the earlier (pre-Rein) one.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e.into()),
        }
    })();
    let _ = fs::remove_file(&tmp);
    result
}

fn temp_path(path: &Path) -> PathBuf {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    dir.join(format!(
        ".{}.rein-tmp-{}-{nanos}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ))
}

/// Plan `patch` for `profile` and, unless a dry run, back up (once) and write it. Writes
/// nothing when no allowed key changes. A missing config dir is not created.
pub fn apply(profile: &Profile, patch: &Patch, opts: ApplyOptions) -> Result<Outcome, ApplyError> {
    let target = resolve_settings_path(profile)?;
    let current = read_settings(&target)?;
    let before = current
        .as_ref()
        .map_or_else(|| Value::Object(Map::new()), |(_, v)| v.clone());
    let (after, changes) = plan(&before, patch).map_err(ApplyError::Plan)?;
    if changes.is_empty() || opts.dry_run {
        return Ok(Outcome {
            written: !changes.is_empty(),
            changes,
        });
    }
    require_config_dir(profile)?;
    ensure_backup(profile)?;
    let read_bytes = current.map(|(b, _)| b);
    write_settings(&target, &after, read_bytes.as_deref())?;
    Ok(Outcome {
        changes,
        written: true,
    })
}

/// Restore the three allowed keys from the backup onto the current settings, then delete
/// the backup. Every other key in the current file is kept as it is now.
pub fn undo(profile: &Profile) -> Result<(), ApplyError> {
    let target = resolve_settings_path(profile)?;
    let backup = backup_path(&target);
    let backup_bytes = match fs::read(&backup) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ApplyError::NoBackup),
        Err(e) => return Err(e.into()),
    };
    let original: Map<String, Value> = if backup_bytes.is_empty() {
        Map::new()
    } else {
        match serde_json::from_slice(&backup_bytes) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(ApplyError::Parse("backup is not a JSON object".into())),
            Err(e) => return Err(ApplyError::Parse(format!("backup: {e}"))),
        }
    };
    let current = read_settings(&target)?;
    let mut next = match current.as_ref().map(|(_, v)| v) {
        None => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return Err(ApplyError::Plan(PlanError::NotAnObject)),
    };
    for key in AllowedKey::ALL {
        let name = key.json_key();
        match original.get(name) {
            Some(v) => next.insert(name.to_owned(), v.clone()),
            None => next.remove(name),
        };
    }

    let unchanged = current
        .as_ref()
        .is_some_and(|(_, v)| v.as_object() == Some(&next));
    let read_bytes = current.map(|(b, _)| b);
    if next.is_empty() && backup_bytes.is_empty() {
        // There was no settings.json and nothing but Rein keys was added since.
        if read_bytes.is_some() {
            unchanged_since(&target, read_bytes.as_deref())?;
            fs::remove_file(&target)?;
        }
    } else if !unchanged {
        write_settings(&target, &Value::Object(next), read_bytes.as_deref())?;
    }
    fs::remove_file(&backup)?;
    Ok(())
}

fn require_config_dir(profile: &Profile) -> Result<(), ApplyError> {
    if profile.config_dir.is_dir() {
        Ok(())
    } else {
        Err(ApplyError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("config dir {} does not exist", profile.config_dir.display()),
        )))
    }
}

/// Fail if the file's bytes differ from what we read (`None` = it did not exist).
fn unchanged_since(path: &Path, read: Option<&[u8]>) -> Result<(), ApplyError> {
    let now = match fs::read(path) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    if now.as_deref() == read {
        Ok(())
    } else {
        Err(ApplyError::ChangedConcurrently)
    }
}

/// Serialise, validate the round trip, and atomically replace `path` — refusing if the
/// file changed since it was read (E-4 security review P2: concurrent writer).
fn write_settings(path: &Path, doc: &Value, read: Option<&[u8]>) -> Result<(), ApplyError> {
    let mut text =
        serde_json::to_string_pretty(doc).map_err(|e| ApplyError::Parse(e.to_string()))?;
    text.push('\n');
    let reparsed: Value =
        serde_json::from_str(&text).map_err(|e| ApplyError::Parse(e.to_string()))?;
    if &reparsed != doc {
        return Err(ApplyError::Parse(
            "serialised settings do not round-trip".into(),
        ));
    }
    let tmp = temp_path(path);
    let result = (|| -> Result<(), ApplyError> {
        let mut f = create_private(&tmp)?;
        f.write_all(text.as_bytes())?;
        copy_permissions(path, &f)?;
        f.sync_all()?;
        unchanged_since(path, read)?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Create a new file readable only by the owner until its real mode is copied
/// (E-4 security review P2: no world-readable window for settings that may hold secrets).
fn create_private(path: &Path) -> std::io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

fn copy_permissions(from: &Path, to: &File) -> std::io::Result<()> {
    match fs::metadata(from) {
        Ok(meta) => to.set_permissions(meta.permissions()),
        Err(_) => Ok(()), // new file: keep the private mode
    }
}
