//! `WslHost` (desktop.md §Windows window W2, D-038): the window on Windows. Rein stays inside
//! WSL; every process this host starts is `wsl.exe`, by absolute path, with a fixed argv
//! shape. Scripts are compile-time constants with no interpolation: the only variable argv
//! elements are the validated distro name, the validated shell path and the validated
//! project path (as `--cd`).
//!
//! Plain Rust on every platform, so its tests run everywhere against a stubbed [`Shell`];
//! only [`SystemWsl`] differs on Windows (no console window).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use rein_platform::wsl::{self, Shell, ShellOut, WslState};

use crate::host::Host;
use crate::ipc::{IpcError, err};
use crate::pty::ChildSpec;
use crate::tmux_overlay;
use crate::wsl_path::{self, PathError};

/// Learns the distro's name, the Linux home and the login shell (W2). `%s\\n` is `%s\n` once
/// sh has removed one backslash: the script holds no quote of any kind.
pub const PROBE_SCRIPT: &str = "printf %s\\\\n $WSL_DISTRO_NAME $HOME $SHELL";
/// Run with `--cd <project>`: exit 5 still in `/` (a failed `--cd` does not fail: `wsl.exe`
/// runs the command in `/` instead — D-038 addendum 1), 3 no `.ai/`, 4 rein not installed,
/// else `pwd -P`.
pub const VALIDATE_SCRIPT: &str = "case $PWD in /) exit 5;; esac; [ -d .ai ] || exit 3; [ -x ~/.rein/bin/rein ] || exit 4; pwd -P";
/// The pty child's script, run by the user's login + interactive shell (W2). `tty` names the
/// pty first: the tmux overlay finds the app's client by it (V6, D-050), and the host takes
/// that line out of what the terminal shows (`tmux_overlay::take_tty_line`).
pub const START_SCRIPT: &str = "tty; exec ~/.rein/bin/rein start";
/// Health inside WSL, from the Linux home (W4, D-045 addendum 1).
pub const DOCTOR_SCRIPT: &str = "cd; exec ~/.rein/bin/rein doctor --scope install --json";
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

/// What the probe learned. Every field has passed [`check_name`] / [`check_posix`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    pub name: String,
    pub home: String,
    pub shell: String,
}

/// A [`Shell`] whose calls can carry their own limits: doctor (120 s, 4 MiB) and directory
/// listing (5 s, 64 KiB) differ from the 30 s default (§Windows window "Limits", W4).
pub trait WslShell: Shell {
    fn run_within(
        &self,
        program: &str,
        args: &[OsString],
        limit: Duration,
        cap: u64,
    ) -> io::Result<ShellOut>;

    /// Run `program args` with the tmux overlay script on stdin (V6), output discarded: the
    /// exit code, `None` when it did not finish within `limit`.
    fn run_script(
        &self,
        program: &str,
        args: &[OsString],
        script: &str,
        limit: Duration,
        alive: &dyn Fn() -> bool,
    ) -> io::Result<Option<i32>>;
}

pub struct WslHost<S: WslShell + Send + Sync> {
    shell: S,
    /// Streams the `rein-app` CLI's lines (release and profile actions, W4).
    runner: Box<dyn crate::cli::Runner>,
    /// `%SystemRoot%\System32\wsl.exe` — never a `PATH` search.
    wsl_exe: String,
    /// `wsl.distro` from `config.toml` (E-37 supplies it; `None` = WSL's default).
    configured: Option<String>,
    probed: Mutex<Option<Distro>>,
}

impl WslHost<SystemWsl> {
    /// The real host: `wsl.exe` under `%SystemRoot%`.
    pub fn system(configured: Option<String>) -> Self {
        Self::new(
            SystemWsl,
            Box::new(crate::cli::SystemRunner),
            wsl_exe(std::env::var("SystemRoot").ok()),
            configured,
        )
    }
}

