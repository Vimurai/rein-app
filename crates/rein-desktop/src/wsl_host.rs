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
use crate::wsl_path::{self, PathError};

/// Learns the distro's name, the Linux home and the login shell (W2). `%s\\n` is `%s\n` once
/// sh has removed one backslash: the script holds no quote of any kind.
pub const PROBE_SCRIPT: &str = "printf %s\\\\n $WSL_DISTRO_NAME $HOME $SHELL";
/// Run with `--cd <project>`: exit 5 still in `/` (a failed `--cd` does not fail: `wsl.exe`
/// runs the command in `/` instead — D-038 addendum 1), 3 no `.ai/`, 4 rein not installed,
/// else `pwd -P`.
pub const VALIDATE_SCRIPT: &str = "case $PWD in /) exit 5;; esac; [ -d .ai ] || exit 3; [ -x ~/.rein/bin/rein ] || exit 4; pwd -P";
/// The pty child's script, run by the user's login + interactive shell (W2).
pub const START_SCRIPT: &str = "exec ~/.rein/bin/rein start";
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

pub struct WslHost<S: Shell + Send + Sync> {
    shell: S,
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
            wsl_exe(std::env::var("SystemRoot").ok()),
            configured,
        )
    }
}

impl<S: Shell + Send + Sync> WslHost<S> {
    pub fn new(shell: S, wsl_exe: String, configured: Option<String>) -> Self {
        Self {
            shell,
            wsl_exe,
            configured,
            probed: Mutex::new(None),
        }
    }

    fn run(&self, args: &[&str]) -> io::Result<ShellOut> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        self.shell.run(&self.wsl_exe, &args)
    }

    /// The window's distro, probed once (W2). Refusals name the command that fixes them.
    pub fn distro(&self) -> Result<Distro, IpcError> {
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

impl<S: Shell + Send + Sync> Host for WslHost<S> {
    fn project(&self, input: &str) -> Result<PathBuf, IpcError> {
        let d = self.distro()?;
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
    fn complete_dir(&self, _prefix: &str) -> Result<Vec<String>, IpcError> {
        Ok(Vec::new())
    }

    /// Health inside WSL is E-45 (§Windows window W4).
    fn doctor(&self) -> crate::cli::HealthView {
        crate::cli::unknown("checking health inside WSL arrives with E-45")
    }

    /// Release and profile actions inside WSL are E-45 (W4).
    fn run_cli(
        &self,
        _action: &crate::cli::CliAction,
        _line: &mut dyn FnMut(String),
    ) -> Result<Option<i32>, IpcError> {
        Err(err(
            "not_yet",
            "update, rollback and profiles inside WSL arrive with E-45",
        ))
    }

    fn rein_start(&self, project: &Path) -> Result<ChildSpec, IpcError> {
        let d = self.distro()?;
        let project = project.to_string_lossy();
        wsl_path::check_linux(&project).map_err(|e| err("unsafe_path", e.to_string()))?;
        Ok(ChildSpec {
            program: PathBuf::from(&self.wsl_exe),
            args: [
                "-d",
                &d.name,
                "--cd",
                &project,
                "-e",
                &d.shell,
                "-lic",
                START_SCRIPT,
            ]
            .iter()
            .map(OsString::from)
            .collect(),
            cwd: None,
            env_remove: ENV_REMOVED,
            env_set: &[],
        })
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
        let mut cmd = Command::new(program);
        cmd.args(args).stderr(Stdio::null());
        for k in ENV_REMOVED {
            cmd.env_remove(k);
        }
        hide_console(&mut cmd);
        match rein_platform::process::output(&mut cmd, CALL_LIMIT, CALL_CAP)? {
            Some((code, stdout)) => Ok(ShellOut { code, stdout }),
            None => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "wsl.exe did not finish in time",
            )),
        }
    }
}

#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    const EXE: &str = "C:\\Windows\\System32\\wsl.exe";

    /// Answers by exact argv; records every call.
    #[derive(Default)]
    struct Stub {
        answers: Vec<(Vec<String>, Option<i32>, Vec<u8>)>,
        calls: Mutex<VecDeque<Vec<String>>>,
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
        WslHost::new(s, EXE.into(), None)
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
            host(Stub::default()).distro().unwrap_err().kind,
            "wsl_absent"
        );
        let empty =
            Stub::default()
                .on(&["--status"], 0, b"")
                .on(&["--list", "--quiet"], 0, &utf16("\r\n"));
        assert_eq!(host(empty).distro().unwrap_err().kind, "wsl_no_distro");
        let h = host(ready());
        let d = h.distro().unwrap();
        assert_eq!(
            d,
            Distro {
                name: "Ubuntu".into(),
                home: "/home/me".into(),
                shell: "/usr/bin/zsh".into()
            }
        );
        h.distro().unwrap();
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
                host(s).distro().unwrap_err().kind,
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
        assert_eq!(host(no_shell).distro().unwrap().shell, "/bin/sh");
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
        let h = WslHost::new(s, EXE.into(), Some("debian".into()));
        assert_eq!(h.distro().unwrap().name, "Debian");
        let missing = WslHost::new(ready(), EXE.into(), Some("Arch".into()));
        assert_eq!(missing.distro().unwrap_err().kind, "wsl_distro_missing");
        let bad = WslHost::new(ready(), EXE.into(), Some("a b".into()));
        assert_eq!(bad.distro().unwrap_err().kind, "wsl_distro_missing");
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
        let spec = h.rein_start(Path::new("/home/me/p")).unwrap();
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
                "exec ~/.rein/bin/rein start"
            ]
        );
        assert_eq!(spec.cwd, None);
        let cmd = spec.command();
        assert_eq!(cmd.get_env("WSLENV"), None);
        assert!(spec.env_set.is_empty(), "TERM comes from WSL, not from us");
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

    #[test]
    fn scripts_are_constants_without_quotes() {
        for s in [PROBE_SCRIPT, VALIDATE_SCRIPT, START_SCRIPT] {
            assert!(!s.contains(['"', '\'']), "{s}");
        }
    }
}
