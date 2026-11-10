//! Usage errors and `--help`/`--version` at the process boundary (§API "All commands", D-016).
//! Extends `main.rs` `parses_profiles_subcommands_and_global_home`, which covers parsing only:
//! the exit code is decided in `main` and is only observable by running the binary.

use std::process::{Command, Output};

fn rein_app(args: &[&str]) -> Output {
    let home = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_rein-app"))
        .args(args)
        .env("HOME", home.path())
        .output()
        .unwrap()
}

#[test]
fn usage_errors_exit_64_on_stderr_never_2() {
    for args in [
        &["frobnicate"][..],
        &["install", "--frobnicate"],
        &["install", "--version", "not-a-version"],
        &["profiles", "apply", "--nope"],
        &[],
    ] {
        let out = rein_app(args);
        assert_eq!(out.status.code(), Some(64), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}: usage goes to stderr");
        assert!(!out.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn help_and_version_exit_0_on_stdout() {
    for args in [
        &["--help"][..],
        &["--version"],
        &["install", "--help"],
        &["help"],
    ] {
        let out = rein_app(args);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert!(!out.stdout.is_empty(), "{args:?}");
    }
}
