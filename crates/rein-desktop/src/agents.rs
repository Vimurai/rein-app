//! The agent-exit bar's core (desktop.md §Look and feel v2 V9 components 4–5, D-057, E-62):
//! which role panes of a tab still run their agent, and closing the project.
//!
//! `rein start --status` has no machine-readable form (E-62's read of rein 0.5.0), so the
//! status is V9's fallback: the constant script [`SCRIPT`] on stdin, which lists the panes of
//! the window the tab's own tmux client shows with a constant format, on the tab's own server.
//! Closing is rein's own `rein start --kill --yes` in the project, on the tab's server — the
//! same path the tab's `rein start` took. Nothing from the web side becomes a word of either.
//!
//! An agent counts as ended only after it was seen running in this tab: the role panes show a
//! shell while `rein start` builds the layout, and that must never read as "exited".

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::ipc::{Desktop, IpcError, err};

/// The status script: `sh -s -- <client tty> <app|user>`.
pub const SCRIPT: &str = include_str!("tmux_agents.sh");
/// One status run (V9 "Execution constraints": a short process).
pub const STATUS_LIMIT: Duration = Duration::from_secs(5);
/// The most a status run's output may be.
pub const STATUS_CAP: u64 = 16 * 1024;
/// At most one status process per this long (V9: one short process per 5 s).
pub const POLL_EVERY: Duration = Duration::from_secs(5);
/// `rein start --kill --yes`.
pub const KILL_LIMIT: Duration = Duration::from_secs(30);
pub const KILL_CAP: u64 = 64 * 1024;
/// The most role panes one answer carries.
pub const MAX_ROLES: usize = 16;

/// The app's fixed sentences; the window is told only these, never tmux's or rein's words.
pub const NO_TMUX: &str = "tmux was not found";
pub const NO_CLIENT: &str = "the tab's tmux client was not found";
pub const NOT_ASKED: &str = "the agents could not be asked";
pub const KILL_FAILED: &str = "rein start --kill --yes did not succeed";
pub const KILL_TIMED_OUT: &str = "rein start --kill --yes did not finish within 30 seconds";
pub const KILL_NOT_STARTED: &str = "rein start --kill --yes could not start";
/// The app's start script refuses a tmux home that is not as the setup left it (V8, 126).
pub const KILL_HOME: &str = "the app's tmux files are not as the app left them";

/// Programs a role pane shows when no agent runs in it: its shell.
const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "mksh", "tcsh", "csh", "ash", "nu",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    Running,
    Ended,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Agent {
    pub role: String,
    pub state: AgentState,
}

/// `project_agents`' answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Reply {
    Agents { agents: Vec<Agent> },
    Unavailable { unavailable: &'static str },
}

/// `project_close`'s answer: closed, or the roles still running that the operator has not
/// confirmed (an empty list: whether one runs is not known).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Close {
    Closed { closed: bool },
    Confirm { confirm: Vec<String> },
}

/// A role as rein names one: lower case, a letter first, at most 32.
pub fn role_ok(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-' || *c == b'_')
}

fn is_shell(cmd: &str) -> bool {
    SHELLS.contains(&cmd.strip_prefix('-').unwrap_or(cmd))
}

/// One pane line: `(role, alive)` — `None` for a pane without a role or a line not of the form.
fn pane(line: &str) -> Option<(&str, bool)> {
    let mut f = line.rsplitn(3, '|');
    let dead = f.next()?;
    let cmd = f.next()?;
    let role = f.next()?;
    if !role_ok(role) || !matches!(dead, "0" | "1") || cmd.len() > 256 {
        return None;
    }
    Some((role, dead == "0" && !cmd.is_empty() && !is_shell(cmd)))
}

/// The roles' states from the script's output. `seen` holds the roles seen running in this
/// tab before; it gains the ones running now. A role seen before and absent now has ended
/// (its pane is gone). Sorted by role, at most [`MAX_ROLES`].
pub fn states(out: &[u8], seen: &mut BTreeSet<String>) -> Vec<Agent> {
    let text = String::from_utf8_lossy(out);
    let mut now: HashMap<String, bool> = HashMap::new();
    for (role, alive) in text.lines().filter_map(pane) {
        *now.entry(role.to_string()).or_default() |= alive;
    }
    for (role, alive) in &now {
        if *alive && seen.len() < MAX_ROLES {
            seen.insert(role.clone());
        }
    }
    let roles: BTreeSet<&String> = now.keys().chain(seen.iter()).collect();
    roles
        .into_iter()
        .take(MAX_ROLES)
        .map(|role| Agent {
            role: role.clone(),
            state: match now.get(role) {
                Some(true) => AgentState::Running,
                _ if seen.contains(role) => AgentState::Ended,
                _ => AgentState::Unknown,
            },
        })
        .collect()
}

