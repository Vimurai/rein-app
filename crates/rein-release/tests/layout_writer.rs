//! LayoutWriter: in-place per-file sync (D-011) — upgrade semantics, untouched outside the
//! mirror, sync failure + restore, snapshots, rollback, refusals (blueprint §Test Plan).

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rein_release::{
    Faults, InstallLock, InstallPaths, LayoutError, RollbackError, VerifiedRelease, install_tree,
    install_tree_with_faults, owner_guard, rollback_tree, verify_release,
};
use semver::Version;
use support::{Listed, Release, TestKey, VERSION, entries_for, good_files, manifest_json};

fn verified(key: &TestKey, files: &[Listed], scratch: &Path) -> VerifiedRelease {
    let manifest = manifest_json(&key.key_id(), files);
    let release = Release::signed(key, &entries_for(files, &manifest), manifest);
    let assets = release.write_to(scratch);
    verify_release(
        &assets,
        &key.verifier(),
        &Version::parse(VERSION).unwrap(),
        &scratch.join("tree"),
    )
    .unwrap()
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// A live 0.4.0 home: operator data inside and outside the mirror, stale release files.
fn seed_live(rein: &Path) {
    write(&rein.join("VERSION"), b"0.4.0\n");
    write(&rein.join("accounts.json"), b"{\"work\":{}}");
    write(&rein.join("secrets/token"), b"s3cret");
    write(&rein.join("config/rein-workspace.txt"), b"/home/op/rein\n");
    write(&rein.join("config/stale.json"), b"{}");
    write(
        &rein.join("mcp/x/node_modules/dep/index.js"),
        b"module.exports=1",
    );
    write(&rein.join("mcp/x/old.mjs"), b"old");
    write(&rein.join("approvals.sqlite"), b"db");
    write(&rein.join("approvals.sqlite-wal"), b"wal");
    write(&rein.join("approvals.sqlite-shm"), b"shm");
    write(&rein.join("ci/x"), b"ci");
    write(&rein.join("trash/y"), b"trash");
    write(&rein.join("bin/old-tool"), b"#!/bin/sh\n");
    write(&rein.join("bin/rein"), b"old rein");
    write(&rein.join("hooks/pre-tool-use.sh"), b"old hook");
}

struct Home {
    _dir: tempfile::TempDir,
    paths: InstallPaths,
    scratch: tempfile::TempDir,
    key: TestKey,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = InstallPaths::new(dir.path());
        Self {
            _dir: dir,
            paths,
            scratch: tempfile::tempdir().unwrap(),
            key: TestKey::generate(),
        }
    }
    fn seeded() -> Self {
        let h = Self::new();
        seed_live(&h.paths.rein());
        h
    }
    /// A freshly verified release (each call verifies into its own directory).
    fn release(&self, files: &[Listed]) -> VerifiedRelease {
        let sub = tempfile::tempdir_in(self.scratch.path()).unwrap().keep();
        verified(&self.key, files, &sub)
    }
    fn rein(&self) -> std::path::PathBuf {
        self.paths.rein()
    }
}

/// A faulted install, through the owner guard like every production caller.
fn faulty(h: &Home, faults: &Faults) -> Result<rein_release::InstallOutcome, LayoutError> {
    let lock = InstallLock::acquire(&h.paths)?;
    let checked = owner_guard(&lock, &h.paths, &[])?;
    install_tree_with_faults(&h.paths, &h.release(&good_files()), &checked, faults)
}

/// Every entry under `root`: path → (symlink target or bytes, mode).
fn digest(root: &Path) -> BTreeMap<String, (Vec<u8>, u32)> {
    fn go(root: &Path, dir: &Path, out: &mut BTreeMap<String, (Vec<u8>, u32)>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            // Manifest-style keys on every OS (Windows joins with '\').
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let meta = fs::symlink_metadata(&p).unwrap();
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&p).unwrap().to_string_lossy().into_owned();
                out.insert(rel, (target.into_bytes(), 0));
            } else if meta.is_dir() {
                out.insert(format!("{rel}/"), (Vec::new(), mode_of(&meta)));
                go(root, &p, out);
            } else {
                let bytes = fs::read(&p).unwrap_or_else(|_| b"<unreadable>".to_vec());
                out.insert(rel, (bytes, mode_of(&meta)));
            }
        }
    }
    let mut out = BTreeMap::new();
    go(root, root, &mut out);
    out
}

