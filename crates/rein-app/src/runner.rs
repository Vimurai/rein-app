//! `Runner` and `RuntimeCheck` (blueprint §Components 4-5, §Security trust boundary 4).
//!
//! The runner executes only contract §2.6 entry points — `rein install`, `rein doctor` —
//! from `$HOME/.rein/bin/rein`, with fixed argv, `CLAUDECODE` removed, `HOME` set to the
//! target home and `PATH` prefixed with `$HOME/.rein/bin`. Output is inherited (verbatim).
//! Every process goes through [`Process`] so tests stub it.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use semver::Version;

/// Official Claude Code installer page (printed, never bundled — Component 5).
pub const CLAUDE_INSTALL_URL: &str = "https://code.claude.com/docs/en/setup";

/// One process to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<&'static str>,
    /// Environment overrides; everything else is inherited.
    pub set: Vec<(&'static str, OsString)>,
    /// Variables removed from the inherited environment.
    pub remove: Vec<&'static str>,
}

/// Starts processes. `run` inherits stdio and returns the exit code (`None` if killed by a
/// signal); `capture` returns stdout, or `None` if the program is absent or failed.
pub trait Process {
    fn run(&self, inv: &Invocation) -> io::Result<Option<i32>>;
    /// Like `run`, but stdout is captured and returned; stderr stays on the terminal.
    /// `Ok(None)`: it did not finish (exit and stdout closed) within `limit`; the child was
    /// killed and reaped (D-032).
    fn output(&self, inv: &Invocation, limit: Duration) -> io::Result<Option<Output>>;
    fn capture(&self, program: &str, args: &[&str]) -> Option<String>;
}

/// How a captured run ended: its exit code (`None`: a signal) and its stdout.
pub type Output = (Option<i32>, Vec<u8>);

/// The most stdout [`Process::output`] keeps: 4 MiB, far above a doctor document (rein's
/// sample is under 10 KiB).
pub const OUTPUT_CAP: u64 = 4 * 1024 * 1024;

/// The real [`Process`].
pub struct SystemProcess;

impl Process for SystemProcess {
    fn run(&self, inv: &Invocation) -> io::Result<Option<i32>> {
        let mut cmd = Command::new(&inv.program);
        cmd.args(&inv.args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        for k in &inv.remove {
            cmd.env_remove(k);
        }
        for (k, v) in &inv.set {
            cmd.env(k, v);
        }
        Ok(cmd.status()?.code())
    }

    fn output(&self, inv: &Invocation, limit: Duration) -> io::Result<Option<Output>> {
        let deadline = Instant::now() + limit;
        let mut cmd = Command::new(&inv.program);
        cmd.args(&inv.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for k in &inv.remove {
            cmd.env_remove(k);
        }
        for (k, v) in &inv.set {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn()?;
        // stdout is read on its own thread, bounded: a document larger than the cap is not one
        // (the verdict then says 14), and the pipe is closed after the cap so a runaway writer
        // ends. The thread finishes when every holder of the pipe has closed it; a grandchild
        // that keeps it open is not waited for past the deadline (D-032).
        let (tx, rx) = mpsc::channel();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let read =
                    io::Read::read_to_end(&mut io::Read::take(out, OUTPUT_CAP + 1), &mut buf);
                let _ = tx.send(read.map(|_| buf));
            });
        } else {
            let _ = tx.send(Ok(Vec::new()));
        }
        let code = loop {
            if let Some(status) = child.try_wait()? {
                break status.code();
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let mut stdout = match rx.recv_timeout(remaining) {
            Ok(read) => read?,
            Err(_) => return Ok(None), // the child is reaped; a holder of its stdout is not waited for
        };
        if stdout.len() as u64 > OUTPUT_CAP {
            stdout.clear();
            stdout.extend_from_slice(b"(output over the size cap)");
        }
        Ok(Some((code, stdout)))
    }

    fn capture(&self, program: &str, args: &[&str]) -> Option<String> {
        let out = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// The rein entry points the app may run (contract §2.6). A closed set: no other argv exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinCommand {
    Install,
    Doctor,
    /// `rein doctor --json`: stdout passed through untouched (`rein-app doctor --json`).
    DoctorJson,
    /// `rein doctor --scope install --json`: the install verdict's document (D-029).
    DoctorInstallJson,
}

impl ReinCommand {
    fn argv(self) -> Vec<&'static str> {
        match self {
            ReinCommand::Install => vec!["install"],
            ReinCommand::Doctor => vec!["doctor"],
            ReinCommand::DoctorJson => vec!["doctor", "--json"],
            ReinCommand::DoctorInstallJson => vec!["doctor", "--scope", "install", "--json"],
        }
    }
}

/// Build the invocation of `rein <cmd>` from the installed tree under `home`.
pub fn rein_invocation(
    home: &Path,
    cmd: ReinCommand,
    inherited_path: Option<OsString>,
) -> Invocation {
    let bin = home.join(".rein").join("bin");
    let mut parts = vec![bin.clone()];
    if let Some(p) = &inherited_path {
        parts.extend(std::env::split_paths(p));
    }
    let path = std::env::join_paths(parts).unwrap_or_else(|_| bin.clone().into_os_string());
    Invocation {
        program: bin.join("rein"),
        args: cmd.argv(),
        set: vec![("HOME", home.as_os_str().to_owned()), ("PATH", path)],
        remove: vec!["CLAUDECODE"],
    }
}

/// How running rein ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    Ok,
    /// `cmd` exited non-zero (or by signal: `None`).
    Failed {
        cmd: ReinCommand,
        code: Option<i32>,
    },
    /// `cmd` could not be started.
    NotStarted {
        cmd: ReinCommand,
        reason: String,
    },
}

/// Run `cmds` in order from the installed tree; stops at the first failure.
pub fn run_rein(proc: &dyn Process, home: &Path, cmds: &[ReinCommand]) -> RunOutcome {
    for &cmd in cmds {
        let inv = rein_invocation(home, cmd, std::env::var_os("PATH"));
        match proc.run(&inv) {
            Ok(Some(0)) => {}
            Ok(code) => return RunOutcome::Failed { cmd, code },
            Err(e) => {
                return RunOutcome::NotStarted {
                    cmd,
                    reason: e.to_string(),
                };
            }
        }
    }
    RunOutcome::Ok
}

/// One runtime dependency's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    Yes(String),
    Missing,
    TooOld { found: Version, need: Version },
}