/// The status script's run as the window is told it.
pub fn status_outcome(
    run: std::io::Result<Option<rein_platform::process::Output>>,
) -> Result<Vec<u8>, &'static str> {
    match run {
        Ok(Some((Some(0), out))) => Ok(out),
        Ok(Some((Some(3), _))) => Err(NO_TMUX),
        Ok(Some((Some(5), _))) => Err(NO_CLIENT),
        _ => Err(NOT_ASKED),
    }
}

/// The kill's run as the window is told it.
pub fn kill_outcome(
    run: std::io::Result<Option<rein_platform::process::Output>>,
) -> Result<(), &'static str> {
    match run {
        Ok(Some((Some(0), _))) => Ok(()),
        Ok(Some((Some(126), _))) => Err(KILL_HOME),
        Ok(Some(_)) => Err(KILL_FAILED),
        Ok(None) => Err(KILL_TIMED_OUT),
        Err(_) => Err(KILL_NOT_STARTED),
    }
}

/// What the window remembers per tab: the roles seen running, the last answer, and when the
/// last status process ran (for the 5 s gate, window-wide).
#[derive(Default)]
pub struct AgentWatch(Mutex<Watch>);

#[derive(Default)]
struct Watch {
    seen: HashMap<u32, BTreeSet<String>>,
    last: HashMap<u32, Vec<Agent>>,
    ran_at: Option<Instant>,
}

impl AgentWatch {
    fn lock(&self) -> std::sync::MutexGuard<'_, Watch> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The tab closed: nothing of it is kept.
    pub fn forget(&self, tab: u32) {
        let mut w = self.lock();
        w.seen.remove(&tab);
        w.last.remove(&tab);
    }

    /// Claim the one status run per [`POLL_EVERY`]; `Err` carries the last answer instead.
    fn claim(&self, tab: u32, now: Instant, force: bool) -> Result<(), Vec<Agent>> {
        let mut w = self.lock();
        if !force && w.ran_at.is_some_and(|t| now.duration_since(t) < POLL_EVERY) {
            return Err(w.last.get(&tab).cloned().unwrap_or_default());
        }
        w.ran_at = Some(now);
        Ok(())
    }

    fn record(&self, tab: u32, out: &[u8]) -> Vec<Agent> {
        let mut w = self.lock();
        let mut seen = w.seen.remove(&tab).unwrap_or_default();
        let agents = states(out, &mut seen);
        w.seen.insert(tab, seen);
        w.last.insert(tab, agents.clone());
        agents
    }
}

/// The status of `tab`'s agents now. `force`: a click (Close project) — past the 5 s gate.
pub fn agents_of(d: &Desktop, tab: u32, force: bool) -> Reply {
    let Some((pty, server, _)) = d.tab_target(tab) else {
        return Reply::Agents { agents: vec![] };
    };
    // Stopped the moment the tab's process ends (V9).
    if !d.host.alive(pty) {
        return Reply::Agents { agents: vec![] };
    }
    let tty = match d.host.tty(pty) {
        Ok(Some(t)) if crate::tmux_overlay::tty_ok(&t) => t,
        _ => {
            return Reply::Unavailable {
                unavailable: NO_CLIENT,
            };
        }
    };
    if let Err(last) = d.agents.claim(tab, Instant::now(), force) {
        return Reply::Agents { agents: last };
    }
    let args = [tty, server.word().to_string()];
    match d.platform.tmux_agents(&args) {
        // A tab that closed meanwhile keeps nothing.
        Ok(out) if d.tab_target(tab).is_some() => Reply::Agents {
            agents: d.agents.record(tab, &out),
        },
        Ok(_) => Reply::Agents { agents: vec![] },
        Err(why) => Reply::Unavailable { unavailable: why },
    }
}

