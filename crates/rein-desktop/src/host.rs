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

/// What a validated directory is to Rein (V9 component 1; the E-63 review's rework 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// It has `.ai/`.
    Project,
    /// No `.ai/` of its own, but a parent has one: that parent, the project it belongs to.
    Inside(PathBuf),
    /// Neither: a folder that can be set up.
    Plain,
}

pub trait Host: Send + Sync {
    /// A project as the user gave it (argv, `:open`, the folder dialog, a second start) →
    /// the validated, canonical path the pty child runs in. May start processes (on Windows
    /// it asks inside WSL); call it off the UI thread.
    /// [`project_dir`](Self::project_dir) refusing a directory without `.ai/` — the
    /// behaviour before V9; the tests of each host's validation use it.
    #[cfg(test)]
    fn project(&self, input: &str) -> Result<PathBuf, IpcError> {
        match self.project_dir(input)? {
            (p, Found::Project) => Ok(p),
            _ => Err(crate::ipc::err(
                "no_ai",
                "not a Rein project (no .ai/ directory) — run `rein init` there first",
            )),
        }
    }

    /// [`project`](Self::project) without the `.ai/` requirement (V9 component 1, E-63): the
    /// canonical directory and what it is to Rein — asked in the same validation as before,
    /// so a normal tab open gains no process.
    fn project_dir(&self, input: &str) -> Result<(PathBuf, Found), IpcError>;

    /// The pty child of Set up this project (V9 component 2): `rein init` in `project`, then
    /// on exit 0 the tab's ordinary start on `server`, in the same terminal. A failed init
    /// ends the child with [`crate::project_setup::INIT_FAILED`].
    fn rein_setup(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
    ) -> Result<ChildSpec, IpcError>;

    /// `rein init --check` or `rein init` in `project` as a constant command (V9 components
    /// 2–3), not yet started; [`crate::project_setup::run_lines`] runs it.
    fn rein_command(
        &self,
        project: &Path,
        which: crate::project_setup::ReinRun,
    ) -> Result<std::process::Command, IpcError>;

    /// The pty child that runs `rein start` in `project`, which [`project`](Self::project)
    /// returned, reaching `server` (V8): the app's own through the shim, or the user's.
    fn rein_start(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
    ) -> Result<ChildSpec, IpcError>;

    /// Read (or, with `read` false, only stat) one of the two panel files under `project`
    /// (panels.md §API, E-59): a regular file only, size limits per file; Linux / macOS
    /// directly, Windows through the distro's UNC path.
    fn read_project_file(
        &self,
        project: &Path,
        which: crate::project_file::ProjectFile,
        read: bool,
    ) -> Result<crate::project_file::FileBytes, crate::project_file::Unavailable>;

    /// Make or check the tmux home before a tab opens on the app's server (V8 3a): the
    /// constant setup script on stdin — `/bin/sh -s` here, `sh -s` inside WSL on Windows.
    /// With `sweep` (the app's first tab on its server, V11 component 3) the same process
    /// also ends a leftover `rein-app` server. `Err` is the app's fixed sentence for why not.
    fn tmux_setup(&self, sweep: bool) -> Result<(), &'static str>;

    /// Quit (V11 component 2): [`crate::tmux_server::KILL_SERVER`] on stdin, at most
    /// [`crate::tmux_server::QUIT_LIMIT`]. `false`: it did not finish in time or could not start.
    fn tmux_kill_server(&self) -> bool;

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

    /// The agents' status script (V9 component 4, E-62) with `args` — the tab's client tty
    /// and server word, checked by the caller: `/bin/sh -s` here, `sh -ls` inside WSL. Its
    /// output, or the app's fixed sentence for why not.
    fn tmux_agents(&self, args: &[String]) -> Result<Vec<u8>, &'static str>;

    /// `rein start --kill --yes` in `project` on `server` (V9 component 4): the same path the
    /// tab's `rein start` took, a constant command. `Err` is the app's fixed sentence.
    fn rein_kill(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
    ) -> Result<(), &'static str>;

    /// The WSL distro a project belongs to, recorded with recent projects (Windows only).
    fn distro(&self) -> Option<String> {
        None
    }

    /// Where the folder dialog opens (E-40): the home directory; on Windows the Linux home
    /// as `\\wsl.localhost\<distro>\home\<user>` (W3).
    fn dialog_start(&self) -> Option<PathBuf>;
}
