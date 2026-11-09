//! The in-place, per-file sync of the mirrored directories (blueprint §Components 3 steps
//! a, b, d; D-011). Shared by install (source = verified release) and by rollback and
//! failure restore (source = a snapshot).
//!
//! Scope rule: only paths under a `layout.mirrored` directory are ever opened. Inside them,
//! paths matching `layout.operator_owned` are never opened: an owned directory is not
//! descended into, an owned file is neither read, written nor pruned.

use std::cell::Cell;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::layout::LayoutError;
use crate::matcher::OperatorOwnedMatcher;

/// What one sync writes and removes (Data Model `SyncPlan`). Pure function of the source's
/// file list and the live mirrored dirs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncPlan {
    /// Source paths to write (operator-owned paths removed).
    pub ship: Vec<String>,
    /// Live files/symlinks under mirrored dirs that are neither shipped nor owned.
    pub prune: Vec<String>,
}

/// Is `path`, or any directory above it, operator-owned? The last segment is tested both
/// as a file and as a directory: a symlink at a directory-only pattern
/// (`mcp/x/node_modules -> /shared/store`) is owned too (E-7 security review P1-1).
pub(crate) fn owned(m: &OperatorOwnedMatcher, path: &str) -> bool {
    if m.matches(path, false) || m.matches(path, true) {
        return true;
    }
    let mut end = 0;
    while let Some(i) = path[end..].find('/') {
        end += i;
        if m.matches(&path[..end], true) {
            return true;
        }
        end += 1;
    }
    false
}

/// Step b (pure).
pub fn plan(source: &[&str], live_entries: &[String], m: &OperatorOwnedMatcher) -> SyncPlan {
    let ship: Vec<String> = source
        .iter()
        .filter(|p| !owned(m, p))
        .map(|p| p.to_string())
        .collect();
    let shipped: std::collections::HashSet<&str> = ship.iter().map(String::as_str).collect();
    let prune = live_entries
        .iter()
        .filter(|e| !shipped.contains(e.as_str()) && !owned(m, e))
        .cloned()
        .collect();
    SyncPlan { ship, prune }
}

/// The live mirrored dirs, listed without opening any owned path.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    /// Files, symlinks and special files (relative paths).
    pub entries: Vec<String>,
    /// Non-owned directories, parents before children (mirrored roots included).
    pub dirs: Vec<String>,
}

pub(crate) fn scan(live: &Path, mirrored: &[String], m: &OperatorOwnedMatcher) -> io::Result<Scan> {
    let mut out = Scan::default();
    for root in mirrored {
        match fs::symlink_metadata(live.join(root)) {
            Ok(meta) if meta.is_dir() => {
                out.dirs.push(root.clone());
                walk(live, root, m, &mut out)?;
            }
            Ok(_) | Err(_) => {} // absent; preflight refused anything else
        }
    }
    Ok(out)
}

fn walk(live: &Path, rel: &str, m: &OperatorOwnedMatcher, out: &mut Scan) -> io::Result<()> {
    for entry in fs::read_dir(live.join(rel))? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue }; // never a manifest path; left alone
        let child = format!("{rel}/{name}");
        let kind = entry.file_type()?;
        // Same rule as `owned()`: a non-directory is tested as a file AND as a directory, so
        // a symlink or a stray file where an owned directory belongs is never opened, copied
        // into a snapshot or pruned (E-7 security review P1-1 and its follow-up).
        let is_owned = if kind.is_dir() {
            m.matches(&child, true)
        } else {
            m.matches(&child, false) || m.matches(&child, true)
        };
        if is_owned {
            continue; // owned: never opened
        }
        if kind.is_dir() {
            out.dirs.push(child.clone());
            walk(live, &child, m, out)?;
        } else {
            out.entries.push(child);
        }
    }
    Ok(())
}