/// Close `tab`'s project: `rein start --kill --yes` on the tab's server. With an agent still
/// running (or not known) and `confirm_running` false, nothing runs: the roles come back for
/// the confirm dialog (V9: default No).
pub fn close(d: &Desktop, tab: u32, confirm_running: bool) -> Result<Close, IpcError> {
    let (_, server, project) = d
        .tab_target(tab)
        .ok_or_else(|| err("unknown_tab", "the tab is not open"))?;
    if !confirm_running {
        match agents_of(d, tab, true) {
            Reply::Agents { agents } => {
                let running: Vec<String> = agents
                    .into_iter()
                    .filter(|a| a.state == AgentState::Running)
                    .map(|a| a.role)
                    .collect();
                if !running.is_empty() {
                    return Ok(Close::Confirm { confirm: running });
                }
            }
            Reply::Unavailable { .. } => return Ok(Close::Confirm { confirm: vec![] }),
        }
    }
    d.platform
        .rein_kill(&project, server)
        .map_err(|why| err("close_failed", why))?;
    Ok(Close::Closed { closed: true })
}

/// `project_agents(tab)`: per role `running | ended | unknown`, or why not known. Off the UI
/// thread; at most one status process per 5 s across the window.
#[tauri::command(async)]
pub fn project_agents(state: State<'_, Desktop>, tab: u32) -> Reply {
    agents_of(&state, tab, false)
}

