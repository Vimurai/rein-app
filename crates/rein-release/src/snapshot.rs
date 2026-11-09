//! Snapshots of the release payload (blueprint §Components 3 step c, §Data Model
//! `Snapshot`; D-011): `$HOME/.rein.prev-<ver>[~N]/{tree/, VERSION, layout.json}`, mode
//! 0700, built under a temp name and renamed when complete. Holds only the non-owned
//! content of the mirrored dirs — never operator data, nothing from outside the mirror.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::Deserialize;

use crate::layout::{InstallPaths, LayoutError, free_prev_slot};
use crate::manifest::{Layout, RawLayout, validate_layout};
use crate::matcher::OperatorOwnedMatcher;
use crate::paths::check_rel_path;
use crate::sync::{ShipItem, Source, file_mode, make_symlink, owned};

/// Snapshots kept beside the live tree (blueprint: N=2).
pub const KEEP_SNAPSHOTS: usize = 2;

const TMP_NAME: &str = ".rein.snapshot-tmp";

/// Copy `entries` (non-owned paths under the live mirrored dirs) into a new snapshot of
/// `version`. Returns its root.
pub(crate) fn take(
    paths: &InstallPaths,
    live: &Path,
    version: &Version,
    entries: &[String],
    ship: &[&str],
    layout: &Layout,
    matcher: &OperatorOwnedMatcher,
) -> io::Result<PathBuf> {
    // The non-owned payload, plus the live file at every shipped path — a shipped path a
    // pattern also matches (mcp/x/package-lock.json) must be restorable (rein D-155 r2).
    let mut all: Vec<String> = entries.to_vec();
    for rel in ship {
        let present = fs::symlink_metadata(live.join(rel)).is_ok_and(|m| !m.is_dir());
        if present && !all.iter().any(|e| e == rel) {
            all.push(rel.to_string());
        }
    }
    let shipped_owned: Vec<&String> = all.iter().filter(|e| owned(matcher, e)).collect();
    let tmp = paths.home().join(TMP_NAME);
    match fs::symlink_metadata(&tmp) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(&tmp)?, // ours by name, under the lock
        Ok(_) => {
            return Err(io::Error::other(format!(
                "{} is not a directory",
                tmp.display()
            )));
        }
        Err(_) => {}
    }
    create_private_dir(&tmp)?;
    let result = (|| {
        let tree = tmp.join("tree");
        fs::create_dir(&tree)?;
        // A temp file left by a killed run is not payload; the sync prunes it.
        for rel in all.iter().filter(|r| !r.ends_with(".rein-tmp")) {
            let from = live.join(rel);
            let to = tree.join(rel);
            let meta = fs::symlink_metadata(&from)?;
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            if meta.file_type().is_symlink() {
                make_symlink(&fs::read_link(&from)?, &to)?;
            } else if meta.is_file() {
                fs::copy(&from, &to)?; // the mode comes along
            } // sockets/FIFOs under a mirrored dir are not release payload
        }
        fs::write(tmp.join("VERSION"), format!("{version}\n"))?;
        // The home's manifest.json goes with the payload it describes (never followed).
        if fs::symlink_metadata(live.join("manifest.json")).is_ok_and(|m| m.is_file()) {
            fs::copy(live.join("manifest.json"), tmp.join("manifest.json"))?;
        }
        let json = serde_json::json!({
            "mirrored": layout.mirrored,
            "operator_owned": layout.operator_owned,
            "shipped_owned": shipped_owned,
        });
        fs::write(tmp.join("layout.json"), serde_json::to_vec_pretty(&json)?)?;
        let slot = free_prev_slot(paths, &version.to_string());
        fs::rename(&tmp, &slot)?;
        Ok(slot)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&tmp);
    }
    result
}

/// Remove all but the [`KEEP_SNAPSHOTS`] newest snapshots; returns what was removed.
pub(crate) fn prune(paths: &InstallPaths) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    if let Ok(all) = paths.previous_trees() {
        for old in all.into_iter().skip(KEEP_SNAPSHOTS) {
            if fs::remove_dir_all(&old).is_ok() {
                removed.push(old);
            }
        }
    }
    removed
}

/// `layout.json` of a snapshot: the layout used, plus the owned-matching paths it holds
/// because they were shipped (only those may be restored over an owned path).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotLayout {
    mirrored: Vec<String>,
    operator_owned: Vec<String>,
    #[serde(default)]
    shipped_owned: Vec<String>,
}

/// A snapshot read back and validated.
#[derive(Debug)]
pub(crate) struct Loaded {
    pub root: PathBuf,
    pub version: Version,
    pub layout: Layout,
    pub matcher: OperatorOwnedMatcher,
    pub items: Vec<ShipItem>,
    /// The home's `manifest.json` at snapshot time (`None`: the tree had none).
    pub manifest: Option<Vec<u8>>,
}