/// Step a: refuse before any write. `$HOME/.rein` itself was checked by the caller. A
/// symlinked mirrored dir or a symlink at an intermediate component is refused; a symlink
/// at the target itself is replaced by the rename (it is never written through).
pub(crate) fn preflight(
    live: &Path,
    mirrored: &[String],
    ship: &[&str],
) -> Result<(), LayoutError> {
    for root in mirrored {
        let p = live.join(root);
        match fs::symlink_metadata(&p) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(LayoutError::Symlink(p)),
            Ok(meta) if !meta.is_dir() => return Err(LayoutError::NotADirectory(p)),
            _ => {}
        }
    }
    for rel in ship {
        if rel.ends_with(".rein-tmp") {
            // Would collide with the writer's own temp names (security review P2-4).
            return Err(LayoutError::Conflict(live.join(rel)));
        }
        let segs: Vec<&str> = rel.split('/').collect();
        let mut p = live.to_path_buf();
        for (i, seg) in segs.iter().enumerate() {
            p.push(seg);
            let last = i + 1 == segs.len();
            match fs::symlink_metadata(&p) {
                Err(_) => break, // the rest does not exist yet
                // A symlink AT the target is replaced by the rename, never written through
                // (security review P1-2); one above it would redirect the write — refuse.
                Ok(meta) if meta.file_type().is_symlink() && last => {}
                Ok(meta) if meta.file_type().is_symlink() => return Err(LayoutError::Symlink(p)),
                Ok(meta) if last && meta.is_dir() => return Err(LayoutError::Conflict(p)),
                Ok(meta) if !last && !meta.is_dir() => return Err(LayoutError::Conflict(p)),
                Ok(_) => {}
            }
        }
    }
    Ok(())
}

/// Where a shipped path's content comes from.
#[derive(Debug, Clone)]
pub(crate) enum Source {
    File { from: PathBuf, mode: u32 },
    Symlink(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) struct ShipItem {
    pub rel: String,
    pub src: Source,
}

/// Test seam: fail the k-th mutating step (write, prune, `VERSION`). Never set in production.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct Faults {
    fail_at: Option<usize>,
    count: Cell<usize>,
    /// Leave the tree as the fault left it (no restore), to observe a stop point.
    pub(crate) skip_restore: bool,
}

impl Faults {
    pub fn none() -> Self {
        Self::default()
    }
    /// Fail the `k`-th mutating step (1-based).
    pub fn fail_at(k: usize) -> Self {
        Self {
            fail_at: Some(k),
            count: Cell::new(0),
            skip_restore: false,
        }
    }
    /// Like [`Self::fail_at`], but the failed install is not restored.
    pub fn stop_at(k: usize) -> Self {
        Self {
            skip_restore: true,
            ..Self::fail_at(k)
        }
    }
    /// Mutating steps attempted so far.
    pub fn steps(&self) -> usize {
        self.count.get()
    }
    pub(crate) fn tick(&self) -> io::Result<()> {
        let n = self.count.get() + 1;
        self.count.set(n);
        if self.fail_at == Some(n) {
            return Err(io::Error::other(format!("injected fault at step {n}")));
        }
        Ok(())
    }
}

/// Step d: mirrored dirs in layout order with `hooks` last; per dir, write its shipped
/// files (temp beside the target, mode, rename), remove its prune entries, then remove
/// directories left empty.
pub(crate) fn apply(
    live: &Path,
    mirrored: &[String],
    items: &[ShipItem],
    plan: &SyncPlan,
    dirs: &[String],
    faults: &Faults,
) -> io::Result<()> {
    let mut order: Vec<&String> = mirrored.iter().filter(|m| *m != "hooks").collect();
    order.extend(mirrored.iter().filter(|m| *m == "hooks"));
    let under = |p: &str, root: &str| p.split('/').next() == Some(root);
    for root in order {
        for item in items.iter().filter(|i| under(&i.rel, root)) {
            write_item(live, item, faults)?;
        }
        for rel in plan.prune.iter().filter(|p| under(p, root)) {
            faults.tick()?;
            match fs::remove_file(live.join(rel)) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        // Deepest first; a non-empty directory simply stays.
        for d in dirs.iter().rev().filter(|d| under(d, root)) {
            let _ = fs::remove_dir(live.join(d));
        }
    }
    Ok(())
}

fn temp_beside(target: &Path) -> PathBuf {
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!(".{name}.rein-tmp"))
}