/// `project_close(tab, confirm_running)`: `rein start --kill --yes` for the tab's project on
/// its tmux server. The web side closes the tab on `{closed: true}`.
#[tauri::command(async)]
pub fn project_close(
    state: State<'_, Desktop>,
    tab: u32,
    confirm_running: bool,
) -> Result<Close, IpcError> {
    close(&state, tab, confirm_running)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(out: &str, seen: &mut BTreeSet<String>) -> Vec<(String, AgentState)> {
        states(out.as_bytes(), seen)
            .into_iter()
            .map(|a| (a.role, a.state))
            .collect()
    }

    #[test]
    fn an_agent_has_ended_only_after_it_was_seen_running() {
        use AgentState::*;
        let mut seen = BTreeSet::new();
        // `rein start` building the layout: shells only — never "ended".
        assert_eq!(
            run("architect|zsh|0\nengineer|bash|0\n", &mut seen),
            [("architect".into(), Unknown), ("engineer".into(), Unknown)]
        );
        assert_eq!(
            run("architect|claude|0\nengineer|node|0\n|zsh|0\n", &mut seen),
            [("architect".into(), Running), ("engineer".into(), Running)]
        );
        // One exited back to its shell (a login shell's `-zsh` too): one ended.
        assert_eq!(
            run("architect|claude|0\nengineer|-zsh|0\n", &mut seen),
            [("architect".into(), Running), ("engineer".into(), Ended)]
        );
        // All ended: a dead pane, and a pane that is gone.
        assert_eq!(
            run("engineer|claude|1\n", &mut seen),
            [("architect".into(), Ended), ("engineer".into(), Ended)]
        );
        // A restarted agent runs again.
        assert_eq!(
            run("architect|claude|0\nengineer|zsh|0\n", &mut seen),
            [("architect".into(), Running), ("engineer".into(), Ended)]
        );
    }

    #[test]
    fn only_rein_roles_from_lines_of_the_form_count() {
        let mut seen = BTreeSet::new();
        let hostile = "Architect|claude|0\nx.y|claude|0\nrole with space|claude|0\n\
                       a|b\nengineer|claude|maybe\n|claude|0\nx\u{202e}|claude|0\n\
                       tester|ev|il|claude|0\n";
        assert!(states(hostile.as_bytes(), &mut seen).is_empty(), "{seen:?}");
        // At most MAX_ROLES, however many panes.
        let many: String = (0..100).map(|i| format!("r{i}|claude|0\n")).collect();
        assert_eq!(states(many.as_bytes(), &mut seen).len(), MAX_ROLES);
        assert!(seen.len() <= MAX_ROLES);
    }

    #[test]
    fn every_outcome_is_a_fixed_sentence() {
        let ok = |code, out: &[u8]| Ok(Some((Some(code), out.to_vec())));
        assert_eq!(status_outcome(ok(0, b"x")), Ok(b"x".to_vec()));
        assert_eq!(status_outcome(ok(3, b"")), Err(NO_TMUX));
        assert_eq!(status_outcome(ok(5, b"")), Err(NO_CLIENT));
        assert_eq!(status_outcome(ok(2, b"")), Err(NOT_ASKED));
        assert_eq!(status_outcome(Ok(None)), Err(NOT_ASKED));
        assert_eq!(kill_outcome(ok(0, b"killed")), Ok(()));
        assert_eq!(kill_outcome(ok(126, b"")), Err(KILL_HOME));
        assert_eq!(kill_outcome(ok(2, b"refusing")), Err(KILL_FAILED));
        assert_eq!(kill_outcome(Ok(None)), Err(KILL_TIMED_OUT));
        assert_eq!(
            kill_outcome(Err(std::io::Error::other("x"))),
            Err(KILL_NOT_STARTED)
        );
    }

    #[test]
    fn the_status_script_only_reads() {
        // V9: the status run never changes tmux — no setting, killing or starting command.
        let code: String = SCRIPT
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        for bad in [
            "set-option",
            "set -",
            "kill-",
            "new-session",
            "start-server",
            "send-keys",
            "run-shell",
            "source",
            "eval",
            "`",
        ] {
            assert!(!code.contains(bad), "{bad:?}");
        }
        let calls: Vec<&str> = code
            .split("tmux ")
            .skip(1)
            .map(|s| s.split_whitespace().next().unwrap_or(""))
            // `command -v tmux >/dev/null` looks tmux up; it is not a call.
            .filter(|c| !c.starts_with('>'))
            .collect();
        for c in calls {
            assert!(
                ["-L", "list-clients", "list-panes", "-V", "()"].contains(&c),
                "tmux {c}"
            );
        }
    }

    #[cfg(unix)]
    type Answers = std::sync::Arc<Mutex<Vec<Result<Vec<u8>, &'static str>>>>;

    /// A host that answers the status with the next of `outs` and records every call.
    #[cfg(unix)]
    struct AgentsHost {
        outs: Answers,
        calls: std::sync::Arc<Mutex<Vec<String>>>,
    }

    #[cfg(unix)]
    impl crate::host::Host for AgentsHost {
        fn project_dir(
            &self,
            _: &str,
        ) -> Result<(std::path::PathBuf, crate::host::Found), IpcError> {
            unreachable!()
        }
        fn rein_setup(
            &self,
            _: &std::path::Path,
            _: crate::tmux_server::Server,
        ) -> Result<crate::pty::ChildSpec, IpcError> {
            unreachable!()
        }
        fn rein_command(
            &self,
            _: &std::path::Path,
            _: crate::project_setup::ReinRun,
        ) -> Result<std::process::Command, IpcError> {
            unreachable!()
        }
        fn rein_start(
            &self,
            _: &std::path::Path,
            _: crate::tmux_server::Server,
        ) -> Result<crate::pty::ChildSpec, IpcError> {
            unreachable!()
        }
        fn read_project_file(
            &self,
            _: &std::path::Path,
            _: crate::project_file::ProjectFile,
            _: bool,
        ) -> Result<crate::project_file::FileBytes, crate::project_file::Unavailable> {
            unreachable!()
        }
        fn tmux_setup(&self) -> Result<(), &'static str> {
            unreachable!()
        }
        fn complete_dir(&self, _: &str) -> Result<Vec<String>, IpcError> {
            unreachable!()
        }
        fn doctor(&self) -> crate::cli::HealthView {
            unreachable!()
        }
        fn run_cli(
            &self,
            _: &crate::cli::CliAction,
            _: &mut dyn FnMut(String),
        ) -> Result<Option<i32>, IpcError> {
            unreachable!()
        }
        fn tmux_overlay(&self, _: &[String], _: &dyn Fn() -> bool) -> Result<(), String> {
            unreachable!()
        }
        fn tmux_agents(&self, args: &[String]) -> Result<Vec<u8>, &'static str> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("status {}", args.join(" ")));
            self.outs.lock().unwrap().remove(0)
        }
        fn rein_kill(
            &self,
            project: &std::path::Path,
            server: crate::tmux_server::Server,
        ) -> Result<(), &'static str> {
            self.calls.lock().unwrap().push(format!(
                "kill {} {}",
                project.display(),
                server.word()
            ));
            Ok(())
        }
        fn dialog_start(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_poll_is_gated_and_close_asks_while_an_agent_runs() {
        use crate::pty::{ChildSpec, Events, PtyId, Size};
        use std::sync::Arc;
        struct Quiet;
        impl Events for Quiet {
            fn output(&self, _: PtyId, _: Vec<u8>) {}
            fn exit(&self, _: PtyId, _: u32) {}
        }
        let outs = Arc::new(Mutex::new(vec![
            Ok(b"architect|claude|0\nengineer|claude|0\n".to_vec()),
            Ok(b"architect|claude|0\nengineer|zsh|0\n".to_vec()),
            Ok(b"architect|zsh|0\nengineer|zsh|0\n".to_vec()),
            Err(NO_CLIENT),
        ]));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let d = Desktop::new(Box::new(AgentsHost {
            outs: Arc::clone(&outs),
            calls: Arc::clone(&calls),
        }));
        let p = tempfile::tempdir().unwrap();
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 30".into()],
            cwd: Some(p.path().into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        let tab = d
            .open_spec(
                &spec,
                p.path(),
                Size { cols: 80, rows: 24 },
                Arc::new(Quiet),
            )
            .unwrap();
        let running = |r: Reply| match r {
            Reply::Agents { agents } => agents
                .into_iter()
                .map(|a| (a.role, a.state))
                .collect::<Vec<_>>(),
            other => panic!("{other:?}"),
        };
        let first = running(agents_of(&d, tab.id, false));
        assert!(first.iter().all(|(_, s)| *s == AgentState::Running));
        // Within 5 s: no second process, the last answer again.
        assert_eq!(running(agents_of(&d, tab.id, false)), first);
        assert_eq!(calls.lock().unwrap().len(), 1);
        let tty = d.host.tty(tab.pty).unwrap().unwrap();
        assert_eq!(calls.lock().unwrap()[0], format!("status {tty} app"));
        // Close with the Architect still running: asks, kills nothing.
        assert_eq!(
            close(&d, tab.id, false).unwrap(),
            Close::Confirm {
                confirm: vec!["architect".into()]
            }
        );
        // All ended now: closes without asking — on the tab's own project and server.
        assert_eq!(
            close(&d, tab.id, false).unwrap(),
            Close::Closed { closed: true }
        );
        let kill = format!("kill {} app", p.path().display());
        assert_eq!(calls.lock().unwrap().last().unwrap(), &kill);
        // Not known (no client): asks, with no role named; confirmed: closes.
        assert_eq!(
            close(&d, tab.id, false).unwrap(),
            Close::Confirm { confirm: vec![] }
        );
        assert_eq!(
            close(&d, tab.id, true).unwrap(),
            Close::Closed { closed: true }
        );
        // A closed tab: nothing runs, nothing is kept.
        d.detach(tab.id);
        let n = calls.lock().unwrap().len();
        assert_eq!(running(agents_of(&d, tab.id, true)), []);
        assert_eq!(close(&d, tab.id, true).unwrap_err().kind, "unknown_tab");
        assert_eq!(calls.lock().unwrap().len(), n);
        assert!(d.agents.lock().seen.is_empty() && d.agents.lock().last.is_empty());
    }

    #[test]
    fn the_answers_shapes() {
        let a = serde_json::to_value(Reply::Agents {
            agents: vec![Agent {
                role: "engineer".into(),
                state: AgentState::Ended,
            }],
        })
        .unwrap();
        assert_eq!(
            a,
            serde_json::json!({"agents": [{"role": "engineer", "state": "ended"}]})
        );
        assert_eq!(
            serde_json::to_value(Reply::Unavailable {
                unavailable: NO_CLIENT
            })
            .unwrap(),
            serde_json::json!({"unavailable": NO_CLIENT})
        );
        assert_eq!(
            serde_json::to_value(Close::Confirm {
                confirm: vec!["architect".into()]
            })
            .unwrap(),
            serde_json::json!({"confirm": ["architect"]})
        );
        assert_eq!(
            serde_json::to_value(Close::Closed { closed: true }).unwrap(),
            serde_json::json!({"closed": true})
        );
    }
}
