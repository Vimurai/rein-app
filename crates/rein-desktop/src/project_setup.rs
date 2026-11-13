//! Set-up and out-of-date (desktop.md §Look and feel v2 V9 components 1–3, D-057, E-63; the
//! ruling of 2026-11-13 14:10). The window runs only rein's own entry points, each on a click
//! and as a constant command: `rein init --check` for the verdict, `rein init` to set a folder
//! up (in the tab's own terminal, then `rein start`) or to bring a project up to date (output
//! in a sheet). What is missing or out of date is rein's verdict — `--check`'s exit code —
//! never a comparison made here; rein's lines are shown as text, capped.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, State};

use crate::ipc::{Desktop, IpcError, Tab, err};

/// The set-up child's exit code when `rein init` did not succeed (`rein start` never ran).
pub const INIT_FAILED: i32 = 119;
/// The user server's start, as a word of the set-up script.
const USER_START: &str = "exec ~/.rein/bin/rein start";

/// One `rein init --check` (V9: off the UI thread, limit 30 s).
pub const CHECK_LIMIT: Duration = Duration::from_secs(30);
/// One `rein init` from the bar (the ruling: a cold `npm ci` can take minutes).
pub const FIX_LIMIT: Duration = Duration::from_secs(600);
/// The ruling's caps on rein's rows.
pub const MAX_ROWS: usize = 40;
pub const MAX_ROW_CHARS: usize = 200;
/// At most this many `--check` runs per tab and stage: a started tab's one after it started
/// and the one after Update project (the ruling, 4); a set-up tab's card (and one spare).
/// Counted apart, so a tab set up later keeps its own budget — and the web side cannot start
/// `rein init --check` without bound.
pub const CHECKS_PER_TAB: u32 = 2;

/// The app's fixed sentences.
pub const NOT_ANSWERED: &str = "rein init --check did not give an answer";
pub const CHECK_TIMED_OUT: &str = "rein init --check did not finish within 30 seconds";

/// Which of rein's entry points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinRun {
    InitCheck,
    Init,
}

impl ReinRun {
    /// The arguments after `rein`.
    pub fn argv(self) -> &'static [&'static str] {
        match self {
            Self::InitCheck => &["init", "--check"],
            Self::Init => &["init"],
        }
    }

    /// The `-lic` word inside WSL: quote-free (V8's argv rule).
    pub fn wsl_script(self) -> &'static str {
        match self {
            Self::InitCheck => "exec ~/.rein/bin/rein init --check",
            Self::Init => "exec ~/.rein/bin/rein init",
        }
    }
}

/// The set-up child's script: `rein init`, then on exit 0 the tab's ordinary start on
/// `server` (V8). Inside WSL the tty line comes first, before init's output (V6).
pub fn setup_script(server: crate::tmux_server::Server, wsl: bool) -> String {
    let start = match server {
        crate::tmux_server::Server::App => crate::tmux_server::start_script(),
        crate::tmux_server::Server::User => USER_START,
    };
    // `rein init`, then — only on exit 0 — the tab's start, in one shell.
    let tty = if wsl { "tty; " } else { "" };
    format!("{tty}~/.rein/bin/rein init || exit {INIT_FAILED}; {start}")
}