impl<S: WslShell + Send + Sync> WslHost<S> {
    pub fn new(
        shell: S,
        runner: Box<dyn crate::cli::Runner>,
        wsl_exe: String,
        configured: Option<String>,
    ) -> Self {
        Self {
            shell,
            runner,
            wsl_exe,
            configured,
            probed: Mutex::new(None),
        }
    }

    fn run(&self, args: &[&str]) -> io::Result<ShellOut> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        self.shell.run(&self.wsl_exe, &args)
    }

    fn run_within(&self, args: &[&str], limit: Duration, cap: u64) -> io::Result<ShellOut> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        self.shell.run_within(&self.wsl_exe, &args, limit, cap)
    }

    /// The window's distro name, if WSL has been probed successfully (recent projects
    /// record it, E-38).
    pub fn distro_name(&self) -> Option<String> {
        self.probed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|d| d.name.clone())
    }

    /// The window's distro, probed once (W2). Refusals name the command that fixes them.
    pub fn probe_distro(&self) -> Result<Distro, IpcError> {
        let mut probed = self.probed.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(d) = probed.as_ref() {
            return Ok(d.clone());
        }
        let d = self.probe()?;
        *probed = Some(d.clone());
        Ok(d)
    }

    fn probe(&self) -> Result<Distro, IpcError> {
        match wsl::detect(&self.shell, &self.wsl_exe) {
            WslState::Absent => {
                return Err(err(
                    "wsl_absent",
                    "WSL is not installed — run `wsl --install` in an administrator terminal, then restart Windows",
                ));
            }
            WslState::NoDistro => {
                return Err(err(
                    "wsl_no_distro",
                    "WSL has no Linux distribution — run `wsl --install -d Ubuntu`",
                ));
            }
            WslState::Ready => {}
        }
        let mut args: Vec<&str> = Vec::new();
        let configured = match &self.configured {
            Some(want) => Some(self.listed(want)?),
            None => None,
        };
        if let Some(name) = &configured {
            args.extend(["-d", name.as_str()]);
        }
        args.extend(["-e", "sh", "-c", PROBE_SCRIPT]);
        let out = self
            .run(&args)
            .ok()
            .filter(ShellOut::ok)
            .ok_or_else(|| err("wsl_failed", "wsl.exe did not answer the distro probe"))?;
        parse_probe(&out.stdout)
    }

    /// The configured distro as `wsl.exe --list` spells it, if it is installed.
    fn listed(&self, want: &str) -> Result<String, IpcError> {
        check_name(want).map_err(|why| err("wsl_distro_missing", why))?;
        let names = self
            .run(&["--list", "--quiet"])
            .ok()
            .filter(ShellOut::ok)
            .map(|o| wsl::distros(&o.stdout))
            .unwrap_or_default();
        names
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(want))
            .ok_or_else(|| {
                err(
                    "wsl_distro_missing",
                    format!("the WSL distro {want} set in config.toml is not installed"),
                )
            })
    }
}

