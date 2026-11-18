//! A start that fails (desktop.md V14 components 1, 2 and 5, D-068): when a tab's end is a
//! failed start, what rein's check maps to, the check's budget, and set-up of a failed tab.
//! A stub host answers the session lookup and runs `rein init --check` as a small script.

use super::*;
use crate::project_setup::{Check, Failed, FailedAnswer, check, failed_verdict};
use crate::tmux_server::{Server, SessionRow};
use std::sync::atomic::AtomicU32;
use std::sync::mpsc;
use std::time::Duration;
use stub::FailHost;

struct Ended(Mutex<Option<mpsc::Sender<u32>>>);
impl Events for Ended {
    fn output(&self, _: PtyId, _: Vec<u8>) {}
    fn exit(&self, _: PtyId, code: u32) {
        if let Some(tx) = self.0.lock().unwrap().take() {
            let _ = tx.send(code);
        }
    }
}

fn desktop(sessions: Vec<SessionRow>, check: &'static str) -> (Desktop, Arc<AtomicU32>) {
    let runs = Arc::new(AtomicU32::new(0));
    let host = FailHost {
        sessions,
        check,
        runs: runs.clone(),
    };
    (Desktop::new(Box::new(host)), runs)
}

/// A tab whose child exits at once with `code`, opened as `kind`; returns it after the exit.
fn ended_tab(d: &Desktop, project: &Path, kind: ChildKind, code: u32) -> Tab {
    let spec = ChildSpec {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), format!("exit {code}").into()],
        cwd: Some(project.into()),
        env_remove: crate::local_host::ENV_REMOVED,
        env_set: crate::local_host::ENV_SET,
        tty_line: false,
    };
    let (tx, rx) = mpsc::channel();
    let tab = d
        .open_spec_as(
            (&spec, kind),
            project,
            Size { cols: 80, rows: 24 },
            Arc::new(Ended(Mutex::new(Some(tx)))),
            Server::App,
            None,
        )
        .unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), code);
    tab
}

#[test]
fn a_failed_start_is_an_ordinary_start_that_ended_non_zero_with_no_session() {
    // Before the lookup: only an ordinary start on the app's server, and never exit 0.
    assert!(failed_start(ChildKind::Start, Server::App, 2));
    assert!(!failed_start(ChildKind::Start, Server::App, 0));
    assert!(!failed_start(ChildKind::Attach, Server::App, 2));
    assert!(!failed_start(ChildKind::Setup, Server::App, 119));
    assert!(!failed_start(ChildKind::Start, Server::User, 2));

    let p = tempfile::tempdir().unwrap();
    let (d, _) = desktop(vec![], "exit 0");
    let t = ended_tab(&d, p.path(), ChildKind::Start, 2);
    assert!(d.start_failed(t.pty, 2), "no session afterwards: failed");
    assert_eq!(d.projects.failed(t.id), Some(Failed::Pending));
    let ok = ended_tab(&d, p.path(), ChildKind::Start, 0);
    assert!(!d.start_failed(ok.pty, 0), "exit 0 is never a failed start");
    let attach = ended_tab(&d, p.path(), ChildKind::Attach, 2);
    assert!(
        !d.start_failed(attach.pty, 2),
        "an attach is never a failed start"
    );
    assert_eq!(d.projects.failed(attach.id), None);

    // The session is alive afterwards (a non-zero end of a start that did start): not failed.
    let tag = d.tag_of(p.path());
    let live = SessionRow {
        id: "$1".into(),
        tag,
        path: p.path().to_string_lossy().into_owned(),
    };
    let (d2, _) = desktop(vec![live], "exit 0");
    let t2 = ended_tab(&d2, p.path(), ChildKind::Start, 2);
    assert!(!d2.start_failed(t2.pty, 2));
    assert_eq!(d2.projects.failed(t2.id), None);
    for x in [t.id, ok.id, attach.id] {
        d.detach(x);
    }
    d2.detach(t2.id);
}

