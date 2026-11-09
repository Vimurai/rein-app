//! LayoutWriter: upgrade semantics, N=2 previous trees, lock, operator-owned carry-over
//! (blueprint §Components 3, §Test Plan "Upgrade semantics", §Rollback Plan).

mod support;

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use rein_release::{InstallPaths, LayoutError, VerifiedRelease, install_tree, verify_release};
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

/// A live 0.4.0 tree with operator data, rein data the schema-1 list does not name, and
/// stale release files.
fn seed_live(rein: &Path) {
    write(&rein.join("VERSION"), b"0.4.0\n");
    write(&rein.join("accounts.json"), b"{\"work\":{}}");
    write(&rein.join("secrets/token"), b"s3cret");
    write(&rein.join("run/pane.pid"), b"42");
    write(&rein.join("config/rein-workspace.txt"), b"/home/op/rein\n");
    write(&rein.join("config/stale.json"), b"{}");
    write(
        &rein.join("mcp/x/node_modules/dep/index.js"),
        b"module.exports=1",
    );
    write(&rein.join("mcp/x/old.mjs"), b"old");
    write(&rein.join("approvals.sqlite"), b"db");
    write(&rein.join("approvals.sqlite-wal"), b"wal");
    write(&rein.join("ci/logs/a.log"), b"log");
    write(&rein.join("bin/old-tool"), b"#!/bin/sh\n");
    write(&rein.join("bin/rein"), b"old rein");
}

#[test]
fn upgrade_replaces_mirrored_dirs_and_carries_operator_data() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    let rein = paths.rein();
    seed_live(&rein);

    let out = install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap();

    // Operator-owned and unlisted top-level data: byte-identical.
    for (p, bytes) in [
        ("accounts.json", &b"{\"work\":{}}"[..]),
        ("secrets/token", b"s3cret"),
        ("run/pane.pid", b"42"),
        ("config/rein-workspace.txt", b"/home/op/rein\n"),
        ("mcp/x/node_modules/dep/index.js", b"module.exports=1"),
        ("approvals.sqlite", b"db"),
        ("approvals.sqlite-wal", b"wal"),
        ("ci/logs/a.log", b"log"),
    ] {
        assert_eq!(fs::read(rein.join(p)).unwrap(), bytes, "{p}");
    }
    // Mirrored dirs hold exactly what the release ships.
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
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &str| fs::metadata(rein.join(p)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("bin/rein"), 0o755);
        assert_eq!(mode("shared/locate.sh"), 0o644);
    }

    // The old tree is kept whole for rollback.
    let prev = home.path().join(".rein.prev-0.4.0");
    assert_eq!(out.previous.as_deref(), Some(prev.as_path()));
    assert_eq!(fs::read(prev.join("bin/old-tool")).unwrap(), b"#!/bin/sh\n");
    assert_eq!(fs::read_to_string(prev.join("VERSION")).unwrap(), "0.4.0\n");
    assert!(!home.path().join(".rein.staging-0.5.0").exists());
    assert!(!paths.lock().exists(), "lock released");
}

#[test]
fn fresh_install_has_only_release_files_and_version() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    let out = install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap();
    assert_eq!(out.previous, None);
    let mut names: Vec<String> = fs::read_dir(paths.rein())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["VERSION", "bin", "config", "hooks", "shared"]);
}

#[test]
fn a_release_file_at_an_operator_owned_path_is_never_written() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    write(&paths.rein().join("config/rein-workspace.txt"), b"mine\n");
    let mut files = good_files();
    files.push(Listed {
        path: "config/rein-workspace.txt",
        bytes: b"from the tarball\n".to_vec(),
        mode: "0644",
    });
    let out = install_tree(&paths, &verified(&key, &files, scratch.path())).unwrap();
    assert_eq!(out.skipped_operator_owned, ["config/rein-workspace.txt"]);
    assert_eq!(
        fs::read(paths.rein().join("config/rein-workspace.txt")).unwrap(),
        b"mine\n"
    );
}

#[test]
fn keeps_two_previous_trees_newest_first() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    let now = SystemTime::now();
    for (label, age_days) in [("0.2.0", 30), ("0.3.0", 10)] {
        let v = paths.prev(label).join("VERSION");
        write(&v, format!("{label}\n").as_bytes());
        let f = fs::File::options().write(true).open(&v).unwrap();
        f.set_modified(now - Duration::from_secs(age_days * 86_400))
            .unwrap();
    }
    write(&paths.rein().join("VERSION"), b"0.4.0\n");

    let out = install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap();
    assert_eq!(out.pruned, [paths.prev("0.2.0")]);
    assert_eq!(
        paths.previous_trees().unwrap(),
        [paths.prev("0.4.0"), paths.prev("0.3.0")]
    );
}

