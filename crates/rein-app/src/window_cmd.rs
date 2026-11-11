//! `rein-app window [DIR]` — start the desktop window (desktop.md §API "CLI", D-033).
//!
//! `rein-desktop` is looked up beside this binary, then on `PATH`. The CLI links no GUI code:
//! it only starts the other binary and returns. A second window start is handled by
//! `rein-desktop` itself (single instance).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const DESKTOP_BIN: &str = if cfg!(windows) {
    "rein-desktop.exe"
} else {
    "rein-desktop"
};

/// The first executable `rein-desktop` beside `exe`, then in `path` (`PATH`'s value).
pub fn find_desktop(exe: Option<&Path>, path: Option<&OsString>) -> Option<PathBuf> {
    let beside = exe.and_then(Path::parent).map(|d| d.join(DESKTOP_BIN));
    let on_path = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|d| d.is_absolute()) // an empty or relative PATH entry means "here": skip it
        .map(|d| d.join(DESKTOP_BIN));
    beside.into_iter().chain(on_path).find(|p| is_executable(p))
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Start the window with `dir` (made absolute) as the first tab. 0 when started, 1 otherwise.
pub fn run(dir: Option<&Path>) -> u8 {
    let exe = std::env::current_exe().ok();
    let Some(bin) = find_desktop(exe.as_deref(), std::env::var_os("PATH").as_ref()) else {
        eprintln!(
            "rein-app: the window is not installed ({DESKTOP_BIN} was not found beside rein-app or on PATH)"
        );
        return 1;
    };
    let mut cmd = Command::new(&bin);
    if let Some(d) = dir {
        match std::path::absolute(d) {
            Ok(abs) => {
                cmd.arg(abs);
            }
            Err(e) => {
                eprintln!("rein-app: {}: {e}", d.display());
                return 1;
            }
        }
    }
    // The window outlives this command; it does not share the terminal.
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match cmd.spawn() {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("rein-app: cannot start {}: {e}", bin.display());
            1
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn exe_file(p: &Path, mode: u32) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn beside_the_binary_wins_over_path() {
        let t = tempfile::tempdir().unwrap();
        let beside = t.path().join("bin/rein-desktop");
        let other = t.path().join("other/rein-desktop");
        exe_file(&beside, 0o755);
        exe_file(&other, 0o755);
        let path = std::env::join_paths([t.path().join("other")]).unwrap();
        let exe = t.path().join("bin/rein-app");
        assert_eq!(find_desktop(Some(&exe), Some(&path)), Some(beside));
    }

    #[test]
    fn path_is_searched_in_order_skipping_relative_and_non_executable_entries() {
        let t = tempfile::tempdir().unwrap();
        exe_file(&t.path().join("a/rein-desktop"), 0o644);
        exe_file(&t.path().join("b/rein-desktop"), 0o755);
        let path =
            std::env::join_paths([PathBuf::from("rel"), t.path().join("a"), t.path().join("b")])
                .unwrap();
        let exe = t.path().join("nowhere/rein-app");
        assert_eq!(
            find_desktop(Some(&exe), Some(&path)),
            Some(t.path().join("b/rein-desktop"))
        );
    }

    #[test]
    fn absent_everywhere_is_none() {
        let t = tempfile::tempdir().unwrap();
        let path = std::env::join_paths([t.path()]).unwrap();
        assert_eq!(
            find_desktop(Some(&t.path().join("rein-app")), Some(&path)),
            None
        );
        assert_eq!(find_desktop(None, None), None);
    }
}
