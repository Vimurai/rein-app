//! `rein-app rollback` (blueprint §Components 7, §Rollback Plan): swap `$HOME/.rein` with
//! the newest `$HOME/.rein.prev-*`. The rolled-back-from tree becomes a previous tree, so
//! a second rollback swaps back. Operator files come with the tree they live in: what was
//! written since the restored tree was replaced stays in the tree swapped out.

use std::fs;
use std::io;
use std::path::PathBuf;

use semver::Version;

use crate::layout::{InstallLock, InstallPaths, LayoutError, free_prev_slot, installed_version};

/// What a rollback did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackOutcome {
    /// Version now live (`None` if its `VERSION` is missing or not semver).
    pub restored: Option<Version>,
    /// Where the tree that was live now lives (`None` if there was no live tree).
    pub swapped_out: Option<PathBuf>,
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
                write!(f, "rollback: no previous tree ($HOME/.rein.prev-*)")
            }
            RollbackError::Layout(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RollbackError {}

/// Take the install lock and swap in the newest previous tree. The caller keeps the
/// returned lock while it runs `rein doctor`.
pub fn rollback_tree(
    paths: &InstallPaths,
) -> Result<(RollbackOutcome, InstallLock), RollbackError> {
    let lock = InstallLock::acquire(paths).map_err(RollbackError::Layout)?;
    let io_err = |e: io::Error| RollbackError::Layout(LayoutError::Staging(e.to_string()));
    let target = paths
        .previous_trees()
        .map_err(io_err)?
        .into_iter()
        .next()
        .ok_or(RollbackError::NoPrevious)?;

    let live = paths.rein();
    let live_exists = match fs::symlink_metadata(&live) {
        Ok(m) if m.is_dir() => true,
        Ok(_) => return Err(RollbackError::Layout(LayoutError::NotADirectory(live))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(io_err(e)),
    };

    let mut swapped_out = None;
    if live_exists {
        let label = installed_version(&live).map_or_else(|| "unknown".into(), |v| v.to_string());
        let slot = free_prev_slot(paths, &label);
        fs::rename(&live, &slot).map_err(|e| {
            RollbackError::Layout(LayoutError::Swap {
                reason: format!("moving {} aside: {e}", live.display()),
                staging: target.clone(),
                restored: true,
            })
        })?;
        swapped_out = Some(slot);
    }
    if let Err(e) = fs::rename(&target, &live) {
        let restored = swapped_out
            .as_ref()
            .is_none_or(|s| fs::rename(s, &live).is_ok());
        return Err(RollbackError::Layout(LayoutError::Swap {
            reason: format!("renaming {} into place: {e}", target.display()),
            staging: target,
            restored,
        }));
    }
    Ok((
        RollbackOutcome {
            restored: installed_version(&live),
            swapped_out,
        },
        lock,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn tree(path: &std::path::Path, version: &str, age_days: u64) {
        fs::create_dir_all(path).unwrap();
        let v = path.join("VERSION");
        fs::write(&v, format!("{version}\n")).unwrap();
        let f = fs::File::options().write(true).open(&v).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(age_days * 86_400))
            .unwrap();
    }

    #[test]
    fn swaps_in_the_newest_previous_tree_and_back_again() {
        let home = tempfile::tempdir().unwrap();
        let p = InstallPaths::new(home.path());
        tree(&p.prev("0.3.0"), "0.3.0", 20);
        tree(&p.prev("0.4.0"), "0.4.0", 10);
        tree(&p.rein(), "0.5.0", 0);

        let (out, lock) = rollback_tree(&p).unwrap();
        assert_eq!(out.restored, Some(Version::new(0, 4, 0)));
        assert_eq!(out.swapped_out, Some(p.prev("0.5.0")));
        assert!(p.lock().exists(), "lock held for the caller's rein doctor");
        drop(lock);

        let (again, _lock) = rollback_tree(&p).unwrap();
        assert_eq!(again.restored, Some(Version::new(0, 5, 0)), "toggles back");
    }

    #[test]
    fn no_previous_tree_is_its_own_error_and_changes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let p = InstallPaths::new(home.path());
        tree(&p.rein(), "0.5.0", 0);
        assert!(matches!(rollback_tree(&p), Err(RollbackError::NoPrevious)));
        assert!(p.rein().join("VERSION").exists());
        assert!(!p.lock().exists());
    }

    #[test]
    fn restores_a_previous_tree_when_the_live_one_is_missing() {
        let home = tempfile::tempdir().unwrap();
        let p = InstallPaths::new(home.path());
        tree(&p.prev("0.4.0"), "0.4.0", 1);
        let (out, _lock) = rollback_tree(&p).unwrap();
        assert_eq!(out.restored, Some(Version::new(0, 4, 0)));
        assert_eq!(out.swapped_out, None);
    }
}