#[test]
fn the_check_after_a_failed_start_maps_exit_1_0_and_a_timeout() {
    let rows = vec!["refresh CLAUDE.md".to_string()];
    assert_eq!(
        failed_verdict(Some(1), rows.clone()),
        Failed::NeedsInit(rows)
    );
    assert_eq!(failed_verdict(Some(0), vec![]), Failed::NoFix);
    assert_eq!(failed_verdict(None, vec![]), Failed::NoFix, "a timeout");
    assert_eq!(
        failed_verdict(Some(2), vec![]),
        Failed::NoFix,
        "any other end"
    );

    // exit 1 → needs_init with rein's rows, asked once: the verdict is kept.
    let p = tempfile::tempdir().unwrap();
    let (d, runs) = desktop(vec![], "printf 'pending:\\nrefresh CLAUDE.md\\n'; exit 1");
    let t = ended_tab(&d, p.path(), ChildKind::Start, 2);
    assert!(d.start_failed(t.pty, 2));
    let want = Check::StartFailed {
        start_failed: FailedAnswer::NeedsInit {
            needs_init: vec!["pending:".into(), "refresh CLAUDE.md".into()],
        },
    };
    assert_eq!(check(&d, t.id).unwrap(), want);
    assert_eq!(check(&d, t.id).unwrap(), want);
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "one --check per failed child"
    );
    let shape = serde_json::to_value(&want).unwrap();
    assert_eq!(shape["start_failed"]["needs_init"][1], "refresh CLAUDE.md");

    // exit 0 → no_fix.
    let (d0, _) = desktop(vec![], "exit 0");
    let t0 = ended_tab(&d0, p.path(), ChildKind::Start, 2);
    assert!(d0.start_failed(t0.pty, 2));
    let no_fix = check(&d0, t0.id).unwrap();
    assert_eq!(
        serde_json::to_value(&no_fix).unwrap(),
        serde_json::json!({"start_failed": {"no_fix": true}})
    );
    d.detach(t.id);
    d0.detach(t0.id);
}

#[test]
fn an_answer_this_tab_already_had_is_used_and_the_budget_holds() {
    // The out-of-date check answered before the start failed: its answer is the verdict.
    let p = tempfile::tempdir().unwrap();
    let (d, runs) = desktop(vec![], "echo 'refresh CLAUDE.md'; exit 1");
    let t = ended_tab(&d, p.path(), ChildKind::Start, 2);
    assert!(matches!(check(&d, t.id).unwrap(), Check::OutOfDate { .. }));
    assert!(d.start_failed(t.pty, 2));
    assert!(matches!(
        check(&d, t.id).unwrap(),
        Check::StartFailed {
            start_failed: FailedAnswer::NeedsInit { .. }
        }
    ));
    assert_eq!(runs.load(Ordering::SeqCst), 1, "no second run");
    d.detach(t.id);

    // Try again held down: each failed child is a new tab; past the project's budget the
    // verdict is no_fix and rein is not run.
    let (d, runs) = desktop(vec![], "exit 1");
    let n = crate::project_setup::FAILED_CHECKS_PER_PROJECT;
    let mut verdicts = Vec::new();
    for _ in 0..n + 2 {
        let t = ended_tab(&d, p.path(), ChildKind::Start, 2);
        assert!(d.start_failed(t.pty, 2));
        verdicts.push(matches!(
            check(&d, t.id).unwrap(),
            Check::StartFailed {
                start_failed: FailedAnswer::NeedsInit { .. }
            }
        ));
        d.detach(t.id);
    }
    assert_eq!(runs.load(Ordering::SeqCst), n);
    let mut want = vec![true; n as usize];
    want.extend([false, false]);
    assert_eq!(verdicts, want);
}