/// rein's lines as rows: each made visible and cut (`panel_sources::shown`), at most
/// [`MAX_ROWS`], then "… and N more".
pub fn rows(lines: &[String]) -> Vec<String> {
    let lines: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let mut out: Vec<String> = lines
        .iter()
        .take(MAX_ROWS)
        .map(|l| crate::panel_sources::shown(l.trim_end(), MAX_ROW_CHARS, false))
        .collect();
    if lines.len() > MAX_ROWS {
        out.push(format!("… and {} more", lines.len() - MAX_ROWS));
    }
    out
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stopped {
    Cancelled,
    TimedOut,
}

/// Run `cmd` with stdin closed, stdout and stderr merged line by line into `line` (each line
/// raw, at most 4 KiB, at most [`crate::cli::MAX_LINES`]), for at most `limit` and until
/// `cancel` is set. `Ok((code, None))` when it exited; `Ok((None, Some(why)))` when it was
/// killed. A grandchild that keeps a pipe open is not waited for past 2 s after the exit.
pub fn run_lines(
    mut cmd: Command,
    limit: Duration,
    cancel: &AtomicBool,
    line: &mut dyn FnMut(String),
) -> std::io::Result<(Option<i32>, Option<Stopped>)> {
    let deadline = Instant::now() + limit;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let (tx, rx) = mpsc::channel::<String>();
    let pipes: [Option<Box<dyn Read + Send>>; 2] = [
        child.stdout.take().map(|o| Box::new(o) as _),
        child.stderr.take().map(|e| Box::new(e) as _),
    ];
    for pipe in pipes.into_iter().flatten() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for l in BufReader::new(pipe).split(b'\n').map_while(Result::ok) {
                let mut end = l.len().min(crate::cli::MAX_LINE);
                let text = loop {
                    match std::str::from_utf8(&l[..end]) {
                        Ok(t) => break t.to_string(),
                        Err(e) if e.valid_up_to() > 0 && e.error_len().is_none() => {
                            end = e.valid_up_to()
                        }
                        Err(_) => break String::from_utf8_lossy(&l[..end]).into_owned(),
                    }
                };
                if tx.send(text.trim_end_matches('\r').to_string()).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);
    let mut n = 0usize;
    let mut emit = |l: String, line: &mut dyn FnMut(String)| {
        n += 1;
        if n <= crate::cli::MAX_LINES {
            line(l);
        }
    };
    let code = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(l) => emit(l, line),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(s) = child.try_wait()? {
                    break s.code();
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(s) = child.try_wait()? {
            break s.code();
        }
        let why = if cancel.load(Ordering::Acquire) {
            Some(Stopped::Cancelled)
        } else if Instant::now() >= deadline {
            Some(Stopped::TimedOut)
        } else {
            None
        };
        if let Some(why) = why {
            let _ = child.kill();
            let _ = child.wait();
            return Ok((None, Some(why)));
        }
    };
    let drain_until = Instant::now() + Duration::from_secs(2);
    while let Ok(l) = rx.recv_timeout(drain_until.saturating_duration_since(Instant::now())) {
        emit(l, line);
    }
    Ok((code, None))
}

/// A tab that shows the set-up card: no pty yet.
#[derive(Debug, Clone)]
pub struct SetupTab {
    pub project: PathBuf,
    pub server: crate::tmux_server::Server,
}

/// What the window keeps for set-up and the out-of-date check.
#[derive(Default)]
pub struct ProjectState {
    pub setup: Mutex<HashMap<u32, SetupTab>>,
    checks: Mutex<HashMap<(u32, bool), u32>>,
    /// The one `rein init` run's Cancel (one run at a time, through the CLI runner's lock).
    cancel: AtomicBool,
    /// The last check that gave no answer, as one health line (the ruling, 1).
    health: Mutex<Option<&'static str>>,
}

impl ProjectState {
    pub fn forget(&self, tab: u32) {
        self.setup
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&tab);
        self.checks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|(t, _), _| *t != tab);
    }

    pub fn setup_tab(&self, tab: u32) -> Option<SetupTab> {
        self.setup
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&tab)
            .cloned()
    }

    pub fn health(&self) -> Option<&'static str> {
        *self.health.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn note(&self, failure: Option<&'static str>) {
        *self.health.lock().unwrap_or_else(|e| e.into_inner()) = failure;
    }

    /// One more check for `tab` (`setup`: while it shows the card), if it has one left.
    fn take_check(&self, tab: u32, setup: bool) -> bool {
        let mut c = self.checks.lock().unwrap_or_else(|e| e.into_inner());
        let n = c.entry((tab, setup)).or_default();
        if *n >= CHECKS_PER_TAB {
            return false;
        }
        *n += 1;
        true
    }
}

