//! `LayoutWriter` — lay a verified release out as the Rein home (blueprint §Components 3,
//! rein D-153 r5, §Security trust boundary 2, §Execution Constraints single-writer).
//!
//! The next tree is built in `$HOME/.rein.staging-<ver>` and swapped in by rename:
//!
//! 1. every listed file under a `layout.mirrored` directory comes from the verified release,
//!    so a mirrored directory holds exactly what the tarball ships (stale files, e.g. a
//!    retired `bin/old-tool`, are gone);
//! 2. every path matching `layout.operator_owned` is carried over from the live tree, never
//!    taken from the tarball — the list is read from the manifest, never hard-coded;
//! 3. every top-level entry of the live tree that is NOT a mirrored directory is carried
//!    over as is. `install-rein.sh` never touches those either, and the live home holds
//!    rein data the schema-1 `operator_owned` list does not name (`approvals.sqlite`,
//!    `*.sqlite-wal`, `ci/`, `trash/`): dropping them would lose committed data;
//! 4. `VERSION` is written last (temp file + rename);
//! 5. the live tree is renamed to `$HOME/.rein.prev-<oldver>` (`~N` appended if taken)
//!    and the staging tree to `$HOME/.rein`; at most [`KEEP_PREVIOUS`] previous trees are
//!    kept.
//!
//! Directories in the new tree take the mode of the same directory in the live tree, and
//! the staging root is 0700 until it is complete, so a `chmod 700` home stays private.
//!
//! Carried-over entries are copied, never moved, so the live tree is unchanged until the
//! swap. Symlinks are copied as links (never followed); sockets and FIFOs are skipped.
//! `$HOME/.rein.lock` keeps a second `install`/`update`/`rollback` out.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use semver::Version;

use crate::manifest::Manifest;
use crate::verify::VerifiedRelease;

/// Previous trees kept beside the live one (blueprint: N=2).
pub const KEEP_PREVIOUS: usize = 2;

const PREV_PREFIX: &str = ".rein.prev-";
const STAGING_PREFIX: &str = ".rein.staging-";

/// Where the Rein home and its siblings live under a home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPaths {
    home: PathBuf,
}

impl InstallPaths {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
    /// `$HOME/.rein`
    pub fn rein(&self) -> PathBuf {
        self.home.join(".rein")
    }
    /// `$HOME/.rein.lock`
    pub fn lock(&self) -> PathBuf {
        self.home.join(".rein.lock")
    }
    /// `$HOME/.rein.staging-<ver>`
    pub fn staging(&self, v: &Version) -> PathBuf {
        self.home.join(format!("{STAGING_PREFIX}{v}"))
    }
    /// `$HOME/.rein.prev-<label>`
    pub fn prev(&self, label: &str) -> PathBuf {
        self.home.join(format!("{PREV_PREFIX}{label}"))
    }
    /// Previous trees, newest first (by when their `VERSION` was written).
    pub fn previous_trees(&self) -> io::Result<Vec<PathBuf>> {
        let mut found = Vec::new();
        for entry in fs::read_dir(&self.home)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.starts_with(PREV_PREFIX) || !entry.file_type()?.is_dir() {
                continue; // a symlink named .rein.prev-* is never ours to touch
            }
            let stamp = fs::symlink_metadata(entry.path().join("VERSION"))
                .and_then(|m| m.modified())
                .ok();
            found.push((stamp, entry.path()));
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        Ok(found.into_iter().map(|(_, p)| p).collect())
    }
}

/// The installed tree's version, read from `<tree>/VERSION`. `None` if absent or not semver.
pub fn installed_version(tree: &Path) -> Option<Version> {
    let raw = fs::read_to_string(tree.join("VERSION")).ok()?;
    Version::parse(raw.trim()).ok()
}