#[test]
fn project_setup_takes_a_failed_tab_only_in_needs_init() {
    let p = tempfile::tempdir().unwrap();
    let (d, _) = desktop(vec![], "echo 'refresh CLAUDE.md'; exit 1");
    let events = || -> Arc<dyn Events> { Arc::new(Ended(Mutex::new(None))) };
    // A started tab that did not fail: refused, and it stays as it is.
    let started = ended_tab(&d, p.path(), ChildKind::Start, 0);
    assert_eq!(
        d.setup(started.id, 80, 24, events()).unwrap_err().kind,
        "not_failed"
    );
    assert!(d.tab_target(started.id).is_some());
    assert_eq!(
        d.setup(999, 80, 24, events()).unwrap_err().kind,
        "unknown_tab"
    );
    // Failed, rein not asked yet: not in needs_init either.
    let failed = ended_tab(&d, p.path(), ChildKind::Start, 2);
    assert!(d.start_failed(failed.pty, 2));
    assert_eq!(d.failed_to_setup(failed.id).unwrap_err().kind, "not_failed");
    // In needs_init: it becomes a set-up tab under its own id, its ended pty let go.
    check(&d, failed.id).unwrap();
    let s = d.failed_to_setup(failed.id).unwrap();
    assert_eq!(s.project, p.path());
    assert!(d.tab_target(failed.id).is_none());
    assert!(d.projects.setup_tab(failed.id).is_some());
    assert_eq!(d.projects.failed(failed.id), None);
    d.detach(started.id);
    d.detach(failed.id);
}

// The stub host is in a test module of its own: the spawn-site guard (tests/spawn_sites.rs)
// reads a file up to its first test module, and this one runs a shell as rein's stand-in.
#[cfg(test)]
mod stub {
    use super::*;

    /// The lookup's rows and the check's script; counts the check's runs.
    pub(super) struct FailHost {
        pub(super) sessions: Vec<SessionRow>,
        pub(super) check: &'static str,
        pub(super) runs: Arc<AtomicU32>,
    }

    impl Host for FailHost {
        fn project_dir(
            &self,
            _: &str,
        ) -> Result<(std::path::PathBuf, crate::host::Found), IpcError> {
            unreachable!()
        }
        fn rein_setup(&self, _: &Path, _: Server) -> Result<ChildSpec, IpcError> {
            unreachable!()
        }
        fn rein_command(
            &self,
            _: &Path,
            which: crate::project_setup::ReinRun,
        ) -> Result<std::process::Command, IpcError> {
            assert_eq!(which, crate::project_setup::ReinRun::InitCheck);
            self.runs.fetch_add(1, Ordering::SeqCst);
            let mut c = std::process::Command::new("/bin/sh");
            c.args(["-c", self.check]);
            Ok(c)
        }
        fn rein_team_add(
            &self,
            _: &Path,
            _: &crate::team::Add,
        ) -> Result<std::process::Command, IpcError> {
            unreachable!()
        }
        fn rein_start(&self, _: &Path, _: Server) -> Result<ChildSpec, IpcError> {
            unreachable!()
        }
        fn tmux_setup(&self, _: &str) -> Result<Vec<SessionRow>, &'static str> {
            Ok(self.sessions.clone())
        }
        fn rein_attach(&self, _: &Path, _: &str, _: Option<&str>) -> Result<ChildSpec, IpcError> {
            unreachable!()
        }
        fn read_project_file(
            &self,
            _: &Path,
            _: crate::project_file::ProjectFile,
            _: bool,
        ) -> Result<crate::project_file::FileBytes, crate::project_file::Unavailable> {
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
        fn tmux_agents(&self, _: &[String]) -> Result<Vec<u8>, &'static str> {
            unreachable!()
        }
        fn rein_kill(&self, _: &Path, _: Server) -> Result<(), &'static str> {
            unreachable!()
        }
        fn dialog_start(&self) -> Option<std::path::PathBuf> {
            None
        }
    }
}