impl<S: WslShell + Send + Sync> Host for WslHost<S> {
    fn project(&self, input: &str) -> Result<PathBuf, IpcError> {
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
            Ok(o) if o.code == Some(0) => {
                let canon = String::from_utf8(o.stdout)
                    .map_err(|_| err("not_a_directory", "not a directory"))?;
                let canon = canon.strip_suffix('\n').unwrap_or(&canon);
                wsl_path::check_linux(canon).map_err(|e| err("unsafe_path", e.to_string()))?;
                Ok(PathBuf::from(canon))
            }
            Ok(o) if o.code == Some(3) => Err(err(
                "no_ai",
                "not a Rein project (no .ai/ directory) — run `rein init` there first",
            )),
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

    /// `wsl.exe -d D -e sh -s` with the setup script on stdin (V8 3a): the tmux home inside
    /// WSL. Only the distro's name is put in argv; the script is a constant.
    fn tmux_setup(&self) -> Result<(), &'static str> {
        let d = self
            .probe_distro()
            .map_err(|_| crate::tmux_server::setup_reason(Some(-1)))?;
        let argv: Vec<OsString> = ["-d", d.name.as_str(), "-e", "sh", "-s"]
            .into_iter()
            .map(OsString::from)
            .collect();
        crate::tmux_server::setup_outcome(self.shell.run_script(
            &self.wsl_exe,
            &argv,
            &crate::tmux_server::setup_script(),
            crate::tmux_server::SETUP_LIMIT,
            &|| true,
        ))
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
}

/// `<SystemRoot>\System32\wsl.exe`, with `SystemRoot` only if it is drive-absolute: a
/// relative value would be resolved against the working directory, or by the pty layer
/// against every `PATH` entry (security review of E-43). Otherwise the default location.
pub fn wsl_exe(system_root: Option<String>) -> String {
    let root = system_root
        .filter(|r| crate::dirs::is_drive_path(r))
        .unwrap_or_else(|| "C:\\Windows".into());
    format!("{}\\System32\\wsl.exe", root.trim_end_matches('\\'))
}

/// The Windows UNC form of a Linux path in `distro` (the inverse of `wsl_path::to_linux`).
pub fn unc_of(distro: &str, linux: &str) -> String {
    format!(
        "\\\\wsl.localhost\\{distro}{}",
        linux.trim_end_matches('/').replace('/', "\\")
    )
}

/// The probe's three lines: distro name, home, shell (empty → `/bin/sh`).
fn parse_probe(stdout: &[u8]) -> Result<Distro, IpcError> {
    let bad = |why: &str| err("wsl_failed", format!("the WSL distro probe returned {why}"));
    let text = std::str::from_utf8(stdout).map_err(|_| bad("text that is not UTF-8"))?;
    let mut lines = text.lines();
    let name = lines.next().unwrap_or_default();
    let home = lines.next().unwrap_or_default();
    let shell = lines.next().filter(|s| !s.is_empty()).unwrap_or("/bin/sh");
    check_name(name).map_err(|_| bad("an unusable distro name"))?;
    check_posix(home).map_err(|_| bad("an unusable home directory"))?;
    check_posix(shell).map_err(|_| bad("an unusable login shell"))?;
    Ok(Distro {
        name: name.into(),
        home: home.into(),
        shell: shell.into(),
    })
}

/// A distro name: `[A-Za-z0-9._-]{1,64}`.
pub fn check_name(s: &str) -> Result<(), &'static str> {
    let ok = (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    ok.then_some(()).ok_or("not a WSL distro name")
}

/// A home or shell path: absolute POSIX, printable ASCII, no space, no `"` or `\`.
pub fn check_posix(s: &str) -> Result<(), &'static str> {
    let ok = s.starts_with('/')
        && s.len() <= 4096
        && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
        && !s.contains(['"', '\\']);
    ok.then_some(()).ok_or("not an absolute POSIX path")
}

/// The real runner for non-pty `wsl.exe` calls: the shared bounded run (30 s, 64 KiB),
/// stderr discarded, `WSLENV` removed, and on Windows no console window.
pub struct SystemWsl;

impl Shell for SystemWsl {
    fn run(&self, program: &str, args: &[OsString]) -> io::Result<ShellOut> {
        self.run_within(program, args, CALL_LIMIT, CALL_CAP)
    }
}

impl WslShell for SystemWsl {
    fn run_within(
        &self,
        program: &str,
        args: &[OsString],
        limit: Duration,
        cap: u64,
    ) -> io::Result<ShellOut> {
        let mut cmd = Self::command(program, args);
        match rein_platform::process::output(&mut cmd, limit, cap)? {
            Some((code, stdout)) => Ok(ShellOut { code, stdout }),
            None => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "wsl.exe did not finish in time",
            )),
        }
    }

    fn run_script(
        &self,
        program: &str,
        args: &[OsString],
        script: &str,
        limit: Duration,
        alive: &dyn Fn() -> bool,
    ) -> io::Result<Option<i32>> {
        tmux_overlay::run_stdin(&mut Self::command(program, args), script, limit, alive)
    }
}

