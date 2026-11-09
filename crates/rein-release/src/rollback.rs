//! `rein-app rollback` (blueprint §Components 7, §Rollback Plan; D-011): take a snapshot of
//! the current payload, sync the newest earlier snapshot back with the LayoutWriter
//! routine, write its `VERSION`, remove the snapshot just restored — so a second rollback
//! returns to the newer version. Restores **code only**: nothing outside the mirrored dirs,
//! and no operator-owned path inside them, is ever opened.

use std::fs;
use std::path::PathBuf;

use semver::Version;

use crate::layout::{
    InstallLock, InstallPaths, LayoutError, RunUid, home_exists, installed_version, owner_guard,
    sync_loaded,
};
use crate::snapshot;
use crate::sync::{self, Faults};

/// What a rollback did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackOutcome {
    /// Version now live.
    pub restored: Version,
    /// Snapshot of the payload that was live before (`None` if it had no valid `VERSION`).
    pub snapshot: Option<PathBuf>,
}

/// Rolling back failed.
#[derive(Debug)]
pub enum RollbackError {
    /// No `$HOME/.rein.prev-*` exists (CLI exit 12).
    NoPrevious,
    Layout(LayoutError),
}

impl std::fmt::Display for RollbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RollbackError::NoPrevious => {
                write!(
                    f,
                    "rollback: no snapshot ($HOME/.rein.prev-*) to roll back to"
                )
            }
            RollbackError::Layout(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RollbackError {}

impl From<LayoutError> for RollbackError {
    fn from(e: LayoutError) -> Self {
        RollbackError::Layout(e)
    }
}

/// Take the install lock and roll back to the newest snapshot. The caller keeps the
/// returned lock while it runs `rein doctor`.
pub fn rollback_tree(
    paths: &InstallPaths,
) -> Result<(RollbackOutcome, InstallLock), RollbackError> {
    rollback_tree_with(paths, InstallLock::run_uid)
}

/// [`rollback_tree`] with the owner guard's uid source injectable (tests).
#[doc(hidden)]
pub fn rollback_tree_with(
    paths: &InstallPaths,
    run_uid: RunUid,
) -> Result<(RollbackOutcome, InstallLock), RollbackError> {
    let lock = InstallLock::acquire(paths)?;
    owner_guard(&lock, run_uid, paths, &[])?;
    let io_err = |e: std::io::Error| RollbackError::Layout(LayoutError::Io(e.to_string()));
    // Chosen before the new snapshot below exists, so it is the newest *earlier* one.
    let target = paths
        .previous_trees()
        .map_err(io_err)?
        .into_iter()
        .next()
        .ok_or(RollbackError::NoPrevious)?;
    // The snapshot steers what rollback writes: it must be ours too (D-013).
    owner_guard(
        &lock,
        run_uid,
        paths,
        &[&target, &target.join("layout.json")],
    )?;
    let snap = snapshot::load(&target)?;
    let live = paths.rein();
    let live_exists = home_exists(&live)?;
    let source = snap.rels();
    sync::preflight(&live, &snap.layout.mirrored, &source)?;

    let current = match installed_version(&live) {
        Some(v) if live_exists => {
            let scan = sync::scan(&live, &snap.layout.mirrored, &snap.matcher).map_err(io_err)?;
            Some(
                snapshot::take(paths, &live, &v, &scan.entries, &snap.layout)
                    .map_err(|e| LayoutError::Snapshot(e.to_string()))?,
            )
        }
        _ => None,
    };
    if !live_exists {
        fs::create_dir(&live).map_err(io_err)?;
    }

    let synced = sync_loaded(&live, &snap, &Faults::none()).and_then(|()| {
        sync::write_version(&live, &snap.version.to_string(), &Faults::none())
            .map_err(|e| LayoutError::Io(e.to_string()))
    });
    if let Err(e) = synced {
        let restored = current
            .as_ref()
            .is_some_and(|s| crate::layout::sync_from_snapshot(&live, s).is_ok());
        return Err(RollbackError::Layout(LayoutError::Sync {
            reason: e.to_string(),
            snapshot: current,
            restored,
        }));
    }
    let _ = fs::remove_dir_all(&snap.root);
    snapshot::prune(paths);
    Ok((
        RollbackOutcome {
            restored: snap.version,
            snapshot: current,
        },
        lock,
    ))
}
