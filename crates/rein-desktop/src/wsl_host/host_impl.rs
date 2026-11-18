//! `WslHost` as the window's [`Host`]: every process is `wsl.exe` with a fixed argv shape.

use super::*;
use rein_platform::process::hide_console;

impl<S: WslShell + Send + Sync> Host for WslHost<S> {
    fn in_wsl(&self) -> bool {
        true
    }

    fn project_dir(&self, input: &str) -> Result<(PathBuf, crate::host::Found), IpcError> {
        let d = self.probe_distro()?;
        let linux = wsl_path::to_linux(input, &d.name).map_err(|e| {
            let kind = match e {
                PathError::NotWsl { .. } => "not_wsl_path",
                PathError::OtherDistro { .. } => "other_distro",
                PathError::Unsafe(_) => "unsafe_path",
            };
            err(kind, e.to_string())
        })?;
        // `/` is never a project, and the validation could not tell it from a failed `--cd`.
        if linux == "/" {
            return Err(err(
                "not_a_directory",
                "the root directory is not a project",
            ));
        }
        let out = self.run(&[
            "-d",
            &d.name,
            "--cd",
            &linux,
            "-e",
            "sh",
            "-c",
            VALIDATE_SCRIPT,
        ]);
        match out {
            // 0: a project; 3: a directory without `.ai/`; 6: inside a project (V9 component
            // 1) — each prints the canonical path first; 6 the parent second.
            Ok(o) if matches!(o.code, Some(0 | 3 | 6)) => {
                use crate::host::Found;
                let text = String::from_utf8(o.stdout).ok();
                let mut lines = text.as_deref().unwrap_or("").lines();
                let canon = lines.next().filter(|c| wsl_path::check_linux(c).is_ok());
                let parent = lines.next().filter(|c| wsl_path::check_linux(c).is_ok());
                match (o.code, canon, parent) {
                    (Some(0), Some(c), _) => Ok((PathBuf::from(c), Found::Project)),
                    (Some(3), Some(c), _) => Ok((PathBuf::from(c), Found::Plain)),
                    (Some(6), Some(c), Some(p)) => {
                        Ok((PathBuf::from(c), Found::Inside(PathBuf::from(p))))
                    }
                    (Some(3 | 6), _, _) => Err(err(
                        "no_ai",
                        "not a Rein project (no .ai/ directory) — run `rein init` there first",
                    )),
                    _ => Err(err("unsafe_path", "not an absolute POSIX path")),
                }
            }
            Ok(o) if o.code == Some(4) => Err(err(
                "rein_missing",
                "rein is not installed inside WSL — install it there with `rein-app install`",
            )),
            _ => Err(err("not_a_directory", "not a directory")),
        }
    }

    /// Listing inside WSL would be a new `wsl.exe` call shape (§Windows window W2 lists them
    /// all), so `:open` offers no directory completion on Windows in this revision.
    /// `:open` completion inside WSL (W4): `--cd <parent>` and a constant listing script, 5 s,
    /// 64 KiB; only `name/` lines count; any failure is an empty list, never an error line.
    fn complete_dir(&self, prefix: &str) -> Result<Vec<String>, IpcError> {
        if wsl_path::check_linux(prefix).is_err() {
            return Ok(Vec::new());
        }
        let Ok(d) = self.probe_distro() else {
            return Ok(Vec::new());
        };
        let (parent, stem) = crate::host::split_prefix(prefix);
        let script = if parent == "/" {
            LIST_ROOT_SCRIPT
        } else {
            LIST_SCRIPT
        };
        let out = self.run_within(
            &["-d", &d.name, "--cd", parent, "-e", "sh", "-c", script],
            LIST_LIMIT,
            CALL_CAP,
        );
        let Ok(out) = out.map(|o| o.stdout).and_then(|b| {
            String::from_utf8(b).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }) else {
            return Ok(Vec::new());
        };
        let dirs = out
            .lines()
            .filter_map(|l| l.strip_suffix('/'))
            .map(String::from)
            .collect::<Vec<_>>();
        Ok(crate::host::completions(parent, stem, dirs))
    }

