//! `LayoutWriter` — lay a verified release out as the Rein home by an **in-place, per-file
//! sync of the mirrored directories** (blueprint §Components 3 steps a–f, D-011, which
//! supersedes the E-5 whole-home staging swap), the way `install-rein.sh` does it.
//!
//! `$HOME/.rein` itself is never renamed, copied or moved. Only paths under a
//! `layout.mirrored` directory and `VERSION` are opened; inside the mirrored dirs,
//! `layout.operator_owned` paths are never opened either. Steps, all under
//! `$HOME/.rein.lock`:
//!
//! a. preflight refusals before any write (symlinked home, mirrored dir or target component);
//! b. pure [`SyncPlan`](crate::SyncPlan) `{ship, prune}`;
//! c. snapshot of the live payload to `$HOME/.rein.prev-<oldver>` (N=2 kept);
//! d. sync, `hooks` last: temp file beside the target, mode, rename; then prune;
//! e. `VERSION` last;
//! f. on a failure in d/e, sync the snapshot back; `VERSION` stays old.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use semver::Version;

use crate::snapshot;
use crate::sync::{self, Faults, ShipItem, Source};
use crate::verify::VerifiedRelease;

const PREV_PREFIX: &str = ".rein.prev-";

/// Where the Rein home and its siblings live under a home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPaths {
    home: PathBuf,
}

