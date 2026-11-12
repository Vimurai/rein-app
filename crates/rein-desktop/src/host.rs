//! The `Host` seam (desktop.md §Windows window W1, D-038): everything that differs between
//! the window on Linux / macOS and the window on Windows. `LocalHost` is the Linux and macOS
//! behaviour; `WslHost` runs every project through `wsl.exe`. The IPC table, the pty flow
//! control and the web UI are the same on both. Doctor, release and profile actions join
//! the seam with E-39 / E-45.

use std::path::{Path, PathBuf};

use crate::cli::{CliAction, HealthView};
use crate::ipc::IpcError;
use crate::pty::ChildSpec;

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
}