    /// `rein doctor --scope install --json` inside WSL, from the Linux home (W4), parsed here
    /// by the shared strict parser (120 s, 4 MiB). WSL absent or without a distro is
    /// `unknown` with the command that fixes it.
    fn doctor(&self) -> crate::cli::HealthView {
        let d = match self.probe_distro() {
            Ok(d) => d,
            Err(e) => {
                let mut v = crate::cli::unknown(e.message);
                v.setup = match e.kind {
                    "wsl_absent" => Some("wsl --install".into()),
                    "wsl_no_distro" => Some("wsl --install -d Ubuntu".into()),
                    _ => None,
                };
                return v;
            }
        };
        let args = ["-d", &d.name, "-e", &d.shell, "-lc", DOCTOR_SCRIPT];
        match self.run_within(&args, crate::cli::DOCTOR_LIMIT, crate::cli::DOCTOR_CAP) {
            Ok(o) => crate::cli::health_from(o.code, &o.stdout),
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {
                crate::cli::unknown("rein doctor did not finish within 120 seconds")
            }
            Err(e) => crate::cli::unknown(format!("cannot run rein doctor inside WSL: {e}")),
        }
    }

    /// `~/.rein/VERSION` inside WSL (updates.md component 1): one constant line, 5 s, 65 bytes.
    fn rein_version(&self) -> Option<Vec<u8>> {
        let d = self.probe_distro().ok()?;
        let args = ["-d", &d.name, "-e", "sh", "-c", VERSION_SCRIPT];
        let o = self
            .run_within(&args, crate::updates::VERSION_LIMIT, 65)
            .ok()?;
        (o.code == Some(0)).then_some(o.stdout)
    }

    /// The `rein-app` CLI inside WSL (W4): the user's login shell runs `exec rein-app …` with
    /// the action's constant argv. Exit 127: the Linux package is not installed there.
    fn run_cli(
        &self,
        action: &crate::cli::CliAction,
        line: &mut dyn FnMut(String),
    ) -> Result<Option<i32>, IpcError> {
        let d = self.probe_distro()?;
        let script = cli_script(action)?;
        let args: Vec<String> = ["-d", &d.name, "-e", &d.shell, "-lc", &script]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let code = self
            .runner
            .run(Path::new(&self.wsl_exe), &args, line)
            .map_err(|e| err("cli", format!("cannot run wsl.exe: {e}")))?;
        if code == Some(127) {
            return Err(err(
                "cli_missing",
                "rein-app is not installed inside WSL, or not on its login shell's PATH — install the Linux package there",
            ));
        }
        Ok(code)
    }

    fn distro(&self) -> Option<String> {
        self.distro_name()
    }

    /// `\\wsl.localhost\<distro>\home\<user>` (W3): the dialog starts in the Linux home; the
    /// folder picked there maps back through `wsl_path`.
    fn dialog_start(&self) -> Option<PathBuf> {
        let d = self.probe_distro().ok()?;
        Some(PathBuf::from(unc_of(&d.name, &d.home)))
    }

    fn rein_start(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
    ) -> Result<ChildSpec, IpcError> {
        let d = self.probe_distro()?;
        // V8: the app's own server through the shim, or the user's (the script before E-57).
        let script = match server {
            crate::tmux_server::Server::App => crate::tmux_server::start_script_wsl(),
            crate::tmux_server::Server::User => START_SCRIPT.to_string(),
        };
        let project = project.to_string_lossy();
        wsl_path::check_linux(&project).map_err(|e| err("unsafe_path", e.to_string()))?;
        Ok(ChildSpec {
            program: PathBuf::from(&self.wsl_exe),
            args: [
                "-d", &d.name, "--cd", &project, "-e", &d.shell, "-lic", &script,
            ]
            .iter()
            .map(OsString::from)
            .collect(),
            cwd: None,
            env_remove: ENV_REMOVED,
            env_set: &[],
            tty_line: true,
        })
    }

    /// The set-up child (V9 component 2): the tab's own `wsl.exe` start with the set-up
    /// constant — the tty line first, `rein init`, then the start on `server`.
    fn rein_setup(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
    ) -> Result<ChildSpec, IpcError> {
        let mut spec = self.rein_start(project, server)?;
        if let Some(last) = spec.args.last_mut() {
            *last = OsString::from(crate::project_setup::setup_script(server, true));
        }
        Ok(spec)
    }