/// A tab opened on a folder without `.ai/` (V9 component 2).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetupView {
    pub id: u32,
    pub project: String,
    pub title: String,
}

/// `tab_open`'s answer: a tab with its terminal, or a set-up card.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Opened {
    Tab(Tab),
    SetUp { setup: SetupView },
}

/// `project_check`'s answer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Check {
    Ready { ready: bool },
    NotSetUp { not_set_up: Vec<String> },
    OutOfDate { out_of_date: Vec<String> },
    Unavailable { unavailable: &'static str },
}

/// `rein init --check` for `tab`: the card's rows (a set-up tab), or the out-of-date verdict
/// (a started tab, at most [`CHECKS_PER_TAB`] times).
pub fn check(d: &Desktop, tab: u32) -> Result<Check, IpcError> {
    let setup = d.projects.setup_tab(tab);
    let project = match &setup {
        Some(s) => s.project.clone(),
        None => d
            .tab_project(tab)
            .ok_or_else(|| err("unknown_tab", "the tab is not open"))?,
    };
    if !d.projects.take_check(tab, setup.is_some()) {
        return Ok(Check::Unavailable {
            unavailable: NOT_ANSWERED,
        });
    }
    let cmd = d.platform.rein_command(&project, ReinRun::InitCheck)?;
    let mut lines = Vec::new();
    let never = AtomicBool::new(false);
    let run = run_lines(cmd, CHECK_LIMIT, &never, &mut |l| lines.push(l));
    let verdict = match (run, setup.is_some()) {
        (Ok((Some(0), _)), _) => Check::Ready { ready: true },
        (Ok((Some(1), _)), true) => Check::NotSetUp {
            not_set_up: rows(&lines),
        },
        (Ok((Some(1), _)), false) => Check::OutOfDate {
            out_of_date: rows(&lines),
        },
        (Ok((None, Some(Stopped::TimedOut))), _) => Check::Unavailable {
            unavailable: CHECK_TIMED_OUT,
        },
        _ => Check::Unavailable {
            unavailable: NOT_ANSWERED,
        },
    };
    if setup.is_none() {
        d.projects.note(match verdict {
            Check::Unavailable { unavailable } => Some(unavailable),
            _ => None,
        });
    }
    Ok(verdict)
}

/// How Update project ended.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FixEnd {
    pub code: Option<i32>,
    pub stopped: Option<Stopped>,
}

/// `rein init` for a started tab's project, its lines escaped to `line`.
pub fn fix(d: &Desktop, tab: u32, line: &mut dyn FnMut(String)) -> Result<FixEnd, IpcError> {
    let project = d
        .tab_project(tab)
        .ok_or_else(|| err("unknown_tab", "the tab is not open"))?;
    let cmd = d.platform.rein_command(&project, ReinRun::Init)?;
    d.projects.cancel.store(false, Ordering::Release);
    let (code, stopped) = run_lines(cmd, FIX_LIMIT, &d.projects.cancel, &mut |l| {
        line(crate::cli::bounded(&l))
    })
    .map_err(|e| err("cli", format!("cannot run rein init: {e}")))?;
    Ok(FixEnd { code, stopped })
}

/// `project_check(tab)`.
#[tauri::command(async)]
pub fn project_check(state: State<'_, Desktop>, tab: u32) -> Result<Check, IpcError> {
    check(&state, tab)
}

/// `project_setup(tab, cols, rows, output)`: the set-up tab gets its terminal — `rein init`,
/// then `rein start` — and becomes an ordinary tab with the same id.
#[tauri::command(async)]
pub fn project_setup(
    app: AppHandle,
    state: State<'_, Desktop>,
    tab: u32,
    cols: u32,
    rows: u32,
    output: Channel<InvokeResponseBody>,
) -> Result<Tab, IpcError> {
    state.setup(tab, cols, rows, crate::ipc::channel_events(output, app))
}

