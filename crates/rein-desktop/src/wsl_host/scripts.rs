//! The constant scripts and limits `WslHost` hands to `wsl.exe` (W2, W4): no interpolation;
//! the only variable argv words are validated outside them.

use super::*;

/// Learns the distro's name, the Linux home and the login shell (W2). `%s\\n` is `%s\n` once
/// sh has removed one backslash: the script holds no quote of any kind.
pub const PROBE_SCRIPT: &str = "printf %s\\\\n $WSL_DISTRO_NAME $HOME $SHELL";
/// Run with `--cd <project>`: exit 5 still in `/` (a failed `--cd` does not fail: `wsl.exe`
/// runs the command in `/` instead — D-038 addendum 1), 3 no `.ai/`, 4 rein not installed,
/// else `pwd -P`.
/// Exit 0 a project, 3 no `.ai/` here or above, 6 inside a project (the parent is the second
/// line); the canonical path is always the first line (V9 component 1; E-63 rework 3). The
/// parent walk runs with `IFS=` and `set -f`, so an unquoted path is never split or globbed.
pub const VALIDATE_SCRIPT: &str = "case $PWD in /) exit 5;; esac; [ -x ~/.rein/bin/rein ] || exit 4; pwd -P; [ -d .ai ] && exit 0; IFS=; set -f; d=$(pwd -P); while [ $d != / ]; do d=${d%/*}; [ -z $d ] && d=/; [ -d $d/.ai ] && cd $d && pwd -P && exit 6; done; exit 3";
/// The pty child's script, run by the user's login + interactive shell (W2). `tty` names the
/// pty first: the tmux overlay finds the app's client by it (V6, D-050), and the host takes
/// that line out of what the terminal shows (`tmux_overlay::take_tty_line`).
pub const START_SCRIPT: &str = "tty; exec ~/.rein/bin/rein start";
/// Close a project on the user's own server (V9 component 4, E-62).
pub const KILL_SCRIPT: &str = "exec ~/.rein/bin/rein start --kill --yes";
/// Health inside WSL, from the Linux home (W4, D-045 addendum 1).
pub const DOCTOR_SCRIPT: &str = "cd; exec ~/.rein/bin/rein doctor --scope install --json";
/// The installed Rein's version (updates.md component 1): read-only, bounded, no login shell.
pub const VERSION_SCRIPT: &str = "exec head -c 65 ~/.rein/VERSION";
/// `:open` completion under a parent other than `/` (W4): the `/` guard, then the listing.
pub const LIST_SCRIPT: &str = "case $PWD in /) exit 5;; esac; ls -1Ap";
/// The same for the parent `/` itself (the `--cd` fallback cannot mislead there).
pub const LIST_ROOT_SCRIPT: &str = "ls -1Ap";
pub const LIST_LIMIT: Duration = Duration::from_secs(5);