    /// `wsl.exe -d D --cd P -e <shell> -lic <constant>`: the start's path, with
    /// `rein init [--check]` in place of `rein start`.
    fn rein_command(
        &self,
        project: &Path,
        which: crate::project_setup::ReinRun,
    ) -> Result<Command, IpcError> {
        let d = self.probe_distro()?;
        let project = project.to_string_lossy();
        wsl_path::check_linux(&project).map_err(|e| err("unsafe_path", e.to_string()))?;
        let mut cmd = Command::new(&self.wsl_exe);
        cmd.args([
            "-d",
            &d.name,
            "--cd",
            &project,
            "-e",
            &d.shell,
            "-lic",
            which.wsl_script(),
        ]);
        for k in ENV_REMOVED {
            cmd.env_remove(k);
        }
        hide_console(&mut cmd);
        Ok(cmd)
    }

    /// `wsl.exe -d D --cd P -e <shell> -lic <ADD_SCRIPT> rein-app <email> <spec>` (security
    /// review C3): the constant re-checks both words under `IFS=` and `noglob` before `exec`.
    fn rein_team_add(
        &self,
        project: &Path,
        values: &crate::team::Add,
    ) -> Result<Command, IpcError> {
        let d = self.probe_distro()?;
        let project = project.to_string_lossy();
        wsl_path::check_linux(&project).map_err(|e| err("unsafe_path", e.to_string()))?;
        let mut cmd = Command::new(&self.wsl_exe);
        cmd.args([
            "-d",
            &d.name,
            "--cd",
            &project,
            "-e",
            &d.shell,
            "-lic",
            crate::team::ADD_SCRIPT,
            crate::team::ADD_ARGV0,
            &values.email,
            &values.spec,
        ]);
        for k in ENV_REMOVED {
            cmd.env_remove(k);
        }
        hide_console(&mut cmd);
        Ok(cmd)
    }

    /// The panel files through the distro's UNC path — no process (panels.md §API; measured
    /// in E-59 against `wsl.exe` + a script). The project is the tab's validated Linux path;
    /// `.` and `..` segments are refused, since the UNC path is not made canonical.
    #[allow(unused_variables)] // the read is Windows-only; elsewhere this host only runs in tests
    fn read_project_file(
        &self,
        project: &Path,
        which: crate::project_file::ProjectFile,
        read: bool,
    ) -> Result<crate::project_file::FileBytes, crate::project_file::Unavailable> {
        use crate::project_file::Unavailable;
        let p = project.to_string_lossy();
        // `.`, `..`, and a segment ending in a dot or a space (Windows strips those, so
        // `proj.` would read `proj`'s files — E-59 review) are refused.
        let odd = |s: &str| s == "." || s == ".." || s.ends_with('.') || s.ends_with(' ');
        if wsl_path::check_linux(&p).is_err() || p.split('/').any(odd) {
            return Err(Unavailable::Unreadable);
        }
        let d = self.probe_distro().map_err(|_| Unavailable::Unreadable)?;
        #[cfg(windows)]
        {
            let ai = PathBuf::from(format!("{}\\.ai", unc_of(&d.name, &p)));
            crate::project_file::read_unc(&ai, which, read)
        }
        #[cfg(not(windows))]
        {
            let _ = d;
            Err(Unavailable::Unreadable)
        }
    }