impl Loaded {
    pub fn rels(&self) -> Vec<&str> {
        self.items.iter().map(|i| i.rel.as_str()).collect()
    }
}

/// Read a snapshot. `layout.json` is validated like a manifest layout; every tree entry
/// must sit under one of its mirrored dirs and must not be operator-owned.
pub(crate) fn load(root: &Path) -> Result<Loaded, LayoutError> {
    let bad = |m: String| LayoutError::Snapshot(format!("{}: {m}", root.display()));
    // Never follow a link out of the snapshot (security review P2-1).
    for (name, want_dir) in [("tree", true), ("VERSION", false), ("layout.json", false)] {
        let ok = fs::symlink_metadata(root.join(name))
            .is_ok_and(|m| if want_dir { m.is_dir() } else { m.is_file() });
        if !ok {
            return Err(bad(format!(
                "{name} is missing or not a real {}",
                if want_dir { "directory" } else { "file" }
            )));
        }
    }
    let raw_version =
        fs::read_to_string(root.join("VERSION")).map_err(|e| bad(format!("VERSION: {e}")))?;
    let version = Version::parse(raw_version.trim()).map_err(|e| bad(format!("VERSION: {e}")))?;
    let snap_layout: SnapshotLayout = serde_json::from_slice(
        &fs::read(root.join("layout.json")).map_err(|e| bad(format!("layout.json: {e}")))?,
    )
    .map_err(|e| bad(format!("layout.json: {e}")))?;
    let shipped_owned = snap_layout.shipped_owned;
    let layout = validate_layout(RawLayout {
        mirrored: snap_layout.mirrored,
        operator_owned: snap_layout.operator_owned,
    })
    .map_err(|e| bad(format!("layout.json: {e}")))?;

    let matcher = OperatorOwnedMatcher::new(&layout.operator_owned)
        .map_err(|e| bad(format!("layout.json: {e}")))?;

    let tree = root.join("tree");
    let mut items = Vec::new();
    for entry in fs::read_dir(&tree).map_err(|e| bad(format!("tree: {e}")))? {
        let entry = entry.map_err(|e| bad(e.to_string()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| bad("non-UTF-8 name".into()))?;
        let is_dir = entry.file_type().map_err(|e| bad(e.to_string()))?.is_dir();
        if !is_dir || !layout.mirrored.contains(&name) {
            return Err(bad(format!("tree/{name} is not a mirrored directory")));
        }
        collect(&tree, &name, &matcher, &shipped_owned, &mut items).map_err(bad)?;
    }
    let manifest = match fs::symlink_metadata(root.join("manifest.json")) {
        Ok(m) if m.is_file() => {
            Some(fs::read(root.join("manifest.json")).map_err(|e| bad(e.to_string()))?)
        }
        Ok(_) => return Err(bad("manifest.json is not a real file".into())),
        Err(_) => None,
    };
    Ok(Loaded {
        root: root.to_path_buf(),
        version,
        layout,
        matcher,
        items,
        manifest,
    })
}

fn collect(
    tree: &Path,
    rel: &str,
    m: &OperatorOwnedMatcher,
    shipped_owned: &[String],
    out: &mut Vec<ShipItem>,
) -> Result<(), String> {
    for entry in fs::read_dir(tree.join(rel)).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF-8 name".to_string())?;
        let child = format!("{rel}/{name}");
        if child.ends_with(".rein-tmp") {
            continue; // never payload (see `take`)
        }
        check_rel_path(&child)?;
        let path = tree.join(&child);
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        // An owned path may be restored only if the snapshot says it held it because it was
        // shipped (rein D-155 r2); an owned directory only to reach such a path.
        if owned(m, &child) {
            let allowed = if meta.is_dir() {
                let prefix = format!("{child}/");
                shipped_owned.iter().any(|p| p.starts_with(&prefix))
            } else {
                shipped_owned.contains(&child)
            };
            if !allowed {
                return Err(format!("tree/{child} is operator-owned"));
            }
        }
        if meta.is_dir() {
            collect(tree, &child, m, shipped_owned, out)?;
        } else if meta.file_type().is_symlink() {
            let to = fs::read_link(&path).map_err(|e| e.to_string())?;
            out.push(ShipItem {
                rel: child,
                src: Source::Symlink(to),
            });
        } else if meta.is_file() {
            let mode = file_mode(&meta);
            out.push(ShipItem {
                rel: child,
                src: Source::File { from: path, mode },
            });
        } else {
            return Err(format!("tree/{child} is not a file, directory or symlink"));
        }
    }
    Ok(())
}

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
