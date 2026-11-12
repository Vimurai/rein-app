//! `LocalHost` (desktop.md §Components 2, §Windows window W1): the window on Linux and
//! macOS. A project is a local directory containing `.ai/`; the pty child is exactly
//! `<home>/.rein/bin/rein start` with the project as its cwd. This is the E-35 behaviour,
//! moved behind the `Host` seam unchanged.

use std::path::{Path, PathBuf};

use crate::cli::{self, CliAction, HealthView, Runner};
use crate::host::Host;
use crate::ipc::{IpcError, err};
use crate::project_path;
use crate::pty::ChildSpec;

/// Variables removed from the child's environment. `CLAUDECODE` marks a Claude Code
/// session (desktop.md §Components 2); `TMUX`/`TMUX_PANE` would make `rein start` run
/// `tmux switch-client` on an outer client instead of attaching inside this pty.
pub const ENV_REMOVED: &[&str] = &["CLAUDECODE", "TMUX", "TMUX_PANE"];
/// Variables set in the child's environment.
pub const ENV_SET: &[(&str, &str)] = &[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

/// The installed `rein` launcher.
pub fn rein_bin(home: &Path) -> PathBuf {
    home.join(".rein/bin/rein")
}

impl ChildSpec {
    /// `<home>/.rein/bin/rein start` in `project` — the only child `LocalHost` starts.
    pub fn rein_start(home: &Path, project: &Path) -> Self {
        Self {
            program: rein_bin(home),
            args: vec!["start".into()],
            cwd: Some(project.to_path_buf()),
            env_remove: ENV_REMOVED,
            env_set: ENV_SET,
        }
    }
}

pub struct LocalHost {
    home: Option<PathBuf>,
    /// The `rein-app` CLI beside the window, else on `PATH`, resolved once (D-045).
    rein_app: Option<PathBuf>,
    runner: Box<dyn Runner>,
}

impl LocalHost {
    /// `home` is `$HOME`; one that is not absolute counts as unset.
    pub fn new(home: Option<PathBuf>) -> Self {
        Self::with_runner(
            home,
            cli::find_rein_app(std::env::var_os("PATH")),
            Box::new(cli::SystemRunner),
        )
    }

    pub fn with_runner(
        home: Option<PathBuf>,
        rein_app: Option<PathBuf>,
        runner: Box<dyn Runner>,
    ) -> Self {
        Self {
            home: home.filter(|h| h.is_absolute()),
            rein_app,
            runner,
        }
    }
}

impl Host for LocalHost {
    fn project(&self, input: &str) -> Result<PathBuf, IpcError> {
        validate_project(input)
    }

    fn rein_start(&self, project: &Path) -> Result<ChildSpec, IpcError> {
        let home = self
            .home
            .as_deref()
            .ok_or_else(|| err("rein_missing", "HOME is not set to an absolute path"))?;
        installed_rein(home)?;
        Ok(ChildSpec::rein_start(home, project))
    }

    fn complete_dir(&self, prefix: &str) -> Result<Vec<String>, IpcError> {
        complete_dir(prefix)
    }

    fn dialog_start(&self) -> Option<PathBuf> {
        self.home.clone()
    }

    fn doctor(&self) -> HealthView {
        match &self.home {
            Some(h) => cli::local_doctor(h),
            None => cli::unknown("HOME is not set to an absolute path"),
        }
    }

    fn run_cli(
        &self,
        action: &CliAction,
        line: &mut dyn FnMut(String),
    ) -> Result<Option<i32>, IpcError> {
        let program = self.rein_app.as_deref().ok_or_else(cli::cli_missing)?;
        self.runner
            .run(program, &action.argv(), line)
            .map_err(|e| err("cli", format!("cannot run rein-app: {e}")))
    }
}

/// Directories under the prefix's parent whose name starts with the typed stem. Names that
/// fail the path text rule are left out, so nothing unprintable reaches the command line.
pub fn complete_dir(prefix: &str) -> Result<Vec<String>, IpcError> {
    project_path::check(prefix).map_err(|why| err("unsafe_path", why))?;
    let (parent, stem) = crate::host::split_prefix(prefix);
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Ok(Vec::new());
    };
    let dirs = entries.flatten().filter_map(|e| {
        let dir = std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir());
        dir.then(|| e.file_name().into_string().ok()).flatten()
    });
    Ok(crate::host::completions(parent, stem, dirs))
}