/// What an install did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    pub version: Version,
    /// Where the replaced tree now lives, if there was one.
    pub previous: Option<PathBuf>,
    /// Previous trees removed to keep [`KEEP_PREVIOUS`].
    pub pruned: Vec<PathBuf>,
    /// Listed files that match `operator_owned` and were therefore not written.
    pub skipped_operator_owned: Vec<String>,
    /// Live-tree entries not carried over (sockets, FIFOs, devices).
    pub skipped_special: Vec<String>,
}

/// Laying out the tree failed. `$HOME/.rein` is unchanged unless the error says otherwise.
#[derive(Debug)]
pub enum LayoutError {
    /// Another install/update/rollback holds `$HOME/.rein.lock`.
    Locked(PathBuf),
    /// `$HOME/.rein` is missing but a previous tree exists.
    OrphanedPrevious(PathBuf),
    /// `$HOME/.rein` exists but is not a directory (a symlink or a file).
    NotADirectory(PathBuf),
    /// Building the staging tree failed; the staging tree was removed.
    Staging(String),
    /// The swap failed; the staging tree is left for inspection. `restored` says whether
    /// `$HOME/.rein` was put back (it is only missing if `restored` is false).
    Swap {
        reason: String,
        staging: PathBuf,
        restored: bool,
    },
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::Locked(p) => write!(
                f,
                "layout: {} exists — another rein-app install/update/rollback is running; it holds that run's PID — remove the file only if no such process is alive",
                p.display()
            ),
            LayoutError::OrphanedPrevious(p) => write!(
                f,
                "layout: $HOME/.rein is missing but {} exists — move it back to $HOME/.rein (or remove it) and re-run",
                p.display()
            ),
            LayoutError::NotADirectory(p) => write!(
                f,
                "layout: {} is not a directory (symlinked Rein homes are not supported)",
                p.display()
            ),
            LayoutError::Staging(m) => write!(f, "layout: staging failed ({m})"),
            LayoutError::Swap {
                reason,
                staging,
                restored,
            } => write!(
                f,
                "layout: swap failed ({reason}); staging tree left at {}; previous tree {}",
                staging.display(),
                if *restored {
                    "restored"
                } else {
                    "NOT restored — see the .rein.prev-* directory"
                }
            ),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Held while laying out; removes `$HOME/.rein.lock` when dropped.
#[derive(Debug)]
pub struct InstallLock {
    path: PathBuf,
    /// What this run wrote into the lock; only a lock holding it is removed (P2-2).
    token: String,
}

impl InstallLock {
    /// Create the lock file exclusively.
    pub fn acquire(paths: &InstallPaths) -> Result<Self, LayoutError> {
        let path = paths.lock();
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos());
                let token = format!("{} {nanos}\n", std::process::id());
                f.write_all(token.as_bytes())
                    .map_err(|e| LayoutError::Staging(format!("{}: {e}", path.display())))?;
                Ok(Self { path, token })
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(LayoutError::Locked(path)),
            Err(e) => Err(LayoutError::Staging(format!("{}: {e}", path.display()))),
        }
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        // Someone may have removed a lock they believed stale and another run taken it.
        if fs::read_to_string(&self.path).is_ok_and(|t| t == self.token) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Install `release` as `$HOME/.rein`. Takes the install lock for the duration.
pub fn install_tree(
    paths: &InstallPaths,
    release: &VerifiedRelease,
) -> Result<InstallOutcome, LayoutError> {
    let _lock = InstallLock::acquire(paths)?;
    install_tree_locked(paths, release)
}

/// [`install_tree`] for a caller that already holds the [`InstallLock`].
pub fn install_tree_locked(
    paths: &InstallPaths,
    release: &VerifiedRelease,
) -> Result<InstallOutcome, LayoutError> {
    let live = paths.rein();
    let live_exists = match fs::symlink_metadata(&live) {
        Ok(m) if m.is_dir() => true,
        Ok(_) => return Err(LayoutError::NotADirectory(live)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(LayoutError::Staging(format!("{}: {e}", live.display()))),
    };
    // No live tree but a previous one: a swap failed and was not restored, or the operator
    // moved the home away. Installing fresh would leave their data behind (P2-4).
    if !live_exists
        && let Some(prev) = paths
            .previous_trees()
            .ok()
            .and_then(|p| p.into_iter().next())
    {
        return Err(LayoutError::OrphanedPrevious(prev));
    }

    let manifest = &release.manifest;
    let staging = paths.staging(&manifest.version);
    let mut outcome = InstallOutcome {
        version: manifest.version.clone(),
        previous: None,
        pruned: Vec::new(),
        skipped_operator_owned: Vec::new(),
        skipped_special: Vec::new(),
    };

    // A staging tree left by an earlier failed swap is ours by name; rebuild from scratch.
    remove_own_dir(&staging).map_err(|e| LayoutError::Staging(e.to_string()))?;
    let built = build_staging(
        manifest,
        &release.tree,
        live_exists.then_some(live.as_path()),
        &staging,
        &mut outcome,
    );
    if let Err(e) = built {
        let _ = fs::remove_dir_all(&staging);
        return Err(LayoutError::Staging(e.to_string()));
    }

    // The swap: the only mutation of $HOME/.rein.
    if live_exists {
        let label =
            installed_version(&live).map_or_else(|| "unknown".to_string(), |v| v.to_string());
        let prev = free_prev_slot(paths, &label);
        if let Err(e) = fs::rename(&live, &prev) {
            return Err(LayoutError::Swap {
                reason: format!("moving {} aside: {e}", live.display()),
                staging,
                restored: true, // the live tree was not moved
            });
        }
        if let Err(e) = fs::rename(&staging, &live) {
            let restored = fs::rename(&prev, &live).is_ok();
            return Err(LayoutError::Swap {
                reason: format!("renaming staging into place: {e}"),
                staging,
                restored,
            });
        }
        outcome.previous = Some(prev);
    } else if let Err(e) = fs::rename(&staging, &live) {
        return Err(LayoutError::Swap {
            reason: format!("renaming staging into place: {e}"),
            staging,
            restored: true, // there was nothing to restore
        });
    }

    // Keep N previous trees. A failure here leaves an extra tree, never a broken one.
    if let Ok(prevs) = paths.previous_trees() {
        for old in prevs.into_iter().skip(KEEP_PREVIOUS) {
            if fs::remove_dir_all(&old).is_ok() {
                outcome.pruned.push(old);
            }
        }
    }
    Ok(outcome)
}

/// `.rein.prev-<label>`, or `.rein.prev-<label>~N` if that name is taken — an existing
/// previous tree is never overwritten (P2-3).
fn free_prev_slot(paths: &InstallPaths, label: &str) -> PathBuf {
    let mut slot = paths.prev(label);
    let mut n = 1;
    while fs::symlink_metadata(&slot).is_ok() {
        slot = paths.prev(&format!("{label}~{n}"));
        n += 1;
    }
    slot
}

/// Remove `dir` if it is a real directory; refuse a symlink or file in its place.
fn remove_own_dir(dir: &Path) -> io::Result<()> {
    match fs::symlink_metadata(dir) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(dir),
        Ok(_) => Err(io::Error::other(format!(
            "{} exists and is not a directory",
            dir.display()
        ))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Where carried entries come from and go to.
struct Trees<'a> {
    staging: &'a Path,
    live: Option<&'a Path>,
    /// Directories created by `ensure_parents`, in creation order (parents first).
    created: std::cell::RefCell<Vec<PathBuf>>,
}

impl Trees<'_> {
    /// Create the missing parents of `rel` inside staging, 0700 for now; [`Self::apply_modes`]
    /// gives each the mode of the same live directory at the end, deepest first, so a
    /// read-only live directory (0500 `config/`) does not block writing into its copy.
    /// The staging root stays 0700 until then (E-5 security review P1-1, re-review P2-1).
    fn ensure_parents(&self, rel: &Path) -> io::Result<()> {
        let Some(parent) = rel.parent() else {
            return Ok(());
        };
        let mut cur = PathBuf::new();
        for comp in parent.components() {
            cur.push(comp);
            let dir = self.staging.join(&cur);
            if fs::symlink_metadata(&dir).is_err() {
                create_private_dir(&dir)?;
                self.created.borrow_mut().push(cur.clone());
            }
        }
        Ok(())
    }

    /// Give every directory `ensure_parents` created its live counterpart's mode (default
    /// 0755), children before parents.
    fn apply_modes(&self) -> io::Result<()> {
        for rel in self.created.borrow().iter().rev() {
            set_dir_mode(
                &self.staging.join(rel),
                self.live.map(|l| l.join(rel)).as_deref(),
            )?;
        }
        Ok(())
    }
}

fn build_staging(
    manifest: &Manifest,
    verified: &Path,
    live: Option<&Path>,
    staging: &Path,
    outcome: &mut InstallOutcome,
) -> io::Result<()> {
    create_private_dir(staging)?;
    let trees = Trees {
        staging,
        live,
        created: std::cell::RefCell::new(Vec::new()),
    };
    let owned = manifest.operator_owned();

    // 1. Mirrored files from the verified release (operator-owned ones are never written).
    for file in &manifest.files {
        if owned.matches(&file.path, false) || has_owned_ancestor(manifest, &file.path) {
            outcome.skipped_operator_owned.push(file.path.clone());
            continue;
        }
        let rel = Path::new(&file.path);
        trees.ensure_parents(rel)?;
        fs::copy(verified.join(rel), staging.join(rel))?; // the verifier's mode comes along
    }

    // 2–3. Carry over from the live tree.
    if let Some(live) = live {
        for entry in fs::read_dir(live)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(utf8) = name.to_str() else {
                // Not a manifest path either way: carry it over untouched.
                copy_entry(&trees, Path::new(&name), outcome)?;
                continue;
            };
            if utf8 == "VERSION" {
                continue; // written last, below
            }
            let mirrored = manifest.layout.mirrored.iter().any(|m| m == utf8);
            if !mirrored {
                copy_entry(&trees, Path::new(&name), outcome)?;
            } else if entry.file_type()?.is_dir() {
                carry_owned(manifest, &trees, utf8, outcome)?;
            }
            // A mirrored name that is not a directory in the live tree is replaced.
        }
    }

    // 4. VERSION last, then the root takes the live root's mode.
    let tmp = staging.join("VERSION.tmp");
    let mut f = File::create_new(&tmp)?;
    writeln!(f, "{}", manifest.version)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, staging.join("VERSION"))?;
    trees.apply_modes()?;
    set_dir_mode(staging, live)
}

