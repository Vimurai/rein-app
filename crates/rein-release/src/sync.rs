//! The in-place, per-file sync of the mirrored directories (blueprint §Components 3 steps
//! a, b, d; D-011). Shared by install (source = verified release) and by rollback and
//! failure restore (source = a snapshot).
//!
//! Scope rule: only paths under a `layout.mirrored` directory (plus `manifest.json` and
//! `VERSION`, handled by the callers) are ever opened. Inside them, `layout.operator_owned`
//! protects from deletion only: a shipped path is always written, even when a pattern
//! matches it; an owned path the release does not ship is never opened, and prune never
//! descends into an owned directory (rein D-155 r2).

use std::cell::Cell;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::layout::LayoutError;
use crate::matcher::OperatorOwnedMatcher;

/// What one sync writes and removes (Data Model `SyncPlan`). Pure function of the source's
/// file list and the live mirrored dirs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncPlan {
    /// Source paths to write — every one, owned or not (a shipped path wins).
    pub ship: Vec<String>,
    /// Live files/symlinks under mirrored dirs that are neither shipped nor owned, removed
    /// after their directory's writes.
    pub prune: Vec<String>,
    /// Prune entries that are a case variant of a shipped path or of a directory above one
    /// (`shared/locate.sh` when `shared/Locate.sh` ships). Removed BEFORE their directory's
    /// writes: on a case-insensitive home both names are one file, and a later prune would
    /// delete the file just written (D-021).
    pub prune_first: Vec<String>,
}

/// Every directory prefix of `path`, then `path` itself.
fn with_prefixes(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/')
        .map(|(i, _)| &path[..i])
        .chain(std::iter::once(path))
}

/// The shipped paths, ASCII-lowercased once each (D-021); [`folded_set`] borrows from it.
fn folded_ship(ship: &[String]) -> Vec<String> {
    ship.iter().map(|p| p.to_ascii_lowercase()).collect()
}

/// Every lowercased shipped path and its directory prefixes, as slices of `lower`: no
/// per-prefix copies, so memory is linear in the input (E-31).
fn folded_set(lower: &[String]) -> HashSet<&str> {
    lower.iter().flat_map(|p| with_prefixes(p)).collect()
}

/// Is `path`, or any directory above it, operator-owned? The last segment is tested both
/// as a file and as a directory: a symlink at a directory-only pattern
/// (`mcp/x/node_modules -> /shared/store`) is owned too (E-7 security review P1-1).
pub(crate) fn owned(m: &OperatorOwnedMatcher, path: &str) -> bool {
    m.owned_pattern(path, false).is_some()
}