/// RuntimeCheck result (Data Model `RuntimeReport`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub rows: Vec<(&'static str, Found)>,
}

/// Tools rein cannot run without; the rest are reported only.
const REQUIRED: [&str; 2] = ["node", "git"];

impl RuntimeReport {
    /// Probe node (≥ `min_node`), git, sqlite3, tmux and Claude Code.
    pub fn probe(proc: &dyn Process, min_node: &Version) -> Self {
        let first_line = |s: String| s.lines().next().unwrap_or_default().trim().to_string();
        let mut rows = Vec::new();
        let node = match proc.capture("node", &["--version"]) {
            None => Found::Missing,
            Some(out) => {
                let line = first_line(out);
                match Version::parse(line.trim_start_matches('v')) {
                    Ok(v) if &v < min_node => Found::TooOld {
                        found: v,
                        need: min_node.clone(),
                    },
                    _ => Found::Yes(line),
                }
            }
        };
        rows.push(("node", node));
        for (name, flag) in [
            ("git", "--version"),
            ("sqlite3", "--version"),
            ("tmux", "-V"),
            ("claude", "--version"),
        ] {
            let found = proc
                .capture(name, &[flag])
                .map_or(Found::Missing, |o| Found::Yes(first_line(o)));
            rows.push((name, found));
        }
        Self { rows }
    }

    /// True when every required tool is present and new enough.
    pub fn ready(&self) -> bool {
        self.rows
            .iter()
            .filter(|(n, _)| REQUIRED.contains(n))
            .all(|(_, f)| matches!(f, Found::Yes(_)))
    }