/// Does any proper ancestor directory of `path` match `operator_owned`?
fn has_owned_ancestor(manifest: &Manifest, path: &str) -> bool {
    let owned = manifest.operator_owned();
    let mut end = 0;
    while let Some(i) = path[end..].find('/') {
        end += i;
        if owned.matches(&path[..end], true) {
            return true;
        }
        end += 1;
    }
    false
}

/// Inside a mirrored directory, carry over only operator-owned entries.
fn carry_owned(
    manifest: &Manifest,
    trees: &Trees<'_>,
    rel: &str,
    outcome: &mut InstallOutcome,
) -> io::Result<()> {
    let owned = manifest.operator_owned();
    let Some(live) = trees.live else {
        return Ok(());
    };
    for entry in fs::read_dir(live.join(rel))? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue }; // never a manifest path
        let child = format!("{rel}/{name}");
        let kind = entry.file_type()?;
        if owned.matches(&child, kind.is_dir()) {
            copy_entry(trees, Path::new(&child), outcome)?;
        } else if kind.is_dir() && owned.may_contain(&child) {
            carry_owned(manifest, trees, &child, outcome)?;
        }
    }
    Ok(())
}

/// Copy the live entry at `rel` (a file, symlink or directory tree) into staging without
/// following symlinks. An existing target is an error, never overwritten.
fn copy_entry(trees: &Trees<'_>, rel: &Path, outcome: &mut InstallOutcome) -> io::Result<()> {
    let Some(live) = trees.live else {
        return Ok(());
    };
    let (from, to) = (live.join(rel), trees.staging.join(rel));
    let meta = fs::symlink_metadata(&from)?;
    trees.ensure_parents(rel)?;
    let kind = meta.file_type();
    if kind.is_symlink() {
        copy_symlink(&from, &to)
    } else if kind.is_dir() {
        create_private_dir(&to)?;
        for entry in fs::read_dir(&from)? {
            copy_entry(trees, &rel.join(entry?.file_name()), outcome)?;
        }
        fs::set_permissions(&to, meta.permissions())
    } else if kind.is_file() {
        if fs::symlink_metadata(&to).is_ok() {
            return Err(io::Error::other(format!(
                "{}: target already exists",
                rel.display()
            )));
        }
        fs::copy(&from, &to).map(|_| ())
    } else {
        outcome.skipped_special.push(rel.display().to_string());
        Ok(())
    }
}