/// Step b (pure). Every source path is shipped — a shipped path wins over an
/// `operator_owned` pattern, which protects from deletion only (rein D-155 r2).
pub fn plan(source: &[&str], live_entries: &[String], m: &OperatorOwnedMatcher) -> SyncPlan {
    let ship: Vec<String> = source.iter().map(|p| p.to_string()).collect();
    let shipped: std::collections::HashSet<&str> = ship.iter().map(String::as_str).collect();
    let lower = folded_ship(&ship);
    let folded = folded_set(&lower);
    let (prune_first, prune) = live_entries
        .iter()
        .filter(|e| !shipped.contains(e.as_str()) && !owned(m, e))
        .cloned()
        .partition(|e| folded.contains(e.to_ascii_lowercase().as_str()));
    SyncPlan {
        ship,
        prune,
        prune_first,
    }
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
    // manifest.json and VERSION are replaced by rename: a directory there would fail only
    // after the sync, so refuse it now (E-13 review P2-1). A symlink is replaced, not followed.
    for top in ["manifest.json", "VERSION"] {
        let p = live.join(top);
        if fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
            return Err(LayoutError::Conflict(p));
        }
    }
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
    // D-023: a symlink named as a case variant of a shipped path IS that path on a
    // case-insensitive home (and would make the snapshot fail later); refused on every
    // filesystem, naming the link.
    // Per directory: the shipped names as written and lowercased, so each live entry is two
    // set lookups, not a comparison with every shipped name (E-31).
    #[derive(Default)]
    struct Names<'a> {
        exact: HashSet<&'a str>,
        lower: HashSet<String>,
    }
    let mut by_parent: std::collections::HashMap<&str, Names<'_>> = Default::default();
    for rel in ship {
        if let Some((dir, name)) = rel.rsplit_once('/') {
            let names = by_parent.entry(dir).or_default();
            names.exact.insert(name);
            names.lower.insert(name.to_ascii_lowercase());
        }
    }
    for (dir, names) in by_parent {
        let Ok(entries) = fs::read_dir(live.join(dir)) else {
            continue; // absent: nothing there yet
        };
        for entry in entries.flatten() {
            let Ok(found) = entry.file_name().into_string() else {
                continue;
            };
            let variant = !names.exact.contains(found.as_str())
                && names.lower.contains(&found.to_ascii_lowercase());
            if variant && entry.file_type().is_ok_and(|t| t.is_symlink()) {
                return Err(LayoutError::CaseVariantSymlink(live.join(dir).join(found)));
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

/// Step d: mirrored dirs in layout order with `hooks` last; per dir, remove the case
/// variants of shipped paths and the directories they leave empty (D-021), write its
/// shipped files (temp beside the target, mode, rename), remove its other prune entries,
/// then remove directories left empty. Last, every shipped path must be present (D-021).
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
    let lower = folded_ship(&plan.ship);
    let folded = folded_set(&lower);
    let exact: HashSet<&str> = plan.ship.iter().flat_map(|p| with_prefixes(p)).collect();
    let case_variant_dir =
        |d: &str| folded.contains(d.to_ascii_lowercase().as_str()) && !exact.contains(d);
    for root in order {
        for rel in plan.prune_first.iter().filter(|p| under(p, root)) {
            remove(live, rel, faults)?;
        }
        for d in dirs
            .iter()
            .rev()
            .filter(|d| under(d, root) && case_variant_dir(d))
        {
            let _ = fs::remove_dir(live.join(d)); // non-empty: it simply stays
        }
        for item in items.iter().filter(|i| under(&i.rel, root)) {
            write_item(live, item, faults)?;
        }
        for rel in plan.prune.iter().filter(|p| under(p, root)) {
            remove(live, rel, faults)?;
        }
        // Deepest first; a non-empty directory simply stays.
        for d in dirs.iter().rev().filter(|d| under(d, root)) {
            let _ = fs::remove_dir(live.join(d));
        }
    }
    check_shipped(live, items)
}

fn remove(live: &Path, rel: &str, faults: &Faults) -> io::Result<()> {
    faults.tick()?;
    match fs::remove_file(live.join(rel)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Post-sync check (D-021): every shipped path is present, a regular file of its source's
/// size (a symlink for a symlink source). A miss fails the sync, which the callers restore,
/// so no future ordering bug can end an update at exit 0 with a shipped file missing.
fn check_shipped(live: &Path, items: &[ShipItem]) -> io::Result<()> {
    for item in items {
        let meta = fs::symlink_metadata(live.join(&item.rel));
        let present = match (&item.src, meta) {
            (Source::File { from, .. }, Ok(m)) => {
                m.is_file() && m.len() == fs::metadata(from)?.len()
            }
            (Source::Symlink(_), Ok(m)) => m.file_type().is_symlink(),
            (_, Err(_)) => false,
        };
        if !present {
            return Err(io::Error::other(format!(
                "{}: shipped but not present after the sync",
                item.rel
            )));
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

/// Step e: the in-tarball `manifest.json`, byte for byte, to `<live>/manifest.json`
/// (temp-and-rename), after `hooks` and before `VERSION` (rein D-155 r1/r4).
pub(crate) fn write_manifest(live: &Path, bytes: &[u8], faults: &Faults) -> io::Result<()> {
    faults.tick()?;
    let tmp = live.join(".manifest.json.rein-tmp");
    fresh(&tmp)?;
    let result = (|| {
        let mut f = File::create_new(&tmp)?;
        io::Write::write_all(&mut f, bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, live.join("manifest.json"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Remove `<live>/manifest.json` (a restore to a tree that had none). Never follows a link.
pub(crate) fn remove_manifest(live: &Path) -> io::Result<()> {
    match fs::remove_file(live.join("manifest.json")) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Step e: `VERSION` last, temp-and-rename. With `manifest.json`, the only paths outside
/// the mirror ever written.
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
    fn a_shipped_path_wins_and_only_unshipped_unowned_files_are_pruned() {
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
        assert_eq!(
            p.ship,
            [
                "bin/rein",
                "config/rein-workspace.txt",
                "mcp/x/node_modules/b.js",
                "hooks/h.sh"
            ],
            "every shipped path is written, owned or not (rein D-155 r2)"
        );
        assert_eq!(p.prune, ["bin/old-tool"], "owned a.js is kept");
        assert!(p.prune_first.is_empty());
    }

    #[test]
    fn case_variants_of_a_shipped_path_or_its_directories_are_pruned_first() {
        let live = [
            "shared/locate.sh".to_string(),
            "shared/Lib/x.mjs".into(),
            "shared/Lib".into(),
            "shared/Lib/y.mjs".into(),
            "shared/old.sh".into(),
        ];
        let p = plan(&["shared/Locate.sh", "shared/lib/x.mjs"], &live, &matcher());
        assert_eq!(
            p.prune_first,
            ["shared/locate.sh", "shared/Lib/x.mjs", "shared/Lib"]
        );
        assert_eq!(
            p.prune,
            ["shared/Lib/y.mjs", "shared/old.sh"],
            "no case variant"
        );
    }

    /// The post-sync check catches what the D-021 ordering prevents: a plan (built by hand,
    /// as the old ordering produced on a case-insensitive home) that prunes a shipped path.
    #[test]
    fn a_shipped_path_missing_after_the_sync_fails_it() {
        let dir = tempfile::tempdir().unwrap();
        let (live, src) = (dir.path().join("live"), dir.path().join("src"));
        fs::create_dir_all(src.join("bin")).unwrap();
        fs::write(src.join("bin/rein"), "new").unwrap();
        fs::create_dir_all(&live).unwrap();
        let mirrored = ["bin".to_string()];
        let items = [ShipItem {
            rel: "bin/rein".into(),
            src: Source::File {
                from: src.join("bin/rein"),
                mode: 0o755,
            },
        }];
        let ok = SyncPlan {
            ship: vec!["bin/rein".into()],
            ..SyncPlan::default()
        };
        apply(&live, &mirrored, &items, &ok, &[], &Faults::none()).unwrap();
        let bad = SyncPlan {
            prune: vec!["bin/rein".into()],
            ..ok
        };
        let err = apply(&live, &mirrored, &items, &bad, &[], &Faults::none()).unwrap_err();
        assert!(
            err.to_string()
                .contains("bin/rein: shipped but not present"),
            "{err}"
        );
        fs::write(live.join("bin/rein"), "longer than new").unwrap();
        let wrong_size = check_shipped(&live, &items).unwrap_err();
        assert!(wrong_size.to_string().contains("bin/rein"), "{wrong_size}");
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