impl InstallPaths {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
    /// The home directory itself.
    pub fn home(&self) -> &Path {
        &self.home
    }
    /// `$HOME/.rein`
    pub fn rein(&self) -> PathBuf {
        self.home.join(".rein")
    }
    /// `$HOME/.rein.lock`
    pub fn lock(&self) -> PathBuf {
        self.home.join(".rein.lock")
    }
    /// `$HOME/.rein.prev-<label>`
    pub fn prev(&self, label: &str) -> PathBuf {
        self.home.join(format!("{PREV_PREFIX}{label}"))
    }
    /// Snapshots (directories holding real `VERSION` and `layout.json` files), newest first
    /// by when their `VERSION` was written.
    pub fn previous_trees(&self) -> io::Result<Vec<PathBuf>> {
        let mut found = Vec::new();
        for entry in fs::read_dir(&self.home)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.starts_with(PREV_PREFIX) || !entry.file_type()?.is_dir() {
                continue; // a symlink named .rein.prev-* is never ours to touch
            }
            // Only a snapshot we wrote (real VERSION + layout.json files) is pruned or
            // restored; an operator's own `.rein.prev-manual` is left alone (review P2-3).
            let real_file =
                |n: &str| fs::symlink_metadata(entry.path().join(n)).is_ok_and(|m| m.is_file());
            if !real_file("VERSION") || !real_file("layout.json") {
                continue;
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

/// `.rein.prev-<label>`, or `.rein.prev-<label>~N` if that name is taken — an existing
/// snapshot is never overwritten.
pub(crate) fn free_prev_slot(paths: &InstallPaths, label: &str) -> PathBuf {
    let mut slot = paths.prev(label);
    let mut n = 1;
    while fs::symlink_metadata(&slot).is_ok() {
        slot = paths.prev(&format!("{label}~{n}"));
        n += 1;
    }
    slot
}

/// What an install did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    pub version: Version,
    /// The snapshot of the replaced payload, if there was an installed version.
    pub previous: Option<PathBuf>,
    /// Snapshots removed to keep [`snapshot::KEEP_SNAPSHOTS`](crate::KEEP_SNAPSHOTS).
    pub pruned: Vec<PathBuf>,
    /// Listed files that match `operator_owned` and were therefore not written.
    pub skipped_operator_owned: Vec<String>,
    /// A populated home without a semver `VERSION` was updated with no snapshot, so this
    /// update cannot be rolled back (D-013 notice).
    pub unsnapshotted: bool,
}

/// Laying out the tree failed.
#[derive(Debug)]
pub enum LayoutError {
    /// Another install/update/rollback holds `$HOME/.rein.lock`.
    Locked(PathBuf),
    /// A path that must be a directory is not one (nothing was written).
    NotADirectory(PathBuf),
    /// A symlink where the writer would have to write through it (nothing was written).
    Symlink(PathBuf),
    /// A directory where a release file goes, or a file where a directory is needed
    /// (nothing was written).
    Conflict(PathBuf),
    /// A snapshot could not be taken or read.
    Snapshot(String),
    /// The sync failed. `snapshot` was synced back when `restored`; `VERSION` is unchanged.
    Sync {
        reason: String,
        snapshot: Option<PathBuf>,
        restored: bool,
    },
    /// The run's user does not own `path` (owner guard, D-013): nothing was written.
    NotOwner {
        path: PathBuf,
        run_uid: u32,
        owner_uid: u32,
    },
    /// Local I/O failed before any write.
    Io(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::Locked(p) => write!(
                f,
                "layout: {} exists — another rein-app install/update/rollback is running; it holds that run's PID — remove the file only if no such process is alive",
                p.display()
            ),
            LayoutError::NotADirectory(p) => {
                write!(
                    f,
                    "layout: {} is not a directory — nothing was changed",
                    p.display()
                )
            }
            LayoutError::Symlink(p) => write!(
                f,
                "layout: {} is a symlink; rein-app never writes through one — nothing was changed",
                p.display()
            ),
            LayoutError::Conflict(p) => write!(
                f,
                "layout: {} is in the way of the release layout (file vs directory) — nothing was changed",
                p.display()
            ),
            LayoutError::Snapshot(m) => write!(f, "layout: snapshot: {m}"),
            LayoutError::Sync {
                reason,
                snapshot,
                restored,
            } => match (snapshot, restored) {
                (Some(s), true) => write!(
                    f,
                    "layout: sync failed ({reason}); the previous files were restored from {}",
                    s.display()
                ),
                (Some(s), false) => write!(
                    f,
                    "layout: sync failed ({reason}) and restoring {} failed too — re-run install or rollback",
                    s.display()
                ),
                (None, _) => write!(
                    f,
                    "layout: sync failed ({reason}); no earlier version to restore — re-run install to complete it"
                ),
            },
            LayoutError::NotOwner {
                path,
                run_uid,
                owner_uid,
            } => write!(
                f,
                "layout: {} belongs to uid {owner_uid} but rein-app runs as uid {run_uid} — run it as the owner (not via sudo); nothing was changed",
                path.display()
            ),
            LayoutError::Io(m) => write!(f, "layout: i/o error ({m})"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Held while laying out; removes `$HOME/.rein.lock` when dropped, if still its own.
#[derive(Debug)]
pub struct InstallLock {
    path: PathBuf,
    token: String,
    /// Owner of the file this run created, read from the open handle (fstat) — never from
    /// the path, which a process owning `$HOME` could rename over (E-10 review P1-1).
    uid: Option<u32>,
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
                    .map_err(|e| LayoutError::Io(format!("{}: {e}", path.display())))?;
                let lock = Self {
                    path,
                    token,
                    uid: None,
                };
                // Fail closed: on unix a uid we cannot read refuses the run instead of
                // silently skipping the owner guard (E-10 re-review P2). Dropping `lock`
                // removes the file.
                let mut lock = lock;
                lock.uid = handle_uid(&f)
                    .map_err(|e| LayoutError::Io(format!("{}: {e}", lock.path.display())))?;
                Ok(lock)
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(LayoutError::Locked(path)),
            Err(e) => Err(LayoutError::Io(format!("{}: {e}", path.display()))),
        }
    }
}

impl InstallLock {
    /// The uid this run writes as: the owner of the lock file it has just created (unix;
    /// no `unsafe`, no extra dependency). `None` elsewhere — the guard is then a no-op.
    pub fn run_uid(&self) -> Option<u32> {
        self.uid
    }
}

#[cfg(unix)]
fn handle_uid(f: &fs::File) -> io::Result<Option<u32>> {
    use std::os::unix::fs::MetadataExt;
    Ok(Some(f.metadata()?.uid()))
}

#[cfg(not(unix))]
fn handle_uid(_f: &fs::File) -> io::Result<Option<u32>> {
    Ok(None)
}

/// How a run learns its own uid; tests substitute a different one to inject a mismatch.
pub type RunUid = fn(&InstallLock) -> Option<u32>;

/// Owner of `path` (not following a symlink). `None` if it does not exist or off unix.
pub fn uid_of(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        fs::symlink_metadata(path).ok().map(|m| m.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Owner of the directory `path` resolves to (following a symlink).
fn dir_uid(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(path).ok().map(|m| m.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// The owner guard's comparison (pure): the first path whose owner is not `run_uid`.
pub fn check_owners(run_uid: u32, owners: &[(PathBuf, u32)]) -> Result<(), LayoutError> {
    match owners.iter().find(|(_, uid)| *uid != run_uid) {
        Some((path, owner_uid)) => Err(LayoutError::NotOwner {
            path: path.clone(),
            run_uid,
            owner_uid: *owner_uid,
        }),
        None => Ok(()),
    }
}

/// Owner guard (D-013, blueprint §Components 3 step a): right after the lock is taken,
/// refuse unless this run's user owns `$HOME`, `$HOME/.rein` (when it exists) and every
/// path in `extra`. A no-op where uids do not exist (Windows).
pub fn owner_guard<'l>(
    lock: &'l InstallLock,
    paths: &InstallPaths,
    extra: &[&Path],
) -> Result<OwnerChecked<'l>, LayoutError> {
    owner_guard_with(lock, InstallLock::run_uid, paths, extra)
}

/// [`owner_guard`] with the uid source injectable (tests inject a mismatch).
#[doc(hidden)]
pub fn owner_guard_with<'l>(
    lock: &'l InstallLock,
    run_uid: RunUid,
    paths: &InstallPaths,
    extra: &[&Path],
) -> Result<OwnerChecked<'l>, LayoutError> {
    // The proof covers the home whose lock is held — never another one (E-11 review P1-1).
    if lock.path != paths.lock() {
        return Err(LayoutError::Io(format!(
            "owner guard: the lock held is {} but the home is {}",
            lock.path.display(),
            paths.home().display()
        )));
    }
    let proof = OwnerChecked {
        _lock: lock,
        home: paths.home().to_path_buf(),
    };
    let Some(me) = run_uid(lock) else {
        return Ok(proof); // no uids on this platform: nothing to compare
    };
    // $HOME is judged by the directory it resolves to — a root-owned symlink to the user's
    // real home must not let root through (review P1-2). The rest are never followed.
    let mut owners: Vec<(PathBuf, u32)> = dir_uid(paths.home())
        .map(|u| (paths.home().to_path_buf(), u))
        .into_iter()
        .collect();
    let mut rest = vec![paths.rein()];
    rest.extend(extra.iter().map(|p| p.to_path_buf()));
    owners.extend(rest.into_iter().filter_map(|p| uid_of(&p).map(|u| (p, u))));
    check_owners(me, &owners).map(|()| proof)
}

/// Proof that the [`owner_guard`] passed for one home while its [`InstallLock`] is held
/// (D-013 addendum). It has no public constructor and borrows the lock, so every
/// lock-held writer that takes one can only be reached through the guard, and only while
/// that lock lives.
///
/// ```compile_fail,E0061
/// // A writer cannot be called without the proof …
/// fn skip(p: &rein_release::InstallPaths, r: &rein_release::VerifiedRelease) {
///     let _ = rein_release::install_tree_locked(p, r);
/// }
/// ```
///
/// ```compile_fail,E0451
/// // … and the proof cannot be built outside the guard.
/// fn forge(home: std::path::PathBuf, lock: &rein_release::InstallLock) {
///     let _ = rein_release::OwnerChecked { _lock: lock, home };
/// }
/// ```
///
/// ```no_run
/// // The only way in:
/// fn guarded(p: &rein_release::InstallPaths, r: &rein_release::VerifiedRelease)
///     -> Result<(), rein_release::LayoutError>
/// {
///     let lock = rein_release::InstallLock::acquire(p)?;
///     let ok = rein_release::owner_guard(&lock, p, &[])?;
///     rein_release::install_tree_locked(p, r, &ok)?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct OwnerChecked<'l> {
    _lock: &'l InstallLock,
    home: PathBuf,
}

impl OwnerChecked<'_> {
    /// Refuse to act on a home the guard did not check.
    pub fn covers(&self, paths: &InstallPaths) -> Result<(), LayoutError> {
        if self.home == paths.home() {
            Ok(())
        } else {
            Err(LayoutError::Io(format!(
                "owner guard ran for {} but the write targets {}",
                self.home.display(),
                paths.home().display()
            )))
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
    let lock = InstallLock::acquire(paths)?;
    let checked = owner_guard(&lock, paths, &[])?;
    install_tree_locked(paths, release, &checked)
}

/// [`install_tree`] for a caller that holds the [`InstallLock`]; `checked` (from
/// [`owner_guard`]) proves the guard passed for this home.
pub fn install_tree_locked(
    paths: &InstallPaths,
    release: &VerifiedRelease,
    checked: &OwnerChecked<'_>,
) -> Result<InstallOutcome, LayoutError> {
    install_tree_with_faults(paths, release, checked, &Faults::none())
}

/// [`install_tree_locked`] with the test fault seam.
#[doc(hidden)]
pub fn install_tree_with_faults(
    paths: &InstallPaths,
    release: &VerifiedRelease,
    checked: &OwnerChecked<'_>,
    faults: &Faults,
) -> Result<InstallOutcome, LayoutError> {
    checked.covers(paths)?;
    let live = paths.rein();
    let live_exists = home_exists(&live)?;
    let manifest = &release.manifest;
    let layout = &manifest.layout;
    let matcher = manifest.operator_owned();
    let source: Vec<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();

    // a. preflight, b. plan
    sync::preflight(&live, &layout.mirrored, &source)?;
    let scan = if live_exists {
        sync::scan(&live, &layout.mirrored, matcher).map_err(|e| LayoutError::Io(e.to_string()))?
    } else {
        sync::Scan::default()
    };
    let plan = sync::plan(&source, &scan.entries, matcher);
    let skipped_operator_owned = source
        .iter()
        .filter(|p| !plan.ship.iter().any(|s| s == *p))
        .map(|p| p.to_string())
        .collect();

    // c. snapshot (only when an installed version exists)
    let unsnapshotted =
        live_exists && !scan.entries.is_empty() && installed_version(&live).is_none();
    let previous = match installed_version(&live) {
        Some(old) => Some(
            snapshot::take(paths, &live, &old, &scan.entries, layout)
                .map_err(|e| LayoutError::Snapshot(e.to_string()))?,
        ),
        None => None,
    };
    if !live_exists {
        fs::create_dir(&live).map_err(|e| LayoutError::Io(e.to_string()))?;
    }

    // d. sync, e. VERSION
    let modes: std::collections::HashMap<&str, u32> = manifest
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.mode.bits()))
        .collect();
    let items: Vec<ShipItem> = plan
        .ship
        .iter()
        .map(|rel| ShipItem {
            rel: rel.clone(),
            src: Source::File {
                from: release.tree.join(rel),
                mode: modes[rel.as_str()],
            },
        })
        .collect();
    let synced = sync::apply(&live, &layout.mirrored, &items, &plan, &scan.dirs, faults)
        .and_then(|()| sync::write_version(&live, &manifest.version.to_string(), faults));

    // f. restore on failure
    if let Err(e) = synced {
        let restored = !faults.skip_restore
            && previous
                .as_ref()
                .is_some_and(|s| sync_from_snapshot(&live, s).is_ok());
        return Err(LayoutError::Sync {
            reason: e.to_string(),
            snapshot: previous,
            restored,
        });
    }
    Ok(InstallOutcome {
        version: manifest.version.clone(),
        previous,
        pruned: snapshot::prune(paths),
        skipped_operator_owned,
        unsnapshotted,
    })
}

/// `$HOME/.rein`: `Ok(false)` if absent, `Ok(true)` if a real directory, else refuse.
pub(crate) fn home_exists(live: &Path) -> Result<bool, LayoutError> {
    match fs::symlink_metadata(live) {
        Ok(m) if m.file_type().is_symlink() => Err(LayoutError::Symlink(live.to_path_buf())),
        Ok(m) if m.is_dir() => Ok(true),
        Ok(_) => Err(LayoutError::NotADirectory(live.to_path_buf())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(LayoutError::Io(format!("{}: {e}", live.display()))),
    }
}

/// Sync the snapshot at `root` into `live` (failure restore and rollback). `VERSION` is
/// not touched.
pub(crate) fn sync_from_snapshot(
    live: &Path,
    root: &Path,
) -> Result<snapshot::Loaded, LayoutError> {
    let snap = snapshot::load(root)?;
    sync_loaded(live, &snap, &Faults::none())?;
    Ok(snap)
}

pub(crate) fn sync_loaded(
    live: &Path,
    snap: &snapshot::Loaded,
    faults: &Faults,
) -> Result<(), LayoutError> {
    let source = snap.rels();
    sync::preflight(live, &snap.layout.mirrored, &source)?;
    let io_err = |e: io::Error| LayoutError::Io(e.to_string());
    let scan = sync::scan(live, &snap.layout.mirrored, &snap.matcher).map_err(io_err)?;
    let plan = sync::plan(&source, &scan.entries, &snap.matcher);
    sync::apply(
        live,
        &snap.layout.mirrored,
        &snap.items,
        &plan,
        &scan.dirs,
        faults,
    )
    .map_err(io_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_siblings_of_the_rein_home() {
        let p = InstallPaths::new("/h");
        assert_eq!(p.rein(), Path::new("/h/.rein"));
        assert_eq!(p.lock(), Path::new("/h/.rein.lock"));
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

#[cfg(test)]
mod owner_tests {
    use super::*;

    #[test]
    fn a_proof_never_covers_a_home_whose_lock_is_not_held() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let lock_a = InstallLock::acquire(&InstallPaths::new(a.path())).unwrap();
        let err = owner_guard(&lock_a, &InstallPaths::new(b.path()), &[]);
        assert!(matches!(err, Err(LayoutError::Io(_))), "{err:?}");
        assert!(owner_guard(&lock_a, &InstallPaths::new(a.path()), &[]).is_ok());
    }

    #[test]
    fn owner_guard_comparison_names_the_first_foreign_path() {
        let home = (PathBuf::from("/h"), 1000);
        let rein = (PathBuf::from("/h/.rein"), 1000);
        let snap = (PathBuf::from("/h/.rein.prev-0.4.0"), 1000);
        assert!(check_owners(1000, &[home.clone(), rein.clone(), snap.clone()]).is_ok());
        let cases = [
            (vec![(PathBuf::from("/h"), 0), rein.clone()], "/h"),
            (
                vec![home.clone(), (PathBuf::from("/h/.rein"), 0)],
                "/h/.rein",
            ),
            (
                vec![
                    home.clone(),
                    rein.clone(),
                    (PathBuf::from("/h/.rein.prev-0.4.0"), 0),
                ],
                "/h/.rein.prev-0.4.0",
            ),
        ];
        for (owners, foreign) in cases {
            match check_owners(1000, &owners) {
                Err(LayoutError::NotOwner {
                    path,
                    run_uid: 1000,
                    owner_uid: 0,
                }) => {
                    assert_eq!(path, Path::new(foreign));
                }
                other => panic!("{foreign}: {other:?}"),
            }
        }
        // root running over a user's home is the case the guard exists for
        assert!(check_owners(0, &[home]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn the_run_uid_is_the_lock_files_owner_and_matches_a_home_we_created() {
        let dir = tempfile::tempdir().unwrap();
        let p = InstallPaths::new(dir.path());
        let lock = InstallLock::acquire(&p).unwrap();
        assert_eq!(lock.run_uid(), uid_of(dir.path()));
        assert!(owner_guard(&lock, &p, &[]).is_ok());
        let foreign: RunUid = |l| l.run_uid().map(|u| u.wrapping_add(1));
        assert!(matches!(
            owner_guard_with(&lock, foreign, &p, &[]),
            Err(LayoutError::NotOwner { .. })
        ));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod owner_symlink_tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn a_home_symlinked_to_someone_elses_directory_is_refused() {
        // /tmp is root-owned; a link to it that WE own must not pass as "our" home.
        let me = tempfile::NamedTempFile::new()
            .unwrap()
            .as_file()
            .metadata()
            .unwrap()
            .uid();
        if fs::metadata("/tmp").unwrap().uid() == me {
            eprintln!("SKIP: /tmp is owned by the test user");
            return;
        }
        // The rule, without a lock under /tmp (the CLI test owns that path): $HOME is
        // judged by the directory it resolves to, not by the link.
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("home");
        std::os::unix::fs::symlink("/tmp", &link).unwrap();
        assert_eq!(uid_of(&link), Some(me), "the link is ours");
        assert_eq!(dir_uid(&link), Some(fs::metadata("/tmp").unwrap().uid()));
        let owners = [(link.clone(), dir_uid(&link).unwrap())];
        assert!(matches!(
            check_owners(me, &owners),
            Err(LayoutError::NotOwner { ref path, .. }) if path == &link
        ));
    }

    #[test]
    fn the_run_uid_survives_a_rename_over_the_lock_path() {
        let dir = tempfile::tempdir().unwrap();
        let p = InstallPaths::new(dir.path());
        let lock = InstallLock::acquire(&p).unwrap();
        let mine = lock.run_uid();
        let planted = dir.path().join("planted");
        fs::write(&planted, "x").unwrap();
        fs::rename(&planted, p.lock()).unwrap();
        assert_eq!(lock.run_uid(), mine, "read from the handle, not the path");
    }
}