/// A project directory as `tab_open` accepts it: the text rule, canonical, a directory, and
/// containing `.ai/`. Nothing inside `.ai/` is read.
///
/// Residual risk (security review of E-35, P3): the check and the child's `chdir` are
/// separate steps, so a process that can write to a parent directory could swap a path
/// component in between. Such a process already runs as the user (the window starts only the
/// user's own `rein`), so the swap gains it nothing it could not do directly.
pub fn validate_project(s: &str) -> Result<PathBuf, IpcError> {
    let unsafe_path = |why: &str| err("unsafe_path", why);
    let p = project_path::check(s).map_err(unsafe_path)?;
    let canon = p
        .canonicalize()
        .map_err(|_| err("not_a_directory", "not a directory"))?;
    // The canonical form (symlinks resolved) must satisfy the same rule.
    project_path::check(&canon.to_string_lossy()).map_err(unsafe_path)?;
    if !canon.is_dir() {
        return Err(err("not_a_directory", "not a directory"));
    }
    if !canon.join(".ai").is_dir() {
        return Err(err(
            "no_ai",
            "not a Rein project (no .ai/ directory) — run `rein init` there first",
        ));
    }
    Ok(canon)
}

/// The installed `rein` launcher, if it is an executable file.
pub fn installed_rein(home: &Path) -> Result<PathBuf, IpcError> {
    use std::os::unix::fs::PermissionsExt;
    let bin = rein_bin(home);
    match std::fs::metadata(&bin) {
        Ok(m) if m.is_file() && m.permissions().mode() & 0o111 != 0 => Ok(bin),
        _ => Err(err(
            "rein_missing",
            "rein is not installed — install it with `rein-app install`",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn complete_dir_lists_matching_directories_only() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        for d in ["proj", "prod", ".private", "other"] {
            std::fs::create_dir(base.join(d)).unwrap();
        }
        std::fs::write(base.join("profile.txt"), "").unwrap();
        let b = base.to_str().unwrap();
        assert_eq!(
            complete_dir(&format!("{b}/pro")).unwrap(),
            [format!("{b}/prod/"), format!("{b}/proj/")]
        );
        assert_eq!(
            complete_dir(&format!("{b}/.p")).unwrap(),
            [format!("{b}/.private/")]
        );
        assert_eq!(
            complete_dir(&format!("{b}/")).unwrap().len(),
            3,
            "hidden left out"
        );
        assert!(complete_dir(&format!("{b}/missing/x")).unwrap().is_empty());
        assert_eq!(complete_dir("relative").unwrap_err().kind, "unsafe_path");
    }

    #[test]
    fn the_production_child_is_exactly_rein_start_in_the_project() {
        let spec = ChildSpec::rein_start(Path::new("/home/u"), Path::new("/home/u/Dev/p"));
        assert_eq!(spec.program, PathBuf::from("/home/u/.rein/bin/rein"));
        assert_eq!(spec.args, vec![OsString::from("start")]);
        assert_eq!(spec.cwd, Some(PathBuf::from("/home/u/Dev/p")));
        let argv = spec.command().get_argv().clone();
        assert_eq!(
            argv,
            vec![OsString::from("/home/u/.rein/bin/rein"), "start".into()]
        );
        let cmd = spec.command();
        assert_eq!(
            cmd.get_cwd().map(PathBuf::from),
            Some(PathBuf::from("/home/u/Dev/p"))
        );
        for k in ENV_REMOVED {
            assert_eq!(cmd.get_env(k), None, "{k} must be unset");
        }
        assert_eq!(
            cmd.get_env("TERM"),
            Some(std::ffi::OsStr::new("xterm-256color"))
        );
    }
}