#[cfg(unix)]
fn mode_of(m: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    m.permissions().mode() & 0o7777
}
#[cfg(not(unix))]
fn mode_of(_m: &fs::Metadata) -> u32 {
    0
}

#[test]
fn upgrade_syncs_mirrored_dirs_and_keeps_operator_files() {
    let h = Home::seeded();
    let rein = h.rein();
    let out = install_tree(&h.paths, &h.release(&good_files())).unwrap();

    for (p, bytes) in [
        ("accounts.json", &b"{\"work\":{}}"[..]),
        ("secrets/token", b"s3cret"),
        ("config/rein-workspace.txt", b"/home/op/rein\n"),
        ("mcp/x/node_modules/dep/index.js", b"module.exports=1"),
        ("approvals.sqlite-wal", b"wal"),
    ] {
        assert_eq!(fs::read(rein.join(p)).unwrap(), bytes, "{p}");
    }
    assert!(
        !rein.join("bin/old-tool").exists(),
        "stale bin/ file pruned"
    );
    assert!(
        !rein.join("config/stale.json").exists(),
        "config/ is mirrored"
    );
    assert!(!rein.join("mcp/x/old.mjs").exists(), "mcp/ is mirrored");
    assert_eq!(
        fs::read(rein.join("bin/rein")).unwrap(),
        b"#!/bin/sh\necho rein\n"
    );
    assert_eq!(
        fs::read(rein.join("config/defaults.json")).unwrap(),
        b"{\"a\":1}\n"
    );
    assert_eq!(fs::read_to_string(rein.join("VERSION")).unwrap(), "0.5.0\n");
    assert!(
        !rein.join("manifest.json").exists(),
        "install-rein.sh writes no manifest"
    );
    #[cfg(unix)]
    {
        let mode = |p: &str| mode_of(&fs::metadata(rein.join(p)).unwrap());
        assert_eq!(mode("bin/rein"), 0o755);
        assert_eq!(mode("shared/locate.sh"), 0o644);
    }
    assert_eq!(out.previous, Some(h.paths.prev("0.4.0")));
    let temps: Vec<String> = digest(&rein)
        .into_keys()
        .filter(|k| k.contains("rein-tmp"))
        .collect();
    assert!(temps.is_empty(), "{temps:?}");
}

#[test]
fn snapshot_holds_only_the_old_payload() {
    let h = Home::seeded();
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    let snap = h.paths.prev("0.4.0");
    let keys: Vec<String> = digest(&snap.join("tree")).into_keys().collect();
    assert_eq!(
        keys,
        [
            "bin/",
            "bin/old-tool",
            "bin/rein",
            "config/",
            "config/stale.json",
            "hooks/",
            "hooks/pre-tool-use.sh",
            "mcp/",
            "mcp/x/",
            "mcp/x/old.mjs"
        ],
        "no operator-owned path, nothing from outside the mirror"
    );
    assert_eq!(fs::read_to_string(snap.join("VERSION")).unwrap(), "0.4.0\n");
    assert!(snap.join("layout.json").is_file());
    #[cfg(unix)]
    assert_eq!(mode_of(&fs::metadata(&snap).unwrap()), 0o700);
}