    /// The printed table.
    pub fn table(&self) -> String {
        let mut s = String::new();
        for (name, found) in &self.rows {
            let need = if REQUIRED.contains(name) {
                "required"
            } else {
                "optional"
            };
            let state = match found {
                Found::Yes(v) => format!("ok       {v}"),
                Found::Missing => "MISSING".to_string(),
                Found::TooOld { found, need } => format!("TOO OLD  {found} (need ≥ {need})"),
            };
            s.push_str(&format!("  {name:<8} {need:<9} {state}\n"));
            if *name == "claude" && matches!(found, Found::Missing) {
                s.push_str(&format!(
                    "           install Claude Code: {CLAUDE_INSTALL_URL}\n"
                ));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Stub {
        tools: HashMap<&'static str, &'static str>,
        exits: RefCell<Vec<io::Result<Option<i32>>>>,
        ran: RefCell<Vec<Invocation>>,
    }

    impl Process for Stub {
        fn run(&self, inv: &Invocation) -> io::Result<Option<i32>> {
            self.ran.borrow_mut().push(inv.clone());
            self.exits.borrow_mut().remove(0)
        }
        fn output(&self, inv: &Invocation, _limit: Duration) -> io::Result<Option<Output>> {
            Ok(Some((self.run(inv)?, Vec::new())))
        }
        fn capture(&self, program: &str, _args: &[&str]) -> Option<String> {
            self.tools.get(program).map(|s| s.to_string())
        }
    }

    #[test]
    fn invocation_is_fixed_argv_from_the_installed_tree_with_a_clean_env() {
        let inv = rein_invocation(
            Path::new("/h"),
            ReinCommand::Doctor,
            Some(OsString::from("/usr/bin")),
        );
        assert_eq!(inv.program, Path::new("/h/.rein/bin/rein"));
        assert_eq!(inv.args, ["doctor"]);
        assert_eq!(inv.remove, ["CLAUDECODE"]);
        let path = &inv.set.iter().find(|(k, _)| *k == "PATH").unwrap().1;
        let parts: Vec<PathBuf> = std::env::split_paths(path).collect();
        assert_eq!(
            parts,
            [PathBuf::from("/h/.rein/bin"), PathBuf::from("/usr/bin")]
        );
        assert!(inv.set.contains(&("HOME", OsString::from("/h"))));
    }

    /// E-32 security review P2: captured stdout is bounded; a writer past the cap ends.
    #[cfg(unix)]
    #[test]
    fn output_is_capped_and_a_runaway_writer_ends() {
        let inv = Invocation {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c", "yes"],
            set: vec![],
            remove: vec![],
        };
        let (_, out) = SystemProcess
            .output(&inv, Duration::from_secs(10))
            .unwrap()
            .unwrap();
        assert!(out.len() as u64 <= OUTPUT_CAP, "{}", out.len());
        assert!(crate::verdict::parse(&out).is_err());
    }

    /// D-032: a run that does not end is killed at the limit; so is the wait for a grandchild
    /// that keeps stdout open after the child itself has exited; a quick run is unchanged.
    #[cfg(unix)]
    #[test]
    fn output_ends_at_the_limit_even_when_a_grandchild_holds_stdout() {
        let sh = |script: &'static str| Invocation {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c", script],
            set: vec![],
            remove: vec![],
        };
        let limit = Duration::from_millis(300);
        for script in ["sleep 30", "(sleep 30) & echo started"] {
            let t = Instant::now();
            assert_eq!(
                SystemProcess.output(&sh(script), limit).unwrap(),
                None,
                "{script}"
            );
            assert!(
                t.elapsed() < Duration::from_secs(5),
                "{script}: {:?}",
                t.elapsed()
            );
        }
        let quick = SystemProcess.output(&sh("echo ok"), limit).unwrap();
        assert_eq!(quick, Some((Some(0), b"ok\n".to_vec())));
    }

    #[test]
    fn run_rein_runs_in_order_and_stops_at_the_first_failure() {
        let install_then_doctor =
            |p: &Stub, h: &Path| run_rein(p, h, &[ReinCommand::Install, ReinCommand::Doctor]);
        let ok = Stub {
            exits: RefCell::new(vec![Ok(Some(0)), Ok(Some(0))]),
            ..Default::default()
        };
        assert_eq!(install_then_doctor(&ok, Path::new("/h")), RunOutcome::Ok);
        let args: Vec<_> = ok.ran.borrow().iter().map(|i| i.args.clone()).collect();
        assert_eq!(args, [vec!["install"], vec!["doctor"]]);

        let failing = Stub {
            exits: RefCell::new(vec![Ok(Some(5))]),
            ..Default::default()
        };
        assert_eq!(
            install_then_doctor(&failing, Path::new("/h")),
            RunOutcome::Failed {
                cmd: ReinCommand::Install,
                code: Some(5)
            }
        );
        assert_eq!(
            failing.ran.borrow().len(),
            1,
            "doctor not run after a failed install"
        );
    }

    #[test]
    fn runtime_report_requires_node_at_min_version_and_git() {
        let need = Version::new(22, 5, 0);
        let all = Stub {
            tools: HashMap::from([
                ("node", "v22.11.0\n"),
                ("git", "git version 2.43.0\n"),
                ("sqlite3", "3.45.1 2024-01-30\n"),
                ("tmux", "tmux 3.4\n"),
                ("claude", "2.1.319 (Claude Code)\n"),
            ]),
            ..Default::default()
        };
        assert!(RuntimeReport::probe(&all, &need).ready());

        let old = Stub {
            tools: HashMap::from([("node", "v20.1.0"), ("git", "git version 2.43.0")]),
            ..Default::default()
        };
        let r = RuntimeReport::probe(&old, &need);
        assert!(!r.ready());
        assert!(
            r.table().contains("TOO OLD  20.1.0 (need ≥ 22.5.0)"),
            "{}",
            r.table()
        );
        assert!(
            r.table().contains(CLAUDE_INSTALL_URL),
            "missing claude names the installer"
        );

        let no_git = Stub {
            tools: HashMap::from([("node", "v24.0.0")]),
            ..Default::default()
        };
        assert!(!RuntimeReport::probe(&no_git, &need).ready());
    }
}
