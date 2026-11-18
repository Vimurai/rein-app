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

/// The status script: `sh -s -- <client tty> <app|user> [<client tty> <app|user>]...`.
pub const SCRIPT: &str = include_str!("tmux_agents.sh");
/// One status run (V9 "Execution constraints": a short process).
pub const STATUS_LIMIT: Duration = Duration::from_secs(5);
/// The most a status run's output may be (16 tabs of panes).
pub const STATUS_CAP: u64 = 64 * 1024;
/// At most one status process per this long (V9: one short process per 5 s).
pub const POLL_EVERY: Duration = Duration::from_secs(5);
/// The most tabs one status run asks about (the script's own limit: 16 pairs).
pub const MAX_PAIRS: usize = 16;
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

/// One tab's part of a status answer: its window's last output and its pane lines; `None`
/// for a tab whose client was not found (or whose part is missing or malformed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub activity: u64,
    pub panes: String,
}

/// A number the script printed: digits only, at most 12.
fn number(s: &str) -> Option<u64> {
    ((1..=12).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// The script's answer for `pairs` tabs: the clock of the machine tmux runs on, and each
/// tab's block in the order asked. Lines out of place are ignored, never guessed at.
pub fn blocks(out: &[u8], pairs: usize) -> (Option<u64>, Vec<Option<Block>>) {
    let text = String::from_utf8_lossy(out);
    let mut clock = None;
    let mut found: Vec<Option<Block>> = vec![None; pairs];
    // The tab whose lines follow, once its header and its activity line were read.
    let mut at: Option<usize> = None;
    let mut next = 0usize;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("clock ") {
            if next == 0 && clock.is_none() {
                clock = number(rest);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("tab ") {
            at = None;
            let mut w = rest.split(' ');
            let (Some(i), Some(code), None) = (w.next(), w.next(), w.next()) else {
                next = pairs; // a malformed header: nothing after it is trusted
                continue;
            };
            if number(i) != Some(next as u64) || next >= pairs {
                next = pairs;
                continue;
            }
            if code == "0" {
                at = Some(next);
            }
            next += 1;
            continue;
        }
        let Some(i) = at else { continue };
        if let Some(rest) = line.strip_prefix("activity ") {
            match (number(rest), &found[i]) {
                (Some(a), None) => {
                    found[i] = Some(Block {
                        activity: a,
                        panes: String::new(),
                    });
                }
                _ => at = None,
            }
            continue;
        }
        if let Some(b) = found[i].as_mut()
            && let Some(pane) = line.strip_prefix("pane ")
            && b.panes.len() + pane.len() < STATUS_CAP as usize
        {
            b.panes.push_str(pane);
            b.panes.push('\n');
        }
    }
    (clock, found)
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
    /// Per tab: its window's output over time (status.md component 1, E-76).
    activity: HashMap<u32, crate::activity::Tracker>,
    /// Per tab: whether the last status run found its tmux client, and whether any did.
    client: HashMap<u32, (bool, bool)>,
    /// The clock of the machine tmux runs on, as last read.
    clock: Option<u64>,
}

/// What the status reads about one tab from the last status run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub agents: Vec<Agent>,
    /// The tab's tmux client was found by the last run / by any run.
    pub client_now: bool,
    pub client_ever: bool,
    pub activity: crate::activity::Activity,
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
        w.activity.remove(&tab);
        w.client.remove(&tab);
    }

    /// What the last status runs saw of `tab`.
    pub fn seen(&self, tab: u32) -> Seen {
        let w = self.lock();
        let (client_now, client_ever) = w.client.get(&tab).copied().unwrap_or_default();
        let activity = match (w.activity.get(&tab), w.clock) {
            (Some(t), Some(c)) => t.at(c),
            _ => crate::activity::Tracker::default().at(0),
        };
        Seen {
            agents: w.last.get(&tab).cloned().unwrap_or_default(),
            client_now,
            client_ever,
            activity,
        }
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

    /// One tab's part of a status run: its agents, and its activity on `clock`.
    fn record(&self, tab: u32, block: Option<&Block>, clock: Option<u64>) -> Vec<Agent> {
        let mut w = self.lock();
        if clock.is_some() {
            w.clock = clock;
        }
        let ever = w.client.get(&tab).is_some_and(|c| c.1);
        w.client
            .insert(tab, (block.is_some(), ever || block.is_some()));
        let Some(block) = block else {
            return w.last.get(&tab).cloned().unwrap_or_default();
        };
        if let Some(c) = clock {
            w.activity.entry(tab).or_default().sample(block.activity, c);
        }
        let mut seen = w.seen.remove(&tab).unwrap_or_default();
        let agents = states(block.panes.as_bytes(), &mut seen);
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
        Ok(out) if d.tab_target(tab).is_some() => {
            let (clock, found) = blocks(&out, 1);
            let block = found.into_iter().next().flatten();
            let agents = d.agents.record(tab, block.as_ref(), clock);
            match block {
                Some(_) => Reply::Agents { agents },
                None => Reply::Unavailable {
                    unavailable: NO_CLIENT,
                },
            }
        }
        Ok(_) => Reply::Agents { agents: vec![] },
        Err(why) => Reply::Unavailable { unavailable: why },
    }
}

/// Every open tab whose process runs, asked in one status run (status.md §Execution
/// Constraints: on Windows one `wsl.exe` for all tabs). Each tab's agents and activity are
/// recorded; the 5 s gate of [`agents_of`] counts this run. `Err` when the run failed.
pub fn poll_all(d: &Desktop) -> Result<(), &'static str> {
    let mut asked = Vec::new();
    let mut args = Vec::new();
    for (tab, pty, server) in d.running_tabs() {
        if asked.len() >= MAX_PAIRS || !d.host.alive(pty) {
            continue;
        }
        match d.host.tty(pty) {
            Ok(Some(t)) if crate::tmux_overlay::tty_ok(&t) => {
                asked.push(tab);
                args.push(t);
                args.push(server.word().to_string());
            }
            // No client tty yet: the tab is still opening.
            _ => {
                d.agents.record(tab, None, None);
            }
        }
    }
    if asked.is_empty() {
        return Ok(());
    }
    d.agents.lock().ran_at = Some(Instant::now());
    let out = d.platform.tmux_agents(&args)?;
    let (clock, found) = blocks(&out, asked.len());
    for (tab, block) in asked.into_iter().zip(found) {
        if d.tab_target(tab).is_some() {
            d.agents.record(tab, block.as_ref(), clock);
        }
    }
    Ok(())
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
        // Every tmux call goes through `on <server>` (the two `command tmux` lines pass the
        // caller's words on); the words `on` is given are read-only commands.
        let direct: Vec<&str> = code
            .split("tmux ")
            .skip(1)
            .map(|s| s.split_whitespace().next().unwrap_or(""))
            // `command -v tmux >/dev/null` looks tmux up; it is not a call.
            .filter(|c| !c.starts_with('>'))
            .collect();
        assert_eq!(direct, ["-L", "\"$@\""]);
        let verbs = |on: &str| -> Vec<String> {
            code.split(on)
                .skip(1)
                .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
                .collect()
        };
        assert_eq!(verbs("on app "), ["list-clients"]);
        assert_eq!(verbs("on user "), ["list-clients"]);
        assert_eq!(verbs("on \"$server\" "), ["list-panes"]);
        assert_eq!(
            code.matches(" on ").count() + code.matches("(on ").count(),
            3,
            "no other call"
        );
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
        fn rein_team_add(
            &self,
            _: &std::path::Path,
            _: &crate::team::Add,
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
        fn tmux_setup(&self, _: &str) -> Result<Vec<crate::tmux_server::SessionRow>, &'static str> {
            unreachable!()
        }
        fn rein_attach(
            &self,
            _: &std::path::Path,
            _: &str,
            _: Option<&str>,
        ) -> Result<crate::pty::ChildSpec, IpcError> {
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
            Ok(
                b"clock 9\ntab 0 0\nactivity 8\npane architect|claude|0\npane engineer|claude|0\n"
                    .to_vec(),
            ),
            Ok(
                b"clock 9\ntab 0 0\nactivity 8\npane architect|claude|0\npane engineer|zsh|0\n"
                    .to_vec(),
            ),
            Ok(
                b"clock 9\ntab 0 0\nactivity 8\npane architect|zsh|0\npane engineer|zsh|0\n"
                    .to_vec(),
            ),
            Ok(b"clock 9\ntab 0 5\n".to_vec()),
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
    fn one_answer_holds_every_tab_in_order_and_nothing_out_of_place() {
        let out = b"clock 1795016300\n\
tab 0 0\nactivity 1795016290\npane architect|claude|0\npane engineer|zsh|0\n\
tab 1 5\n\
tab 2 0\nactivity 1795016299\npane engineer|claude|0\n";
        let (clock, b) = blocks(out, 3);
        assert_eq!(clock, Some(1_795_016_300));
        assert_eq!(
            b,
            [
                Some(Block {
                    activity: 1_795_016_290,
                    panes: "architect|claude|0\nengineer|zsh|0\n".into()
                }),
                None,
                Some(Block {
                    activity: 1_795_016_299,
                    panes: "engineer|claude|0\n".into()
                }),
            ]
        );
        // Out of order, a repeated or a bad header: nothing after it is trusted.
        let (_, b) = blocks(b"clock 5\ntab 1 0\nactivity 4\nx|claude|0\n", 2);
        assert_eq!(b, [None, None]);
        let (_, b) = blocks(b"clock 5\ntab 0 0\nactivity 4\ntab 0 0\nactivity 4\n", 2);
        assert_eq!(b[0].as_ref().map(|x| x.activity), Some(4));
        assert_eq!(b[1], None);
        let (_, b) = blocks(b"clock 5\ntab 0 0 extra\nactivity 4\n", 1);
        assert_eq!(b, [None]);
        // Numbers are digits only, at most 12; a block without its activity line is none.
        for bad in [
            "clock -1\n",
            "clock 1e9\n",
            "clock 9999999999999\n",
            "clock \n",
        ] {
            assert_eq!(blocks(bad.as_bytes(), 0).0, None, "{bad}");
        }
        let (_, b) = blocks(b"tab 0 0\nactivity 12x\npane engineer|claude|0\n", 1);
        assert_eq!(b, [None]);
        let (_, b) = blocks(b"tab 0 0\nengineer|claude|0\n", 1);
        assert_eq!(b, [None]);
        // More tabs answered than asked: the extra ones are dropped.
        let (_, b) = blocks(b"tab 0 0\nactivity 1\ntab 1 0\nactivity 2\n", 1);
        assert_eq!(b.len(), 1);
        // A pane line must carry the marker; a role with a newline cannot print a header:
        // the script drops its continuation line, and `pane tab 1 0` is no header.
        let (_, b) = blocks(b"tab 0 0\nactivity 1\nengineer|claude|0\npane tab 1 0\n", 2);
        assert_eq!(b[0].as_ref().map(|x| x.panes.as_str()), Some("tab 1 0\n"));
        assert_eq!(b[1], None);
        // The pane lines go through `states`, which keeps only rein's roles.
        let mut seen = BTreeSet::new();
        let (_, b) = blocks(b"tab 0 0\nactivity 1\nclock 7\npane Evil|claude|0\n", 1);
        assert!(states(b[0].as_ref().unwrap().panes.as_bytes(), &mut seen).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn one_status_run_asks_every_tab_and_keeps_its_activity() {
        use crate::pty::{ChildSpec, Events, PtyId, Size};
        use std::sync::Arc;
        struct Quiet;
        impl Events for Quiet {
            fn output(&self, _: PtyId, _: Vec<u8>) {}
            fn exit(&self, _: PtyId, _: u32) {}
        }
        let outs = Arc::new(Mutex::new(vec![
            Ok(b"clock 1000\ntab 0 0\nactivity 999\npane engineer|claude|0\ntab 1 5\n".to_vec()),
            Ok(b"clock 1030\ntab 0 0\nactivity 1004\npane engineer|claude|0\ntab 1 0\nactivity 1030\npane architect|zsh|0\n".to_vec()),
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
        let size = Size { cols: 80, rows: 24 };
        let a = d.open_spec(&spec, p.path(), size, Arc::new(Quiet)).unwrap();
        let b = d.open_spec(&spec, p.path(), size, Arc::new(Quiet)).unwrap();
        poll_all(&d).unwrap();
        let tty = |t: &crate::ipc::Tab| d.host.tty(t.pty).unwrap().unwrap();
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [format!("status {} app {} app", tty(&a), tty(&b))],
            "one process for every tab"
        );
        let sa = d.agents.seen(a.id);
        assert!(sa.client_now && sa.client_ever);
        assert_eq!(sa.activity.quiet_s, Some(1));
        assert_eq!(sa.agents[0].state, AgentState::Running);
        let sb = d.agents.seen(b.id);
        assert!(!sb.client_now && !sb.client_ever, "no client yet");
        assert_eq!(sb.activity.quiet_s, None);
        poll_all(&d).unwrap();
        let sa = d.agents.seen(a.id);
        assert_eq!((sa.activity.quiet_s, sa.activity.worked_s), (Some(26), 5));
        assert!(d.agents.seen(b.id).client_ever);
        // The web side's ask within 5 s of the poll: the poll's answer, no new process.
        assert!(matches!(agents_of(&d, a.id, false), Reply::Agents { .. }));
        assert_eq!(calls.lock().unwrap().len(), 2);
        d.detach(a.id);
        d.detach(b.id);
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
