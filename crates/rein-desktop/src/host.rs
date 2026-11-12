//! The `Host` seam (desktop.md §Windows window W1, D-038): everything that differs between
//! the window on Linux / macOS and the window on Windows. `LocalHost` is the Linux and macOS
//! behaviour; `WslHost` runs every project through `wsl.exe`. The IPC table, the pty flow
//! control and the web UI are the same on both. Doctor, release and profile actions join
//! the seam with E-39 / E-45.

use std::path::{Path, PathBuf};

use crate::cli::{CliAction, HealthView};
use crate::ipc::IpcError;
use crate::pty::ChildSpec;

/// The most directories one `:open` Tab completion offers.
pub const MAX_COMPLETIONS: usize = 50;

/// `/home/me/pro` → (`/home/me/`, `pro`).
pub fn split_prefix(prefix: &str) -> (&str, &str) {
    prefix.split_at(prefix.rfind('/').map_or(0, |i| i + 1))
}

/// The completions of `stem` under `parent` from the directory names found there — one rule
/// for every host: matching names only, hidden ones only when the stem starts with `.`, no
/// `.` / `..`, every full path passing the path text rule, sorted, at most 50.
pub fn completions(
    parent: &str,
    stem: &str,
    dir_names: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut out: Vec<String> = dir_names
        .into_iter()
        .filter(|n| n != "." && n != "..")
        .filter(|n| n.starts_with(stem) && (!n.starts_with('.') || stem.starts_with('.')))
        .map(|n| format!("{parent}{n}/"))
        .filter(|full| crate::project_path::check(full).is_ok())
        .collect();
    out.sort();
    out.dedup();
    out.truncate(MAX_COMPLETIONS);
    out
}

pub trait Host: Send + Sync {
    /// A project as the user gave it (argv, `:open`, the folder dialog, a second start) →
    /// the validated, canonical path the pty child runs in. May start processes (on Windows
    /// it asks inside WSL); call it off the UI thread.
    fn project(&self, input: &str) -> Result<PathBuf, IpcError>;

    /// The pty child that runs `rein start` in `project`, which [`project`](Self::project)
    /// returned.
    fn rein_start(&self, project: &Path) -> Result<ChildSpec, IpcError>;

    /// `:open` Tab completion (E-36): directories whose path starts with `prefix`, each ending
    /// in `/`, a bounded number, sorted. Hidden ones only when the typed name
    /// starts with `.`. A host that cannot list returns none.
    fn complete_dir(&self, prefix: &str) -> Result<Vec<String>, IpcError>;

    /// `rein doctor --scope install --json` from the installed tree, as a `HealthView` (E-39).
    fn doctor(&self) -> HealthView;

    /// Run the `rein-app` CLI for `action` (constant argv, D-045), each line of its output
    /// escaped to `line`; the exit code (`None`: killed by a signal).
    fn run_cli(
        &self,
        action: &CliAction,
        line: &mut dyn FnMut(String),
    ) -> Result<Option<i32>, IpcError>;

    /// Run the tmux overlay script (V6, D-050) with `args`, which
    /// [`tmux_overlay::args`](crate::tmux_overlay::args) validated: on Linux / macOS
    /// `/bin/sh -s`, on Windows `sh -ls` inside WSL. `Err` says why it was not applied.
    /// `alive`: the tab's terminal is still there; the run is stopped when it turns false.
    fn tmux_overlay(&self, args: &[String], alive: &dyn Fn() -> bool) -> Result<(), String>;

    /// The WSL distro a project belongs to, recorded with recent projects (Windows only).
    fn distro(&self) -> Option<String> {
        None
    }

    /// Where the folder dialog opens (E-40): the home directory; on Windows the Linux home
    /// as `\\wsl.localhost\<distro>\home\<user>` (W3).
    fn dialog_start(&self) -> Option<PathBuf>;
}
