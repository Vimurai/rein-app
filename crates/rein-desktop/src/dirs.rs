//! The app's own directories (desktop.md §Data Model "Files"): XDG base directories; on macOS
//! `~/Library/Application Support/rein-app` only when the XDG variable is unset.

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

/// `$XDG_STATE_HOME/rein-app` (default `~/.local/state/rein-app`).
pub fn state_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    xdg(env, "XDG_STATE_HOME", ".local/state")
}

/// The process environment, as the `env` argument above expects.
pub fn process_env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
    }
}