/// `exec rein-app <argv>` for the login shell (W4). The argv is the action's constant words
/// and profile names that passed `profile_name_ok`; every word is checked again here to hold
/// only `[A-Za-z0-9._-]`, so nothing in the script can be read by `wsl.exe` or a shell as
/// anything but a word.
pub fn cli_script(action: &crate::cli::CliAction) -> Result<String, IpcError> {
    let argv = action.argv();
    let plain = |w: &String| {
        !w.is_empty()
            && w.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    // A name after `--profile` must also pass the full profile-name rule (never a flag).
    let names_ok = argv
        .windows(2)
        .filter(|w| w[0] == "--profile")
        .all(|w| crate::cli::profile_name_ok(&w[1]));
    if !argv.iter().all(plain) || !names_ok {
        return Err(err(
            "bad_profile",
            "a profile name may hold letters, digits and . _ - only",
        ));
    }
    Ok(format!("exec rein-app {}", argv.join(" ")))
}

/// Nothing from Windows crosses into WSL (W2 "Environment").
pub const ENV_REMOVED: &[&str] = &["WSLENV"];

/// Every non-pty `wsl.exe` call (§Windows window "Limits"): a cold WSL VM takes seconds.
pub const CALL_LIMIT: Duration = Duration::from_secs(30);
pub const CALL_CAP: u64 = 64 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    /// The validate script itself, at its process boundary (`sh -c`, as `wsl.exe -e sh -c`
    /// runs it): exit 0 / 3 / 6 with the canonical path first; the parent walk never splits
    /// or globs a path (a space, `*`, `?` in a name).
    #[cfg(unix)]
    #[test]
    fn the_validate_script_walks_up_to_the_nearest_project() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".rein/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("rein"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(bin.join("rein"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let proj = root.join("my proj*");
        let deep = proj.join("a b/c?d");
        std::fs::create_dir_all(proj.join(".ai")).unwrap();
        std::fs::create_dir_all(&deep).unwrap();
        // A decoy a glob would match: `my proj*` must not expand to it.
        std::fs::create_dir_all(root.join("my projX/.ai")).unwrap();
        let plain = root.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let run = |dir: &std::path::Path| {
            let o = std::process::Command::new("/bin/sh")
                .args(["-c", VALIDATE_SCRIPT])
                .current_dir(dir)
                .env("HOME", home.path())
                .output()
                .unwrap();
            (o.status.code(), String::from_utf8(o.stdout).unwrap())
        };
        let p = |x: &std::path::Path| x.to_string_lossy().into_owned();
        assert_eq!(run(&proj), (Some(0), format!("{}\n", p(&proj))));
        assert_eq!(
            run(&deep),
            (Some(6), format!("{}\n{}\n", p(&deep), p(&proj)))
        );
        // `plain` sits beside the projects, not under one: no parent is a project, unless the
        // temp root itself is inside one (then that is the answer, and still exit 6).
        let (code, out) = run(&plain);
        assert!(matches!(code, Some(3 | 6)), "{code:?}");
        assert!(out.starts_with(&format!("{}\n", p(&plain))));
        assert!(!out.contains("my proj"), "{out}");
    }

    #[test]
    fn scripts_are_constants_without_quotes() {
        for s in [
            PROBE_SCRIPT,
            VALIDATE_SCRIPT,
            START_SCRIPT,
            KILL_SCRIPT,
            DOCTOR_SCRIPT,
            VERSION_SCRIPT,
            LIST_SCRIPT,
            LIST_ROOT_SCRIPT,
        ] {
            assert!(!s.contains(['"', '\'']), "{s}");
        }
    }

    /// E-68 (D-060 ruling 7): the move changed no script. Each constant against its text as it
    /// stood in `wsl_host.rs` before the split (copied verbatim from that file).
    #[test]
    fn every_script_is_byte_identical_to_before_the_split() {
        for (now, before) in [
            (PROBE_SCRIPT, "printf %s\\\\n $WSL_DISTRO_NAME $HOME $SHELL"),
            (
                VALIDATE_SCRIPT,
                "case $PWD in /) exit 5;; esac; [ -x ~/.rein/bin/rein ] || exit 4; pwd -P; [ -d .ai ] && exit 0; IFS=; set -f; d=$(pwd -P); while [ $d != / ]; do d=${d%/*}; [ -z $d ] && d=/; [ -d $d/.ai ] && cd $d && pwd -P && exit 6; done; exit 3",
            ),
            (START_SCRIPT, "tty; exec ~/.rein/bin/rein start"),
            (KILL_SCRIPT, "exec ~/.rein/bin/rein start --kill --yes"),
            (
                DOCTOR_SCRIPT,
                "cd; exec ~/.rein/bin/rein doctor --scope install --json",
            ),
            (LIST_SCRIPT, "case $PWD in /) exit 5;; esac; ls -1Ap"),
            (LIST_ROOT_SCRIPT, "ls -1Ap"),
        ] {
            assert_eq!(now, before);
        }
    }
}