impl SystemWsl {
    fn command(program: &str, args: &[OsString]) -> Command {
        let mut cmd = Command::new(program);
        cmd.args(args).stderr(Stdio::null());
        for k in ENV_REMOVED {
            cmd.env_remove(k);
        }
        hide_console(&mut cmd);
        cmd
    }
}

use rein_platform::process::hide_console;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Arc;

    const EXE: &str = "C:\\Windows\\System32\\wsl.exe";

    /// Answers by exact argv; records every call.
    #[derive(Default)]
    struct Stub {
        answers: Vec<(Vec<String>, Option<i32>, Vec<u8>)>,
        calls: Mutex<VecDeque<Vec<String>>>,
        limits: Mutex<Vec<Duration>>,
    }

    impl Stub {
        fn on(mut self, args: &[&str], code: i32, out: &[u8]) -> Self {
            self.answers.push((
                args.iter().map(|s| s.to_string()).collect(),
                Some(code),
                out.to_vec(),
            ));
            self
        }
        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().iter().cloned().collect()
        }
    }

    impl WslShell for Stub {
        fn run_within(
            &self,
            program: &str,
            args: &[OsString],
            limit: Duration,
            _cap: u64,
        ) -> io::Result<ShellOut> {
            self.limits.lock().unwrap().push(limit);
            self.run(program, args)
        }

        fn run_script(
            &self,
            program: &str,
            args: &[OsString],
            _script: &str,
            limit: Duration,
            _alive: &dyn Fn() -> bool,
        ) -> io::Result<Option<i32>> {
            self.limits.lock().unwrap().push(limit);
            self.run(program, args).map(|o| o.code)
        }
    }

    /// Records the CLI runs; answers with fixed lines and code.
    #[derive(Default)]
    struct Cli {
        calls: Arc<Mutex<Vec<Vec<String>>>>,
        code: Option<i32>,
    }

    impl crate::cli::Runner for Cli {
        fn run(
            &self,
            program: &Path,
            args: &[String],
            line: &mut dyn FnMut(String),
        ) -> io::Result<Option<i32>> {
            assert_eq!(program, Path::new(EXE));
            self.calls.lock().unwrap().push(args.to_vec());
            line("a line".into());
            Ok(self.code)
        }
    }

    impl Shell for Stub {
        fn run(&self, program: &str, args: &[OsString]) -> io::Result<ShellOut> {
            assert_eq!(program, EXE, "wsl.exe by absolute path only");
            let a: Vec<String> = args
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            self.calls.lock().unwrap().push_back(a.clone());
            for (want, code, out) in &self.answers {
                if *want == a {
                    return Ok(ShellOut {
                        code: *code,
                        stdout: out.clone(),
                    });
                }
            }
            Err(io::Error::new(io::ErrorKind::NotFound, "absent"))
        }
    }

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn ready() -> Stub {
        Stub::default()
            .on(&["--status"], 0, b"")
            .on(&["--list", "--quiet"], 0, &utf16("Ubuntu\r\nDebian\r\n"))
            .on(
                &["-e", "sh", "-c", PROBE_SCRIPT],
                0,
                b"Ubuntu\n/home/me\n/usr/bin/zsh\n",
            )
    }

    fn host(s: Stub) -> WslHost<Stub> {
        WslHost::new(s, Box::new(Cli::default()), EXE.into(), None)
    }

    fn validate(path: &str) -> Vec<&str> {
        vec![
            "-d",
            "Ubuntu",
            "--cd",
            path,
            "-e",
            "sh",
            "-c",
            VALIDATE_SCRIPT,
        ]
    }

    #[test]
    fn states_absent_no_distro_ready() {
        assert_eq!(
            host(Stub::default()).probe_distro().unwrap_err().kind,
            "wsl_absent"
        );
        let empty =
            Stub::default()
                .on(&["--status"], 0, b"")
                .on(&["--list", "--quiet"], 0, &utf16("\r\n"));
        assert_eq!(
            host(empty).probe_distro().unwrap_err().kind,
            "wsl_no_distro"
        );
        let h = host(ready());
        let d = h.probe_distro().unwrap();
        assert_eq!(
            d,
            Distro {
                name: "Ubuntu".into(),
                home: "/home/me".into(),
                shell: "/usr/bin/zsh".into()
            }
        );
        h.probe_distro().unwrap();
        assert_eq!(h.shell.calls().len(), 3, "probed once, then cached");
    }

    #[test]
    fn the_probe_refuses_bad_name_home_or_shell_and_defaults_the_shell() {
        for out in [
            &b"Ubu ntu\n/home/me\n/bin/bash\n"[..],
            b"Ubuntu\nrelative\n/bin/bash\n",
            b"Ubuntu\n/home/m e\n/bin/bash\n",
            b"Ubuntu\n/home/me\n/bin/ba\"sh\n",
            b"Ubuntu\n/home/\xc3\xa9\n/bin/bash\n",
            b"",
        ] {
            let s = Stub::default()
                .on(&["--status"], 0, b"")
                .on(&["--list", "--quiet"], 0, b"Ubuntu\n")
                .on(&["-e", "sh", "-c", PROBE_SCRIPT], 0, out);
            assert_eq!(
                host(s).probe_distro().unwrap_err().kind,
                "wsl_failed",
                "{:?}",
                String::from_utf8_lossy(out)
            );
        }
        let no_shell = Stub::default()
            .on(&["--status"], 0, b"")
            .on(&["--list", "--quiet"], 0, b"Ubuntu\n")
            .on(
                &["-e", "sh", "-c", PROBE_SCRIPT],
                0,
                b"Ubuntu\n/home/me\n\n",
            );
        assert_eq!(host(no_shell).probe_distro().unwrap().shell, "/bin/sh");
    }

    #[test]
    fn a_configured_distro_must_be_installed_and_is_passed_with_d() {
        let s = Stub::default()
            .on(&["--status"], 0, b"")
            .on(&["--list", "--quiet"], 0, &utf16("Ubuntu\r\nDebian\r\n"))
            .on(
                &["-d", "Debian", "-e", "sh", "-c", PROBE_SCRIPT],
                0,
                b"Debian\n/home/me\n/bin/bash\n",
            );
        let h = WslHost::new(
            s,
            Box::new(Cli::default()),
            EXE.into(),
            Some("debian".into()),
        );
        assert_eq!(h.probe_distro().unwrap().name, "Debian");
        let missing = WslHost::new(
            ready(),
            Box::new(Cli::default()),
            EXE.into(),
            Some("Arch".into()),
        );
        assert_eq!(
            missing.probe_distro().unwrap_err().kind,
            "wsl_distro_missing"
        );
        let bad = WslHost::new(
            ready(),
            Box::new(Cli::default()),
            EXE.into(),
            Some("a b".into()),
        );
        assert_eq!(bad.probe_distro().unwrap_err().kind, "wsl_distro_missing");
    }

    #[test]
    fn validation_exits_map_to_the_four_errors() {
        let s = ready()
            .on(&validate("/home/me/p"), 0, b"/home/me/p\n")
            .on(&validate("/home/me/noai"), 3, b"")
            .on(&validate("/home/me/norein"), 4, b"")
            .on(&validate("/home/me/file"), 2, b"")
            .on(&validate("/home/me/link"), 0, b"/home/me/a\"b\n")
            .on(&validate("/home/me/gone"), 5, b"");
        let h = host(s);
        assert_eq!(
            h.project("\\\\wsl.localhost\\Ubuntu\\home\\me\\p").unwrap(),
            PathBuf::from("/home/me/p")
        );
        let kind = |p: &str| h.project(p).unwrap_err().kind;
        assert_eq!(kind("/home/me/noai"), "no_ai");
        assert_eq!(kind("/home/me/norein"), "rein_missing");
        assert_eq!(kind("/home/me/file"), "not_a_directory");
        assert_eq!(
            kind("/home/me/link"),
            "unsafe_path",
            "the canonical path is re-checked"
        );
        assert_eq!(kind("/home/me/absent"), "not_a_directory");
        assert_eq!(
            kind("/home/me/gone"),
            "not_a_directory",
            "--cd fell back to /"
        );
        let calls = h.shell.calls().len();
        assert_eq!(kind("/"), "not_a_directory");
        assert_eq!(kind("\\\\wsl.localhost\\Ubuntu"), "not_a_directory");
        assert_eq!(
            h.shell.calls().len(),
            calls,
            "`/` is refused before any call"
        );
        assert_eq!(kind("C:\\Users\\me"), "not_wsl_path");
        assert_eq!(kind("\\\\wsl$\\Debian\\home"), "other_distro");
        assert_eq!(kind("/home/me/a\\b"), "unsafe_path");
    }

    #[test]
    fn the_pty_child_is_wsl_exe_with_the_constant_script_and_no_wslenv() {
        let h = host(ready());
        let spec = h
            .rein_start(Path::new("/home/me/p"), crate::tmux_server::Server::User)
            .unwrap();
        assert_eq!(spec.program, PathBuf::from(EXE));
        let argv: Vec<String> = spec
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            argv,
            [
                "-d",
                "Ubuntu",
                "--cd",
                "/home/me/p",
                "-e",
                "/usr/bin/zsh",
                "-lic",
                "tty; exec ~/.rein/bin/rein start"
            ]
        );
        assert!(spec.tty_line, "the tty line is taken out of the output");
        assert_eq!(spec.cwd, None);
        let cmd = spec.command();
        assert_eq!(cmd.get_env("WSLENV"), None);
        assert!(spec.env_set.is_empty(), "TERM comes from WSL, not from us");
    }

    /// E-57 (V8 3b): on the app's server the argv start script is the quote-free constant with
    /// the tty line, and nothing else changes in the child.
    #[test]
    fn the_app_server_start_is_the_quote_free_constant() {
        let h = host(ready());
        let spec = h
            .rein_start(Path::new("/home/me/p"), crate::tmux_server::Server::App)
            .unwrap();
        let argv: Vec<String> = spec
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            &argv[..7],
            [
                "-d",
                "Ubuntu",
                "--cd",
                "/home/me/p",
                "-e",
                "/usr/bin/zsh",
                "-lic"
            ]
        );
        assert_eq!(argv[7], crate::tmux_server::start_script_wsl());
        assert!(
            !argv[7].contains(['\'', '"']),
            "argv rule: no quote character"
        );
        assert!(spec.tty_line);
        assert_eq!(spec.command().get_env("WSLENV"), None);
    }

    #[test]
    fn the_overlay_runs_sh_ls_inside_wsl_with_only_the_validated_words() {
        let words: Vec<String> = [
            "/dev/pts/3",
            "hidden",
            "#1e1e2e",
            "#cdd6f4",
            "#a6adc8",
            "#6c7086",
            "#cba6f7",
        ]
        .map(String::from)
        .to_vec();
        let mut want = vec!["-d", "Ubuntu", "-e", "sh", "-ls", "--"];
        want.extend(words.iter().map(String::as_str));
        let h = host(ready().on(&want, 0, b""));
        assert_eq!(h.tmux_overlay(&words, &|| true), Ok(()));
        assert_eq!(h.shell.calls().last().unwrap(), &want);
        assert_eq!(
            h.shell.limits.lock().unwrap().last(),
            Some(&tmux_overlay::LIMIT)
        );
        let h = host(ready().on(&want, 4, b""));
        assert_eq!(
            h.tmux_overlay(&words, &|| true),
            Err("tmux is older than 3.2".into())
        );
    }

    #[test]
    fn wsl_exe_is_always_drive_absolute() {
        assert_eq!(
            wsl_exe(Some("D:\\Win".into())),
            "D:\\Win\\System32\\wsl.exe"
        );
        assert_eq!(
            wsl_exe(Some("C:\\Windows\\".into())),
            "C:\\Windows\\System32\\wsl.exe"
        );
        for bad in [None, Some(""), Some("x"), Some("\\\\srv\\w"), Some("C:rel")] {
            assert_eq!(
                wsl_exe(bad.map(String::from)),
                "C:\\Windows\\System32\\wsl.exe"
            );
        }
    }

    fn doc_ok() -> Vec<u8> {
        br#"{"schema":1,"version":"0.5.0","status":"ok","checks":[]}"#.to_vec()
    }

    #[test]
    fn doctor_runs_the_install_scope_inside_wsl_with_the_long_limit() {
        let s = ready().on(
            &["-d", "Ubuntu", "-e", "/usr/bin/zsh", "-lc", DOCTOR_SCRIPT],
            0,
            &doc_ok(),
        );
        let h = host(s);
        assert_eq!(h.doctor().status, "ok");
        assert!(
            h.shell
                .limits
                .lock()
                .unwrap()
                .contains(&crate::cli::DOCTOR_LIMIT)
        );
        assert_eq!(
            host(Stub::default()).doctor().status,
            "unknown",
            "WSL absent"
        );
        assert_eq!(
            h.distro_name().as_deref(),
            Some("Ubuntu"),
            "recorded with recent projects"
        );
        let absent = host(Stub::default()).doctor();
        assert_eq!(absent.setup.as_deref(), Some("wsl --install"));
        let empty =
            Stub::default()
                .on(&["--status"], 0, b"")
                .on(&["--list", "--quiet"], 0, &utf16("\r\n"));
        assert_eq!(
            host(empty).doctor().setup.as_deref(),
            Some("wsl --install -d Ubuntu")
        );
        assert!(
            host(Stub::default())
                .doctor()
                .reason
                .unwrap()
                .contains("wsl --install")
        );
    }

    #[test]
    fn cli_actions_run_the_constant_script_in_the_login_shell() {
        let cli = Cli::default();
        let calls = Arc::clone(&cli.calls);
        let h = WslHost::new(ready(), Box::new(cli), EXE.into(), None);
        let mut lines = Vec::new();
        h.run_cli(&crate::cli::CliAction::UpdateCheck, &mut |l| lines.push(l))
            .unwrap();
        let names = vec!["work".to_string(), "default".to_string()];
        h.run_cli(
            &crate::cli::CliAction::ProfilesApply {
                dry_run: true,
                names,
            },
            &mut |_| {},
        )
        .unwrap();
        let calls = calls.lock().unwrap();
        let want = |script: &str| -> Vec<String> {
            ["-d", "Ubuntu", "-e", "/usr/bin/zsh", "-lc", script]
                .iter()
                .map(|s| s.to_string())
                .collect()
        };
        assert_eq!(calls[0], want("exec rein-app update --check"));
        assert_eq!(
            calls[1],
            want("exec rein-app profiles apply --dry-run --profile work --profile default")
        );
        assert_eq!(lines, ["a line"]);
    }

    #[test]
    fn every_cli_action_has_its_exact_script_inside_wsl() {
        use crate::cli::CliAction;
        let cli = Cli::default();
        let calls = Arc::clone(&cli.calls);
        let h = WslHost::new(ready(), Box::new(cli), EXE.into(), None);
        let names = vec!["work".to_string()];
        for a in [
            CliAction::Update,
            CliAction::Rollback,
            CliAction::ProfilesList,
            CliAction::ProfilesApply {
                dry_run: false,
                names,
            },
        ] {
            h.run_cli(&a, &mut |_| {}).unwrap();
        }
        let scripts: Vec<String> = calls.lock().unwrap().iter().map(|c| c[5].clone()).collect();
        assert_eq!(
            scripts,
            [
                "exec rein-app update --yes",
                "exec rein-app rollback",
                "exec rein-app profiles list",
                "exec rein-app profiles apply --profile work",
            ]
        );
    }

    #[test]
    fn exit_127_says_the_linux_package_is_missing_and_bad_names_never_reach_a_script() {
        let cli = Cli {
            code: Some(127),
            ..Cli::default()
        };
        let h = WslHost::new(ready(), Box::new(cli), EXE.into(), None);
        let e = h
            .run_cli(&crate::cli::CliAction::Update, &mut |_| {})
            .unwrap_err();
        assert_eq!(e.kind, "cli_missing");
        assert!(e.message.contains("inside WSL"));
        for bad in ["a b", "x;id", "$(id)", "a\"b", "", "-x", "--enable"] {
            let a = crate::cli::CliAction::ProfilesApply {
                dry_run: false,
                names: vec![bad.into()],
            };
            assert_eq!(cli_script(&a).unwrap_err().kind, "bad_profile", "{bad:?}");
        }
    }

    #[test]
    fn dir_completion_lists_inside_wsl_with_the_guard_and_never_errors() {
        let list = b"proj/\nprod/\nprofile.txt\n.hidden/\n./\n../\n";
        let s = ready()
            .on(
                &[
                    "-d",
                    "Ubuntu",
                    "--cd",
                    "/home/me/",
                    "-e",
                    "sh",
                    "-c",
                    LIST_SCRIPT,
                ],
                0,
                list,
            )
            .on(
                &[
                    "-d",
                    "Ubuntu",
                    "--cd",
                    "/",
                    "-e",
                    "sh",
                    "-c",
                    LIST_ROOT_SCRIPT,
                ],
                0,
                b"home/\netc/\n",
            );
        let h = host(s);
        assert_eq!(
            h.complete_dir("/home/me/pro").unwrap(),
            ["/home/me/prod/", "/home/me/proj/"]
        );
        assert_eq!(
            h.complete_dir("/h").unwrap(),
            ["/home/"],
            "the root uses the unguarded listing"
        );
        assert!(
            h.complete_dir("/home/me/missing/x").unwrap().is_empty(),
            "a failed call: no lines"
        );
        assert!(h.complete_dir("relative").unwrap().is_empty());
        assert!(h.shell.limits.lock().unwrap().contains(&LIST_LIMIT));
        assert!(
            host(Stub::default())
                .complete_dir("/home/me/p")
                .unwrap()
                .is_empty(),
            "WSL absent"
        );
    }

    #[test]
    fn the_dialog_starts_in_the_linux_home_and_maps_back() {
        let h = host(ready());
        let start = h.dialog_start().unwrap();
        assert_eq!(start, PathBuf::from("\\\\wsl.localhost\\Ubuntu\\home\\me"));
        assert_eq!(
            wsl_path::to_linux(&start.to_string_lossy(), "Ubuntu").unwrap(),
            "/home/me"
        );
        assert_eq!(
            host(Stub::default()).dialog_start(),
            None,
            "WSL absent: the system default"
        );
    }

    #[test]
    fn scripts_are_constants_without_quotes() {
        for s in [
            PROBE_SCRIPT,
            VALIDATE_SCRIPT,
            START_SCRIPT,
            DOCTOR_SCRIPT,
            LIST_SCRIPT,
            LIST_ROOT_SCRIPT,
        ] {
            assert!(!s.contains(['"', '\'']), "{s}");
        }
    }
}