/// `mkdir` with mode 0700 (unix).
fn create_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(dir)
    }
}

/// Give `dir` the permission bits of `like` when that is a directory, else 0755.
#[cfg(unix)]
fn set_dir_mode(dir: &Path, like: Option<&Path>) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = like
        .and_then(|p| fs::symlink_metadata(p).ok())
        .filter(fs::Metadata::is_dir)
        .map_or(0o755, |m| m.permissions().mode() & 0o7777);
    fs::set_permissions(dir, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_dir_mode(_dir: &Path, _like: Option<&Path>) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(from)?, to)
}

#[cfg(not(unix))]
fn copy_symlink(from: &Path, _to: &Path) -> io::Result<()> {
    // Windows runs rein inside WSL (Component 8); the layout writer runs there.
    Err(io::Error::other(format!(
        "{}: symlinks are not supported on this platform",
        from.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_siblings_of_the_rein_home() {
        let p = InstallPaths::new("/h");
        let v = Version::new(0, 5, 0);
        assert_eq!(p.rein(), Path::new("/h/.rein"));
        assert_eq!(p.lock(), Path::new("/h/.rein.lock"));
        assert_eq!(p.staging(&v), Path::new("/h/.rein.staging-0.5.0"));
        assert_eq!(p.prev("0.4.0"), Path::new("/h/.rein.prev-0.4.0"));
    }

    #[test]
    fn installed_version_rejects_non_semver_so_it_never_names_a_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("VERSION"), "../../etc\n").unwrap();
        assert_eq!(installed_version(dir.path()), None);
        fs::write(dir.path().join("VERSION"), "0.4.0\n").unwrap();
        assert_eq!(installed_version(dir.path()), Some(Version::new(0, 4, 0)));
    }

    #[test]
    fn lock_is_exclusive_and_released_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let p = InstallPaths::new(dir.path());
        let held = InstallLock::acquire(&p).unwrap();
        assert!(matches!(
            InstallLock::acquire(&p),
            Err(LayoutError::Locked(_))
        ));
        drop(held);
        assert!(InstallLock::acquire(&p).is_ok());
    }

    #[test]
    fn dropping_a_lock_never_removes_another_runs_lock() {
        let dir = tempfile::tempdir().unwrap();
        let p = InstallPaths::new(dir.path());
        let held = InstallLock::acquire(&p).unwrap();
        fs::write(p.lock(), "4242 1\n").unwrap(); // taken over by another run
        drop(held);
        assert!(p.lock().exists());
    }
}
