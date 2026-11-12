//! The app's own directories (desktop.md §Data Model "Files"): XDG base directories; on macOS
//! `~/Library/Application Support/rein-app` only when the XDG variable is unset; on Windows
//! `%LOCALAPPDATA%\rein-app` (§Windows window W6).

use std::path::PathBuf;

fn xdg(env: &dyn Fn(&str) -> Option<String>, var: &str, fallback: &str) -> Option<PathBuf> {
    if let Some(d) = env(var).map(PathBuf::from).filter(|p| p.is_absolute()) {
        return Some(d.join("rein-app"));
    }
    let home = env("HOME").map(PathBuf::from).filter(|p| p.is_absolute())?;
    if cfg!(target_os = "macos") {
        return Some(home.join("Library/Application Support/rein-app"));
    }
    Some(home.join(fallback).join("rein-app"))
}

/// `$XDG_STATE_HOME/rein-app` (default `~/.local/state/rein-app`); on Windows
/// `%LOCALAPPDATA%\rein-app` (§Windows window W6: the XDG variables are ignored there).
pub fn state_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if cfg!(windows) {
        return windows_dir(env("LOCALAPPDATA"));
    }
    xdg(env, "XDG_STATE_HOME", ".local/state")
}

/// `$XDG_CONFIG_HOME/rein-app` (default `~/.config/rein-app`), where `config.toml` lives; on
/// Windows `%APPDATA%\rein-app` (§Windows window W6).
pub fn config_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if cfg!(windows) {
        return windows_dir(env("APPDATA"));
    }
    xdg(env, "XDG_CONFIG_HOME", ".config")
}

/// `<value>\rein-app` when `value` is a drive-absolute Windows path (`X:\…`). A network
/// share is refused: the lock and the handoff directory must sit under the profile's own
/// permissions (security review of E-43). Checked as text, so the rule is the same (and
/// testable) on every platform.
fn windows_dir(value: Option<String>) -> Option<PathBuf> {
    let v = value?;
    is_drive_path(&v).then(|| PathBuf::from(format!("{v}\\rein-app")))
}

/// `X:\…` — absolute on a local drive; never resolved against a working directory or `PATH`.
pub fn is_drive_path(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

/// The process environment, as the `env` argument above expects.
pub fn process_env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)] // Windows ignores XDG (W6); see the test below
    fn state_dir_follows_xdg_then_home() {
        let set = |k: &str| match k {
            "XDG_STATE_HOME" => Some("/s".into()),
            "HOME" => Some("/home/u".into()),
            _ => None,
        };
        assert_eq!(state_dir(&set), Some(PathBuf::from("/s/rein-app")));
        let unset = |k: &str| (k == "HOME").then(|| "/home/u".to_string());
        let want = if cfg!(target_os = "macos") {
            "/home/u/Library/Application Support/rein-app"
        } else {
            "/home/u/.local/state/rein-app"
        };
        assert_eq!(state_dir(&unset), Some(PathBuf::from(want)));
        assert_eq!(state_dir(&|_: &str| None), None);
        let cfg = |k: &str| match k {
            "XDG_CONFIG_HOME" => Some("/c".into()),
            "HOME" => Some("/home/u".into()),
            _ => None,
        };
        assert_eq!(config_dir(&cfg), Some(PathBuf::from("/c/rein-app")));
    }

    #[test]
    fn windows_dirs_need_an_absolute_windows_path() {
        assert_eq!(
            windows_dir(Some("C:\\Users\\me\\AppData\\Local".into())),
            Some(PathBuf::from("C:\\Users\\me\\AppData\\Local\\rein-app"))
        );
        for bad in ["", "relative", "C:", "C:rel", "/home/me", "\\\\srv\\home"] {
            assert_eq!(windows_dir(Some(bad.into())), None, "{bad}");
        }
        assert_eq!(windows_dir(None), None);
    }
}