    /// `wsl.exe -d D -e sh -s -- <tag>` with the setup script on stdin (V8 3a): the tmux home
    /// inside WSL, and the live lookup's answer on stdout (V12 component 2). Only the distro's
    /// name and the tag (64 hex) are put in argv; the script is a constant.
    fn tmux_setup(&self, tag: &str) -> Result<Vec<crate::tmux_server::SessionRow>, &'static str> {
        if !crate::session_tag::is_tag(tag) {
            return Err(crate::tmux_server::setup_reason(Some(26)));
        }
        let d = self
            .probe_distro()
            .map_err(|_| crate::tmux_server::setup_reason(Some(-1)))?;
        let argv: Vec<OsString> = ["-d", d.name.as_str(), "-e", "sh", "-s", "--", tag]
            .into_iter()
            .map(OsString::from)
            .collect();
        crate::tmux_server::setup_outcome(self.shell.run_script_output(
            &self.wsl_exe,
            &argv,
            &crate::tmux_server::setup_script(),
            crate::tmux_server::SETUP_LIMIT,
            crate::tmux_server::SETUP_CAP,
        ))
    }

    /// The tab's own `wsl.exe` start (same distro, `--cd`, shell and flags) running the attach
    /// script, the tag its one positional word (V12 component 3); the tty line first.
    fn rein_attach(
        &self,
        project: &Path,
        tag: &str,
        adopt: Option<&str>,
    ) -> Result<ChildSpec, IpcError> {
        if !crate::session_tag::is_tag(tag)
            || adopt.is_some_and(|id| !crate::tmux_server::session_id_ok(id))
        {
            return Err(err("tmux_setup", "the app's project tag was refused"));
        }
        let mut spec = self.rein_start(project, crate::tmux_server::Server::App)?;
        if let Some(last) = spec.args.last_mut() {
            *last = OsString::from(crate::tmux_server::attach_script_wsl());
        }
        spec.args
            .extend([OsString::from("sh"), OsString::from(tag)]);
        spec.args.extend(adopt.map(OsString::from));
        Ok(spec)
    }

    /// `wsl.exe -d D -e sh -ls -- <args>` with the constant script on stdin: the login shell's
    /// `PATH` finds the tmux `rein start` runs. Only the validated words follow `--`.
    fn tmux_overlay(&self, args: &[String], alive: &dyn Fn() -> bool) -> Result<(), String> {
        let d = self.probe_distro().map_err(|e| e.message)?;
        let mut argv: Vec<OsString> = ["-d", d.name.as_str(), "-e", "sh", "-ls", "--"]
            .into_iter()
            .map(OsString::from)
            .collect();
        argv.extend(args.iter().map(OsString::from));
        tmux_overlay::outcome(self.shell.run_script(
            &self.wsl_exe,
            &argv,
            tmux_overlay::SCRIPT,
            tmux_overlay::LIMIT,
            alive,
        ))
    }

    /// `wsl.exe -d D -e sh -ls -- <args>` with the status script on stdin, as the overlay.
    fn tmux_agents(&self, args: &[String]) -> Result<Vec<u8>, &'static str> {
        let d = self.probe_distro().map_err(|_| crate::agents::NOT_ASKED)?;
        let mut argv: Vec<OsString> = ["-d", d.name.as_str(), "-e", "sh", "-ls", "--"]
            .into_iter()
            .map(OsString::from)
            .collect();
        argv.extend(args.iter().map(OsString::from));
        crate::agents::status_outcome(self.shell.run_script_output(
            &self.wsl_exe,
            &argv,
            crate::agents::SCRIPT,
            crate::agents::STATUS_LIMIT,
            crate::agents::STATUS_CAP,
        ))
    }

    /// The tab's own `wsl.exe` start made into `--kill --yes`: the same distro, `--cd`, shell
    /// and flags; a constant script with no quote character.
    fn rein_kill(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
    ) -> Result<(), &'static str> {
        let d = self
            .probe_distro()
            .map_err(|_| crate::agents::KILL_NOT_STARTED)?;
        let script = match server {
            crate::tmux_server::Server::App => crate::tmux_server::kill_script(),
            crate::tmux_server::Server::User => KILL_SCRIPT.to_string(),
        };
        let project = project.to_string_lossy();
        wsl_path::check_linux(&project).map_err(|_| crate::agents::KILL_NOT_STARTED)?;
        let run = self.run_within(
            &[
                "-d", &d.name, "--cd", &project, "-e", &d.shell, "-lic", &script,
            ],
            crate::agents::KILL_LIMIT,
            crate::agents::KILL_CAP,
        );
        crate::agents::kill_outcome(match run {
            Ok(o) => Ok(Some((o.code, o.stdout))),
            Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(None),
            Err(e) => Err(e),
        })
    }
}