#[test]
fn a_held_lock_refuses_and_leaves_the_live_tree_alone() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    seed_live(&paths.rein());
    fs::write(paths.lock(), b"999").unwrap();
    let err = install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap_err();
    assert!(matches!(err, LayoutError::Locked(_)), "{err}");
    assert!(paths.rein().join("bin/old-tool").exists());
    assert!(paths.lock().exists(), "someone else's lock is not removed");
    assert!(!home.path().join(".rein.staging-0.5.0").exists());
}

#[test]
fn a_leftover_staging_tree_is_rebuilt_not_merged() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    write(&home.path().join(".rein.staging-0.5.0/bin/leftover"), b"x");
    install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap();
    assert!(!paths.rein().join("bin/leftover").exists());
}

#[cfg(unix)]
#[test]
fn symlinks_are_carried_as_links_and_a_symlinked_home_is_refused() {
    use std::os::unix::fs::symlink;
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    let rein = paths.rein();
    write(&rein.join("VERSION"), b"0.4.0\n");
    fs::create_dir_all(rein.join("secrets")).unwrap();
    symlink("/etc/hostname", rein.join("secrets/link")).unwrap();
    install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap();
    let link = rein.join("secrets/link");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(&link).unwrap(), Path::new("/etc/hostname"));

    // A Rein home that is itself a symlink is refused before anything is staged.
    let other = tempfile::tempdir().unwrap();
    let paths2 = InstallPaths::new(other.path());
    symlink(&rein, paths2.rein()).unwrap();
    let scratch2 = tempfile::tempdir().unwrap();
    let err = install_tree(&paths2, &verified(&key, &good_files(), scratch2.path())).unwrap_err();
    assert!(matches!(err, LayoutError::NotADirectory(_)), "{err}");
}

#[cfg(unix)]
#[test]
fn a_private_home_and_its_directories_keep_their_modes() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    let rein = paths.rein();
    seed_live(&rein);
    let chmod = |p: &Path, m| fs::set_permissions(p, fs::Permissions::from_mode(m)).unwrap();
    chmod(&rein, 0o700);
    chmod(&rein.join("config"), 0o700);
    chmod(&rein.join("secrets"), 0o700);

    install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap();
    let mode = |p: &str| fs::metadata(rein.join(p)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(""), 0o700, "the Rein home");
    assert_eq!(
        mode("config"),
        0o700,
        "a mirrored dir takes the live dir's mode"
    );
    assert_eq!(mode("secrets"), 0o700, "a carried dir keeps its mode");
    assert_eq!(
        mode("hooks"),
        0o755,
        "a dir new in this release defaults to 0755"
    );
}

#[test]
fn an_existing_previous_tree_with_the_same_version_is_kept() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    write(&paths.prev("0.4.0").join("VERSION"), b"0.4.0\n");
    write(&paths.prev("0.4.0").join("marker"), b"older");
    write(&paths.rein().join("VERSION"), b"0.4.0\n");
    let out = install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap();
    assert_eq!(out.previous, Some(paths.prev("0.4.0~1")));
    assert_eq!(
        fs::read(paths.prev("0.4.0").join("marker")).unwrap(),
        b"older"
    );
}

#[test]
fn a_missing_home_with_a_previous_tree_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    write(&paths.prev("0.4.0").join("accounts.json"), b"{}");
    let err = install_tree(&paths, &verified(&key, &good_files(), scratch.path())).unwrap_err();
    assert!(matches!(err, LayoutError::OrphanedPrevious(_)), "{err}");
    assert!(!paths.rein().exists());
}

#[cfg(unix)]
#[test]
fn a_read_only_live_directory_does_not_block_the_upgrade() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let key = TestKey::generate();
    let paths = InstallPaths::new(home.path());
    let rein = paths.rein();
    seed_live(&rein);
    let ro = fs::Permissions::from_mode(0o500);
    fs::set_permissions(rein.join("config"), ro).unwrap();

    let result = install_tree(&paths, &verified(&key, &good_files(), scratch.path()));
    // Restore write access so the temp dirs can be removed whatever happened.
    for dir in [rein.join("config"), paths.prev("0.4.0").join("config")] {
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    }
    result.unwrap();
    assert_eq!(
        fs::read(rein.join("config/rein-workspace.txt")).unwrap(),
        b"/home/op/rein\n"
    );
}