/// `project_fix(tab, which, output)`: `which` is `init` (the ruling, 2); one run at a time
/// with the CLI runner's other runs; 10 minutes; [`project_fix_cancel`] stops it.
#[tauri::command(async)]
pub fn project_fix(
    state: State<'_, Desktop>,
    cli_state: State<'_, crate::cli_ipc::CliState>,
    tab: u32,
    which: String,
    output: Channel<String>,
) -> Result<FixEnd, IpcError> {
    if which != "init" {
        return Err(err("bad_kind", "init is the only fix"));
    }
    let _busy = cli_state.busy.begin()?;
    fix(&state, tab, &mut |l| {
        let _ = output.send(l);
    })
}

/// Cancel the running Update project.
#[tauri::command(async)]
pub fn project_fix_cancel(state: State<'_, Desktop>) {
    state.projects.cancel.store(true, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux_server::Server;

    #[test]
    fn the_setup_script_runs_init_then_the_tabs_own_start() {
        let app = setup_script(Server::App, false);
        assert!(app.starts_with("~/.rein/bin/rein init || exit 119; set -o noglob;"));
        assert!(app.ends_with(crate::tmux_server::start_script()));
        assert_eq!(
            setup_script(Server::User, false),
            "~/.rein/bin/rein init || exit 119; exec ~/.rein/bin/rein start"
        );
        // Inside WSL the tty line comes first, before init's output (V6's 4 KiB window).
        assert_eq!(
            setup_script(Server::User, true),
            "tty; ~/.rein/bin/rein init || exit 119; exec ~/.rein/bin/rein start"
        );
        assert!(setup_script(Server::App, true).starts_with("tty; ~/.rein/bin/rein init"));
        // V8's argv rule: one line, no quote character, for every script.
        for s in [
            setup_script(Server::App, false),
            setup_script(Server::App, true),
            setup_script(Server::User, true),
            ReinRun::Init.wsl_script().into(),
            ReinRun::InitCheck.wsl_script().into(),
        ] {
            assert!(!s.contains(['"', '\'', '\n']), "{s}");
        }
        assert_eq!(ReinRun::InitCheck.argv(), ["init", "--check"]);
        assert_eq!(ReinRun::Init.argv(), ["init"]);
    }

    /// The set-up script at its process boundary: `sh -c` with a stand-in rein in HOME.
    #[cfg(unix)]
    #[test]
    fn init_then_start_in_one_shell_and_a_failed_init_never_starts() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".rein/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let log = home.path().join("log");
        std::fs::write(
            bin.join("rein"),
            format!(
                "#!/bin/sh\necho \"$*\" >>{}\n[ \"$1\" = init ] && [ -n \"$FAIL\" ] && exit 3\nexit 0\n",
                log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(bin.join("rein"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let run = |fail: bool| {
            let _ = std::fs::remove_file(&log);
            let mut c = Command::new("/bin/sh");
            c.args(["-c", &setup_script(Server::User, false)])
                .env("HOME", home.path());
            if fail {
                c.env("FAIL", "1");
            }
            let code = c.status().unwrap().code();
            (code, std::fs::read_to_string(&log).unwrap_or_default())
        };
        assert_eq!(run(false), (Some(0), "init\nstart\n".into()));
        assert_eq!(run(true), (Some(INIT_FAILED), "init\n".into()));
    }

    #[test]
    fn rows_are_capped_made_visible_and_counted() {
        let many: Vec<String> = (0..45).map(|i| format!("  refresh  f{i}")).collect();
        let r = rows(&many);
        assert_eq!(r.len(), MAX_ROWS + 1);
        assert_eq!(r[0], "  refresh  f0");
        assert_eq!(r[MAX_ROWS], "… and 5 more");
        let hostile = vec![
            format!("x{}", "y".repeat(500)),
            "a\u{1b}[31mred\u{202e}b".into(),
            String::new(),
        ];
        let r = rows(&hostile);
        assert_eq!(r.len(), 2, "blank lines are not rows");
        assert!(r[0].chars().count() <= MAX_ROW_CHARS + 1);
        assert_eq!(r[1], "a␛[31mred⟨U+202E⟩b");
        // rein's own line for a folder without `.ai/` is one row, as it is.
        let one = "rein init --check: no .ai/ here — rein init would provision a new project.";
        assert_eq!(rows(&[one.into()]), [one]);
    }

    #[cfg(unix)]
    #[test]
    fn a_run_streams_lines_and_stops_at_the_limit_or_on_cancel() {
        let sh = |script: &str| {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", script]);
            c
        };
        let never = AtomicBool::new(false);
        let mut got = Vec::new();
        let end = run_lines(
            sh("echo one; echo two >&2; exit 1"),
            CHECK_LIMIT,
            &never,
            &mut |l| got.push(l),
        )
        .unwrap();
        assert_eq!(end, (Some(1), None));
        got.sort();
        assert_eq!(got, ["one", "two"]);
        let t = Instant::now();
        let end = run_lines(
            sh("sleep 30"),
            Duration::from_millis(300),
            &never,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(end, (None, Some(Stopped::TimedOut)));
        assert!(t.elapsed() < Duration::from_secs(5));
        let cancel = AtomicBool::new(true);
        let end = run_lines(sh("sleep 30"), CHECK_LIMIT, &cancel, &mut |_| {}).unwrap();
        assert_eq!(end, (None, Some(Stopped::Cancelled)));
        // A grandchild holding the pipe does not hold the run past 2 s after the exit.
        let t = Instant::now();
        let end = run_lines(sh("sleep 30 & echo bye"), CHECK_LIMIT, &never, &mut |_| {}).unwrap();
        assert_eq!(end.0, Some(0));
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    /// The web side cannot start `rein init --check` without bound: two per tab and stage.
    #[cfg(unix)]
    #[test]
    fn checks_are_capped_per_tab() {
        use crate::pty::{Events, PtyId};
        struct Quiet;
        impl Events for Quiet {
            fn output(&self, _: PtyId, _: Vec<u8>) {}
            fn exit(&self, _: PtyId, _: u32) {}
        }
        // No rein installed: a check that is allowed fails at the command, before a process.
        let d = Desktop::new(Box::new(crate::local_host::LocalHost::new(Some(
            "/nonexistent-home".into(),
        ))));
        let dir = tempfile::tempdir().unwrap();
        let id = match d
            .open(
                dir.path().to_str().unwrap(),
                80,
                24,
                std::sync::Arc::new(Quiet),
                Server::User,
            )
            .unwrap()
        {
            Opened::SetUp { setup } => setup.id,
            other => panic!("{other:?}"),
        };
        for _ in 0..CHECKS_PER_TAB {
            assert_eq!(check(&d, id).unwrap_err().kind, "rein_missing");
        }
        assert_eq!(
            check(&d, id).unwrap(),
            Check::Unavailable {
                unavailable: NOT_ANSWERED
            }
        );
        assert_eq!(check(&d, id + 50).unwrap_err().kind, "unknown_tab");
        d.detach(id);
        assert_eq!(check(&d, id).unwrap_err().kind, "unknown_tab");
    }

    #[test]
    fn the_answers_shapes() {
        let j = |c: Check| serde_json::to_value(c).unwrap();
        assert_eq!(
            j(Check::Ready { ready: true }),
            serde_json::json!({"ready": true})
        );
        assert_eq!(
            j(Check::NotSetUp {
                not_set_up: vec!["x".into()]
            }),
            serde_json::json!({"not_set_up": ["x"]})
        );
        assert_eq!(
            j(Check::OutOfDate {
                out_of_date: vec![]
            }),
            serde_json::json!({"out_of_date": []})
        );
        assert_eq!(
            serde_json::to_value(FixEnd {
                code: None,
                stopped: Some(Stopped::Cancelled)
            })
            .unwrap(),
            serde_json::json!({"code": null, "stopped": "cancelled"})
        );
    }
}