/// Remove a stale temp of ours, then create it fresh.
fn fresh(tmp: &Path) -> io::Result<()> {
    match fs::symlink_metadata(tmp) {
        Ok(m) if m.is_dir() => Err(io::Error::other(format!(
            "{} is a directory",
            tmp.display()
        ))),
        Ok(_) => fs::remove_file(tmp),
        Err(_) => Ok(()),
    }
}

fn write_item(live: &Path, item: &ShipItem, faults: &Faults) -> io::Result<()> {
    let target = live.join(&item.rel);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_beside(&target);
    match &item.src {
        Source::File { from, mode } => {
            if same_file(from, &target, *mode)? {
                return Ok(());
            }
            faults.tick()?;
            fresh(&tmp)?;
            let result = (|| {
                let mut out = File::create_new(&tmp)?;
                io::copy(&mut File::open(from)?, &mut out)?;
                set_mode(&out, *mode)?;
                out.sync_all()?;
                drop(out);
                fs::rename(&tmp, &target)
            })();
            if result.is_err() {
                let _ = fs::remove_file(&tmp);
            }
            result
        }
        Source::Symlink(to) => {
            if fs::read_link(&target).is_ok_and(|t| &t == to) {
                return Ok(());
            }
            faults.tick()?;
            fresh(&tmp)?;
            make_symlink(to, &tmp)?;
            fs::rename(&tmp, &target).inspect_err(|_| {
                let _ = fs::remove_file(&tmp);
            })
        }
    }
}

/// Is `target` a regular file with `from`'s bytes and `mode` already? Then it is skipped.
fn same_file(from: &Path, target: &Path, mode: u32) -> io::Result<bool> {
    let Ok(meta) = fs::symlink_metadata(target) else {
        return Ok(false);
    };
    if !meta.is_file() || meta.len() != fs::metadata(from)?.len() || file_mode(&meta) != mode {
        return Ok(false);
    }
    Ok(fs::read(from)? == fs::read(target)?)
}

/// Step e: `VERSION` last, temp-and-rename. The only path outside the mirror ever written.
pub(crate) fn write_version(live: &Path, version: &str, faults: &Faults) -> io::Result<()> {
    faults.tick()?;
    let tmp = live.join(".VERSION.rein-tmp");
    fresh(&tmp)?;
    let mut f = File::create_new(&tmp)?;
    io::Write::write_all(&mut f, format!("{version}\n").as_bytes())?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, live.join("VERSION"))
}

#[cfg(unix)]
pub(crate) fn file_mode(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
pub(crate) fn file_mode(_meta: &fs::Metadata) -> u32 {
    0o644
}

#[cfg(unix)]
fn set_mode(f: &File, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    f.set_permissions(fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_f: &File, _mode: u32) -> io::Result<()> {
    Ok(()) // Windows runs rein inside WSL (Component 8)
}

#[cfg(unix)]
pub(crate) fn make_symlink(to: &Path, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(to, at)
}

#[cfg(not(unix))]
pub(crate) fn make_symlink(_to: &Path, at: &Path) -> io::Result<()> {
    Err(io::Error::other(format!(
        "{}: symlinks are not supported on this platform",
        at.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher() -> OperatorOwnedMatcher {
        OperatorOwnedMatcher::new(&["config/rein-workspace.txt", "mcp/*/node_modules/"]).unwrap()
    }

    #[test]
    fn plan_ships_unowned_files_and_prunes_unshipped_unowned_ones() {
        let live = [
            "bin/rein".to_string(),
            "bin/old-tool".into(),
            "config/rein-workspace.txt".into(),
            "mcp/x/node_modules/a.js".into(),
        ];
        let p = plan(
            &[
                "bin/rein",
                "config/rein-workspace.txt",
                "mcp/x/node_modules/b.js",
                "hooks/h.sh",
            ],
            &live,
            &matcher(),
        );
        assert_eq!(p.ship, ["bin/rein", "hooks/h.sh"]);
        assert_eq!(p.prune, ["bin/old-tool"]);
    }

    #[test]
    fn the_kth_step_fails_and_counting_continues() {
        let f = Faults::fail_at(2);
        assert!(f.tick().is_ok());
        assert!(f.tick().is_err());
        assert!(f.tick().is_ok());
        assert_eq!(f.steps(), 3);
        assert!(Faults::none().tick().is_ok());
    }
}