#[cfg(unix)]
#[test]
fn nothing_outside_the_mirror_is_opened_by_install_or_rollback() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let inode = |p: &Path| fs::symlink_metadata(p).unwrap().ino();
    let h = Home::seeded();
    let rein = h.rein();
    write(&rein.join("secrets/locked"), b"x");
    fs::set_permissions(
        rein.join("secrets/locked"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let watched = [
        "approvals.sqlite",
        "approvals.sqlite-wal",
        "approvals.sqlite-shm",
        "ci/x",
        "trash/y",
        "accounts.json",
        "secrets/locked",
        "config/rein-workspace.txt",
        "mcp/x/node_modules/dep/index.js",
    ];
    let before: Vec<u64> = watched.iter().map(|p| inode(&rein.join(p))).collect();
    // Bytes of every watched file except the unreadable one (read before and after).
    let bytes = || -> Vec<Vec<u8>> {
        watched
            .iter()
            .filter(|p| **p != "secrets/locked")
            .map(|p| fs::read(rein.join(p)).unwrap())
            .collect()
    };
    let bytes_before = bytes();

    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    let after_install: Vec<u64> = watched.iter().map(|p| inode(&rein.join(p))).collect();
    let (_, lock) = rollback_tree(&h.paths).unwrap();
    drop(lock);
    let after_rollback: Vec<u64> = watched.iter().map(|p| inode(&rein.join(p))).collect();
    assert_eq!(before, after_install, "same files after install");
    assert_eq!(before, after_rollback, "same files after rollback");
    fs::set_permissions(
        rein.join("secrets/locked"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(
        bytes(),
        bytes_before,
        "same bytes after install and rollback"
    );
    assert_eq!(fs::read(rein.join("secrets/locked")).unwrap(), b"x");
}

#[test]
fn a_failure_at_every_step_restores_the_old_payload() {
    let before = digest(&Home::seeded().rein());
    let mut k = 1;
    loop {
        let h = Home::seeded();
        let faults = Faults::fail_at(k);
        match faulty(&h, &faults) {
            Ok(_) => {
                assert!(k > faults.steps(), "the fault at step {k} never fired");
                break;
            }
            Err(LayoutError::Sync { restored, .. }) => {
                assert!(restored, "step {k}: restore failed");
                assert_eq!(
                    digest(&h.rein()),
                    before,
                    "step {k}: tree differs after restore"
                );
            }
            Err(e) => panic!("step {k}: {e}"),
        }
        k += 1;
        assert!(k < 100, "runaway");
    }
    assert!(k > 5, "the sync has more than a handful of steps ({k})");
}

#[test]
fn a_failed_fresh_install_has_no_version_and_a_rerun_completes_it() {
    let h = Home::new();
    let err = faulty(&h, &Faults::fail_at(2)).unwrap_err();
    assert!(
        matches!(err, LayoutError::Sync { snapshot: None, .. }),
        "{err}"
    );
    assert!(!h.rein().join("VERSION").exists());
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    assert_eq!(
        fs::read_to_string(h.rein().join("VERSION")).unwrap(),
        "0.5.0\n"
    );
}

#[test]
fn fresh_install_has_only_release_files_and_version() {
    let h = Home::new();
    let out = install_tree(&h.paths, &h.release(&good_files())).unwrap();
    assert_eq!(out.previous, None);
    let mut names: Vec<String> = fs::read_dir(h.rein())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["VERSION", "bin", "config", "hooks", "shared"]);
}

#[test]
fn a_release_file_at_an_operator_owned_path_is_never_written() {
    let h = Home::new();
    write(&h.rein().join("config/rein-workspace.txt"), b"mine\n");
    let mut files = good_files();
    files.push(Listed {
        path: "config/rein-workspace.txt",
        bytes: b"from the tarball\n".to_vec(),
        mode: "0644",
    });
    let out = install_tree(&h.paths, &h.release(&files)).unwrap();
    assert_eq!(out.skipped_operator_owned, ["config/rein-workspace.txt"]);
    assert_eq!(
        fs::read(h.rein().join("config/rein-workspace.txt")).unwrap(),
        b"mine\n"
    );
}

#[test]
fn keeps_two_snapshots_and_never_overwrites_one() {
    let h = Home::seeded();
    for old in ["0.2.0", "0.3.0", "0.4.0"] {
        fs::write(h.rein().join("VERSION"), format!("{old}\n")).unwrap();
        install_tree(&h.paths, &h.release(&good_files())).unwrap();
    }
    assert_eq!(
        h.paths.previous_trees().unwrap(),
        [h.paths.prev("0.4.0"), h.paths.prev("0.3.0")]
    );
    fs::write(h.rein().join("VERSION"), "0.4.0\n").unwrap();
    let out = install_tree(&h.paths, &h.release(&good_files())).unwrap();
    assert_eq!(out.previous, Some(h.paths.prev("0.4.0~1")));
}

#[test]
fn rollback_restores_code_only_and_a_second_rollback_returns() {
    let h = Home::seeded();
    let rein = h.rein();
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    write(&rein.join("accounts.json"), b"{\"new\":{}}");
    write(&rein.join("approvals.sqlite"), b"db2");

    let (out, lock) = rollback_tree(&h.paths).unwrap();
    drop(lock);
    assert_eq!(out.restored, Version::new(0, 4, 0));
    assert_eq!(fs::read(rein.join("bin/rein")).unwrap(), b"old rein");
    assert!(rein.join("bin/old-tool").exists());
    assert!(
        !rein.join("shared/locate.sh").exists(),
        "0.5.0-only file removed"
    );
    assert_eq!(
        fs::read(rein.join("hooks/pre-tool-use.sh")).unwrap(),
        b"old hook"
    );
    assert_eq!(fs::read_to_string(rein.join("VERSION")).unwrap(), "0.4.0\n");
    assert_eq!(
        fs::read(rein.join("accounts.json")).unwrap(),
        b"{\"new\":{}}",
        "data kept"
    );
    assert_eq!(fs::read(rein.join("approvals.sqlite")).unwrap(), b"db2");
    assert!(
        !h.paths.prev("0.4.0").exists(),
        "the restored snapshot is removed"
    );

    let (again, _lock) = rollback_tree(&h.paths).unwrap();
    assert_eq!(again.restored, Version::new(0, 5, 0));
    assert_eq!(
        fs::read(rein.join("bin/rein")).unwrap(),
        b"#!/bin/sh\necho rein\n"
    );
    assert!(!rein.join("bin/old-tool").exists());
}

#[test]
fn rollback_without_a_snapshot_and_with_a_tampered_layout_is_refused() {
    let h = Home::seeded();
    assert!(matches!(
        rollback_tree(&h.paths),
        Err(RollbackError::NoPrevious)
    ));
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    let before = digest(&h.rein());
    let layout = h.paths.prev("0.4.0").join("layout.json");
    for bad in [
        r#"{"mirrored":["../x"],"operator_owned":[]}"#,
        r#"{"mirrored":["bin/sub"],"operator_owned":[]}"#,
        r#"{"mirrored":["bin"],"operator_owned":[],"extra":1}"#,
    ] {
        fs::write(&layout, bad).unwrap();
        let err = rollback_tree(&h.paths).unwrap_err();
        assert!(
            matches!(err, RollbackError::Layout(LayoutError::Snapshot(_))),
            "{bad}: {err}"
        );
        assert_eq!(digest(&h.rein()), before, "{bad}: nothing changed");
    }
}

#[test]
fn a_held_lock_refuses_and_leaves_everything_alone() {
    let h = Home::seeded();
    fs::write(h.paths.lock(), b"999").unwrap();
    let before = digest(&h.rein());
    let err = install_tree(&h.paths, &h.release(&good_files())).unwrap_err();
    assert!(matches!(err, LayoutError::Locked(_)), "{err}");
    assert!(
        h.paths.lock().exists(),
        "someone else's lock is not removed"
    );
    assert_eq!(digest(&h.rein()), before);
}

#[cfg(unix)]
#[test]
fn symlinks_and_conflicts_are_refused_before_any_write() {
    use std::os::unix::fs::symlink;
    let elsewhere = tempfile::tempdir().unwrap();
    type Setup = fn(&Path, &Path);
    let setups: [(&str, Setup); 3] = [
        ("symlinked mirrored dir", |rein, other| {
            symlink(other, rein.join("shared")).unwrap();
        }),
        ("symlink above a target", |rein, other| {
            fs::remove_dir_all(rein.join("hooks")).unwrap();
            fs::create_dir_all(rein.join("hooks")).unwrap();
            fs::remove_dir(rein.join("hooks")).unwrap();
            symlink(other, rein.join("hooks")).unwrap();
        }),
        ("directory where a file goes", |rein, _| {
            fs::remove_file(rein.join("bin/rein")).unwrap();
            fs::create_dir(rein.join("bin/rein")).unwrap();
        }),
    ];
    for (name, setup) in setups {
        let h = Home::seeded();
        setup(&h.rein(), elsewhere.path());
        let before = digest(&h.rein());
        let err = install_tree(&h.paths, &h.release(&good_files())).unwrap_err();
        assert!(
            matches!(err, LayoutError::Symlink(_) | LayoutError::Conflict(_)),
            "{name}: {err}"
        );
        assert_eq!(digest(&h.rein()), before, "{name}: nothing written");
        assert!(
            h.paths.previous_trees().unwrap().is_empty(),
            "{name}: no snapshot"
        );
        assert!(
            fs::read_dir(elsewhere.path()).unwrap().next().is_none(),
            "{name}: nothing written through the link"
        );
    }

    let h = Home::new();
    let real = tempfile::tempdir().unwrap();
    symlink(real.path(), h.rein()).unwrap();
    let err = install_tree(&h.paths, &h.release(&good_files())).unwrap_err();
    assert!(
        matches!(err, LayoutError::Symlink(_)),
        "symlinked home: {err}"
    );
}

#[test]
fn at_every_stop_point_hooks_are_old_or_new_and_land_last() {
    let mut k = 1;
    loop {
        let h = Home::seeded();
        let faults = Faults::stop_at(k);
        if faulty(&h, &faults).is_ok() {
            break;
        }
        let rein = h.rein();
        let hook = fs::read(rein.join("hooks/pre-tool-use.sh")).unwrap();
        assert!(
            hook == b"old hook" || hook == b"#!/bin/sh\nexit 0\n",
            "step {k}: hook is neither old nor new"
        );
        if hook != b"old hook" {
            for (p, new) in [
                ("bin/rein", &b"#!/bin/sh\necho rein\n"[..]),
                ("shared/locate.sh", b"rein_locate() { :; }\n"),
                ("config/defaults.json", b"{\"a\":1}\n"),
            ] {
                assert_eq!(
                    fs::read(rein.join(p)).unwrap(),
                    new,
                    "step {k}: {p} before hooks"
                );
            }
        }
        assert_eq!(
            fs::read_to_string(rein.join("VERSION")).unwrap(),
            "0.4.0\n",
            "step {k}: VERSION is written last"
        );
        k += 1;
        assert!(k < 100, "runaway");
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_at_a_target_is_replaced_not_written_through() {
    use std::os::unix::fs::symlink;
    let h = Home::seeded();
    let victim = tempfile::tempdir().unwrap();
    write(&victim.path().join("x"), b"victim");
    symlink(
        victim.path().join("x"),
        h.rein().join("config/defaults.json"),
    )
    .unwrap();
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    assert_eq!(fs::read(victim.path().join("x")).unwrap(), b"victim");
    let target = h.rein().join("config/defaults.json");
    assert!(fs::symlink_metadata(&target).unwrap().is_file());
}

#[cfg(unix)]
#[test]
fn a_symlink_at_an_owned_directory_path_is_never_pruned() {
    use std::os::unix::fs::symlink;
    let h = Home::seeded();
    let store = tempfile::tempdir().unwrap();
    fs::create_dir_all(h.rein().join("mcp/y")).unwrap();
    symlink(store.path(), h.rein().join("mcp/y/node_modules")).unwrap();
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    let link = h.rein().join("mcp/y/node_modules");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let snap = h.paths.prev("0.4.0").join("tree/mcp/y/node_modules");
    assert!(
        fs::symlink_metadata(snap).is_err(),
        "not snapshotted either"
    );
    let (_, _lock) = rollback_tree(&h.paths).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn a_live_symlink_does_not_block_restore_or_rollback() {
    use std::os::unix::fs::symlink;
    let seed = || {
        let h = Home::seeded();
        symlink("/usr/bin/env", h.rein().join("bin/env-link")).unwrap();
        h
    };
    let before = digest(&seed().rein());
    for k in 1..=3 {
        let h = seed();
        let err = faulty(&h, &Faults::fail_at(k)).unwrap_err();
        assert!(
            matches!(err, LayoutError::Sync { restored: true, .. }),
            "k={k}: {err}"
        );
        assert_eq!(digest(&h.rein()), before, "k={k}");
    }
    let h = seed();
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    symlink("/usr/bin/env", h.rein().join("bin/env-link")).unwrap(); // re-created by the operator
    let (out, _lock) = rollback_tree(&h.paths).unwrap();
    assert_eq!(out.restored, Version::new(0, 4, 0));
    assert_eq!(
        fs::read_link(h.rein().join("bin/env-link")).unwrap(),
        Path::new("/usr/bin/env")
    );
}

#[test]
fn only_real_snapshots_are_pruned_or_restored() {
    let h = Home::seeded();
    write(&h.paths.prev("manual").join("keep.txt"), b"mine");
    for old in ["0.2.0", "0.3.0", "0.4.0"] {
        fs::write(h.rein().join("VERSION"), format!("{old}\n")).unwrap();
        install_tree(&h.paths, &h.release(&good_files())).unwrap();
    }
    assert_eq!(
        fs::read(h.paths.prev("manual").join("keep.txt")).unwrap(),
        b"mine"
    );
    assert_eq!(h.paths.previous_trees().unwrap().len(), 2);
}

#[cfg(unix)]
#[test]
fn a_snapshot_whose_tree_is_a_symlink_is_refused() {
    use std::os::unix::fs::symlink;
    let h = Home::seeded();
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(&outside.path().join("bin/stolen"), b"OUTSIDE");
    let tree = h.paths.prev("0.4.0").join("tree");
    fs::remove_dir_all(&tree).unwrap();
    symlink(outside.path(), &tree).unwrap();
    let err = rollback_tree(&h.paths).unwrap_err();
    assert!(
        matches!(err, RollbackError::Layout(LayoutError::Snapshot(_))),
        "{err}"
    );
    assert!(!h.rein().join("bin/stolen").exists());
}

#[cfg(unix)]
#[test]
fn a_symlink_at_an_intermediate_component_is_refused() {
    use std::os::unix::fs::symlink;
    let h = Home::seeded();
    let elsewhere = tempfile::tempdir().unwrap();
    symlink(elsewhere.path(), h.rein().join("mcp/srv")).unwrap();
    let mut files = good_files();
    files.push(Listed {
        path: "mcp/srv/index.mjs",
        bytes: b"export {}\n".to_vec(),
        mode: "0644",
    });
    let before = digest(&h.rein());
    let err = install_tree(&h.paths, &h.release(&files)).unwrap_err();
    assert!(matches!(err, LayoutError::Symlink(_)), "{err}");
    assert_eq!(digest(&h.rein()), before);
    assert!(fs::read_dir(elsewhere.path()).unwrap().next().is_none());
}

#[test]
fn a_file_at_an_owned_directory_path_is_left_alone_and_restores_still_work() {
    let h = Home::seeded();
    write(&h.rein().join("mcp/z/node_modules"), b"stray");
    let before = digest(&h.rein());
    let err = faulty(&h, &Faults::fail_at(1)).unwrap_err();
    assert!(
        matches!(err, LayoutError::Sync { restored: true, .. }),
        "{err}"
    );
    assert_eq!(digest(&h.rein()), before);
    install_tree(&h.paths, &h.release(&good_files())).unwrap();
    assert_eq!(
        fs::read(h.rein().join("mcp/z/node_modules")).unwrap(),
        b"stray"
    );
    let (_, _lock) = rollback_tree(&h.paths).unwrap();
    assert_eq!(
        fs::read(h.rein().join("mcp/z/node_modules")).unwrap(),
        b"stray"
    );
}

#[test]
fn a_populated_home_without_a_version_updates_with_the_no_snapshot_notice() {
    let h = Home::seeded();
    fs::remove_file(h.rein().join("VERSION")).unwrap();
    let out = install_tree(&h.paths, &h.release(&good_files())).unwrap();
    assert!(out.unsnapshotted);
    assert_eq!(out.previous, None);
    assert_eq!(
        fs::read_to_string(h.rein().join("VERSION")).unwrap(),
        "0.5.0\n"
    );
    let fresh = Home::new();
    assert!(
        !install_tree(&fresh.paths, &fresh.release(&good_files()))
            .unwrap()
            .unsnapshotted
    );
}
