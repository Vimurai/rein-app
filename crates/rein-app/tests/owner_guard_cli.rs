//! Owner guard at the process boundary (D-013, blueprint §Test Plan "Owner guard"): the
//! real binary, a real uid mismatch, no root. On Linux `/tmp` is owned by root and
//! writable by everyone, so `--home /tmp` lets the run create its lock (owned by the test
//! user) while `$HOME` belongs to someone else — exactly the `sudo` shape inverted.
//! `install` and `rollback` refuse before any network or write; the uid-injected
//! in-process tests in install_cmd.rs / update_cmd.rs cover `update` and Windows/macOS.
#![cfg(target_os = "linux")]

use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

#[test]
fn install_and_rollback_as_a_non_owner_name_the_path_and_leave_nothing() {
    let home = Path::new("/tmp");
    let probe = tempfile::NamedTempFile::new().unwrap();
    let me = probe.as_file().metadata().unwrap().uid();
    let owner = home.metadata().unwrap().uid();
    if owner == me {
        eprintln!("SKIP: /tmp is owned by the test user (uid {me}); no mismatch to observe");
        return;
    }
    if home.join(".rein.lock").exists() {
        eprintln!("SKIP: /tmp/.rein.lock exists (another run in progress)");
        return;
    }
    // One process at a time: both runs use /tmp/.rein.lock.
    for args in [&["install", "--version", "0.0.1"][..], &["rollback"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_rein-app"))
            .arg("--home")
            .arg(home)
            .args(args)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(
            stderr.contains(&format!(
                "/tmp belongs to uid {owner} but rein-app runs as uid {me}"
            )),
            "{args:?}: the refused path is named: {stderr}"
        );
        assert!(!home.join(".rein.lock").exists(), "{args:?}: lock released");
        assert!(!home.join(".rein").exists(), "{args:?}: nothing written");
        assert!(
            !home.join(".rein.fetch-0.0.1").exists(),
            "{args:?}: nothing fetched"
        );
    }
}
