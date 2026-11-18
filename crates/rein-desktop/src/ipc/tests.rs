//! The tests of `ipc` (moved from ipc.rs unchanged, E-83).

use super::*;
use crate::local_host::{LocalHost, validate_project};
use std::sync::mpsc;
use std::time::Duration;

struct Quiet(Mutex<Option<mpsc::Sender<u32>>>);
impl Events for Quiet {
    fn output(&self, _: PtyId, _: Vec<u8>) {}
    fn exit(&self, _: PtyId, code: u32) {
        if let Some(tx) = self.0.lock().unwrap().take() {
            let _ = tx.send(code);
        }
    }
}

fn project() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join(".ai")).unwrap();
    t
}

fn kind<T: std::fmt::Debug>(r: Result<T, IpcError>) -> &'static str {
    r.unwrap_err().kind
}

#[test]
fn tab_open_refuses_bad_projects() {
    let d = Desktop::new(Box::new(LocalHost::new(Some("/nonexistent-home".into()))));
    let q = || -> Arc<dyn Events> { Arc::new(Quiet(Mutex::new(None))) };
    // V9 (E-63): a directory without `.ai/` is not refused — it opens as a set-up tab,
    // with no process, counted against the tab limit, forgotten when closed.
    let plain = tempfile::tempdir().unwrap();
    let p = plain.path().to_str().unwrap();
    let canon = plain.path().canonicalize().unwrap();
    let id = match d.open(p, 80, 24, q(), crate::tmux_server::Server::User) {
        Ok(crate::project_setup::Opened::SetUp { setup }) => {
            assert_eq!(setup.project, canon.to_string_lossy());
            setup.id
        }
        other => panic!("{other:?}"),
    };
    assert!(d.tabs().is_empty(), "no pty for a set-up tab");
    assert!(
        d.tab_project(id).is_none(),
        "not a project tab until set up"
    );
    for _ in 1..MAX_TABS {
        d.open(p, 80, 24, q(), crate::tmux_server::Server::User)
            .unwrap();
    }
    assert_eq!(
        kind(d.open(p, 80, 24, q(), crate::tmux_server::Server::User)),
        "too_many_tabs"
    );
    for t in 0..=MAX_TABS as u32 + 1 {
        d.detach(t);
    }
    assert!(d.projects.setup.lock().unwrap().is_empty());
    // Set up on a host without rein: refused before any process, the tab still waiting.
    let id = match d.open(p, 80, 24, q(), crate::tmux_server::Server::User) {
        Ok(crate::project_setup::Opened::SetUp { setup }) => setup.id,
        other => panic!("{other:?}"),
    };
    assert_eq!(kind(d.setup(id, 80, 24, q())), "rein_missing");
    assert!(d.projects.setup_tab(id).is_some());
    assert_eq!(kind(d.setup(id + 100, 80, 24, q())), "unknown_tab");
    d.detach(id);
    // E-63 rework 3: a folder inside a project carries the project and is never set up.
    let proj = project();
    let sub = proj.path().join("src");
    std::fs::create_dir(&sub).unwrap();
    let id = match d.open(
        sub.to_str().unwrap(),
        80,
        24,
        q(),
        crate::tmux_server::Server::User,
    ) {
        Ok(crate::project_setup::Opened::SetUp { setup }) => {
            assert_eq!(
                setup.inside.as_deref(),
                Some(proj.path().canonicalize().unwrap().to_str().unwrap())
            );
            setup.id
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(kind(d.setup(id, 80, 24, q())), "inside_project");
    assert_eq!(
        crate::project_setup::check(&d, id).unwrap_err().kind,
        "inside_project"
    );
    assert!(d.tabs().is_empty());
    d.detach(id);
    assert_eq!(
        kind(d.open(
            "/a\u{1b}[2Jb",
            80,
            24,
            q(),
            crate::tmux_server::Server::User
        )),
        "unsafe_path"
    );
    assert_eq!(
        kind(d.open("/a\u{202e}b", 80, 24, q(), crate::tmux_server::Server::User)),
        "unsafe_path"
    );
    assert_eq!(
        kind(d.open("relative", 80, 24, q(), crate::tmux_server::Server::User)),
        "unsafe_path"
    );
    assert_eq!(
        kind(d.open(
            "/definitely/not/here",
            80,
            24,
            q(),
            crate::tmux_server::Server::User
        )),
        "not_a_directory"
    );
    let file = plain.path().join("f");
    std::fs::write(&file, "").unwrap();
    assert_eq!(
        kind(d.open(
            file.to_str().unwrap(),
            80,
            24,
            q(),
            crate::tmux_server::Server::User
        )),
        "not_a_directory"
    );
    let proj = project();
    // A valid project, but rein is not installed under this home.
    assert_eq!(
        kind(d.open(
            proj.path().to_str().unwrap(),
            80,
            24,
            q(),
            crate::tmux_server::Server::User
        )),
        "rein_missing"
    );
    assert_eq!(
        kind(d.open(
            proj.path().to_str().unwrap(),
            1,
            24,
            q(),
            crate::tmux_server::Server::User
        )),
        "out_of_range"
    );
}

#[test]
fn a_symlink_to_a_project_opens_as_its_canonical_path() {
    let proj = project();
    let link_dir = tempfile::tempdir().unwrap();
    let link = link_dir.path().join("link");
    std::os::unix::fs::symlink(proj.path(), &link).unwrap();
    assert_eq!(
        validate_project(link.to_str().unwrap()).unwrap(),
        proj.path().canonicalize().unwrap()
    );
}

#[test]
fn a_clipboard_write_is_bounded() {
    assert_eq!(
        kind(validate_clipboard(&"x".repeat(MAX_CLIPBOARD + 1))),
        "too_large"
    );
    assert!(validate_clipboard(&"x".repeat(MAX_CLIPBOARD)).is_ok());
}

#[test]
fn resize_and_write_are_bounded() {
    for (c, r) in [
        (1, 24),
        (501, 24),
        (80, 1),
        (80, 201),
        (0, 0),
        (u32::MAX, 24),
    ] {
        assert_eq!(kind(validate_size(c, r)), "out_of_range", "{c}x{r}");
    }
    assert!(validate_size(2, 2).is_ok() && validate_size(500, 200).is_ok());
    assert_eq!(
        kind(validate_write(&"x".repeat(MAX_WRITE + 1))),
        "too_large"
    );
    assert!(validate_write(&"x".repeat(MAX_WRITE)).is_ok());
}

#[test]
fn unknown_ptys_and_oversized_acks_are_refused() {
    let d = Desktop::new(Box::new(LocalHost::new(None)));
    assert_eq!(kind(d.write(42, "x")), "unknown_pty");
    assert_eq!(kind(d.resize(42, 80, 24)), "unknown_pty");
    assert_eq!(kind(d.ack(42, 1)), "unknown_pty");
    d.detach(42); // unknown tab: no error

    let proj = project();
    let (tx, rx) = mpsc::channel();
    let spec = ChildSpec {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "cat".into()],
        cwd: Some(proj.path().into()),
        env_remove: crate::local_host::ENV_REMOVED,
        env_set: crate::local_host::ENV_SET,
        tty_line: false,
    };
    let tab = d
        .open_spec(
            &spec,
            proj.path(),
            Size { cols: 80, rows: 24 },
            Arc::new(Quiet(Mutex::new(Some(tx)))),
        )
        .unwrap();
    // Nothing has been sent yet, so any ack is more than is outstanding.
    assert_eq!(kind(d.ack(tab.pty, 1)), "ack_too_large");
    assert_eq!(
        d.host.unacked(tab.pty).unwrap(),
        0,
        "a refused ack changes nothing"
    );
    assert_eq!(
        kind(d.write(tab.pty, &"x".repeat(MAX_WRITE + 1))),
        "too_large"
    );
    assert_eq!(kind(d.resize(tab.pty, 80, 999)), "out_of_range");
    d.detach(tab.id);
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(kind(d.write(tab.pty, "x")), "unknown_pty");
}

#[test]
fn at_most_sixteen_tabs_even_when_opened_concurrently() {
    let d = Arc::new(Desktop::new(Box::new(LocalHost::new(None))));
    let proj = project();
    let spec = ChildSpec {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "cat".into()],
        cwd: Some(proj.path().into()),
        env_remove: crate::local_host::ENV_REMOVED,
        env_set: crate::local_host::ENV_SET,
        tty_line: false,
    };
    let handles: Vec<_> = (0..24)
        .map(|_| {
            let (d, spec, p) = (Arc::clone(&d), spec.clone(), proj.path().to_path_buf());
            std::thread::spawn(move || {
                let q: Arc<dyn Events> = Arc::new(Quiet(Mutex::new(None)));
                d.open_spec(&spec, &p, Size { cols: 80, rows: 24 }, q)
                    .is_ok()
            })
        })
        .collect();
    let opened = handles
        .into_iter()
        .filter(|_| true)
        .map(|h| h.join().unwrap())
        .filter(|&ok| ok)
        .count();
    assert_eq!(opened, MAX_TABS);
    d.detach_all();
}

/// A host that records the overlay's words and answers with `answer`.
struct OverlayHost {
    seen: Arc<Mutex<Vec<Vec<String>>>>,
    answer: Result<(), String>,
    /// Stands in for a script still waiting for a tmux client that never attaches: the
    /// run lasts until `alive` turns false (at most 10 s).
    waits: bool,
}
impl Host for OverlayHost {
    fn project_dir(&self, _: &str) -> Result<(std::path::PathBuf, crate::host::Found), IpcError> {
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
    fn rein_start(&self, _: &Path, _: crate::tmux_server::Server) -> Result<ChildSpec, IpcError> {
        unreachable!()
    }
    fn tmux_setup(&self, _: &str) -> Result<Vec<crate::tmux_server::SessionRow>, &'static str> {
        unreachable!()
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
    fn tmux_overlay(&self, args: &[String], alive: &dyn Fn() -> bool) -> Result<(), String> {
        self.seen.lock().unwrap().push(args.to_vec());
        let until = std::time::Instant::now() + Duration::from_secs(10);
        while self.waits && alive() && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        self.answer.clone()
    }
    fn tmux_agents(&self, _: &[String]) -> Result<Vec<u8>, &'static str> {
        unreachable!()
    }
    fn rein_kill(&self, _: &Path, _: crate::tmux_server::Server) -> Result<(), &'static str> {
        unreachable!()
    }
    fn dialog_start(&self) -> Option<std::path::PathBuf> {
        None
    }
}

#[test]
fn the_overlay_is_run_for_the_tabs_own_tty_and_never_for_a_closed_tab() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let d = Desktop::new(Box::new(OverlayHost {
        seen: Arc::clone(&seen),
        answer: Err("tmux is older than 3.2".into()),
        waits: false,
    }));
    let proj = project();
    let spec = ChildSpec {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "cat".into()],
        cwd: Some(proj.path().into()),
        env_remove: crate::local_host::ENV_REMOVED,
        env_set: crate::local_host::ENV_SET,
        tty_line: false,
    };
    let q: Arc<dyn Events> = Arc::new(Quiet(Mutex::new(None)));
    let tab = d
        .open_spec(&spec, proj.path(), Size { cols: 80, rows: 24 }, q)
        .unwrap();
    let c = Colours {
        bg: "#1e1e2e".into(),
        fg: "#cdd6f4".into(),
        muted: "#a6adc8".into(),
        border: "#6c7086".into(),
        accent: "#cba6f7".into(),
    };
    let out = d.tmux_overlay(tab.id, "app", &c);
    assert_eq!(out, Some(Err("tmux is older than 3.2".into())));
    let words = seen.lock().unwrap()[0].clone();
    assert_eq!(Some(words[0].clone()), d.host.tty(tab.pty).unwrap());
    assert_eq!(&words[1..3], ["app", "#1e1e2e"]);
    // A bad colour never reaches the host.
    let mut bad = c.clone();
    bad.bg = "#1e1e2e; kill-server".into();
    assert!(matches!(d.tmux_overlay(tab.id, "app", &bad), Some(Err(_))));
    assert_eq!(seen.lock().unwrap().len(), 1);
    d.detach(tab.id);
    assert_eq!(d.tmux_overlay(tab.id, "app", &c), None, "a closed tab");
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// E-56: while the overlay waits for the tab's tmux client (a fresh `rein start` attaches
/// late; a failed one never), the tab's terminal ending stops the run at once, and nothing
/// is recorded for a terminal that is gone.
#[test]
fn the_overlay_stops_waiting_when_the_tabs_terminal_ends() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let d = Desktop::new(Box::new(OverlayHost {
        seen: Arc::clone(&seen),
        answer: Err("the tab's tmux client was not attached within 60 seconds".into()),
        waits: true,
    }));
    let proj = project();
    let spec = ChildSpec {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "sleep 0.3".into()],
        cwd: Some(proj.path().into()),
        env_remove: crate::local_host::ENV_REMOVED,
        env_set: crate::local_host::ENV_SET,
        tty_line: false,
    };
    let (tx, rx) = mpsc::channel();
    let q: Arc<dyn Events> = Arc::new(Quiet(Mutex::new(Some(tx))));
    let tab = d
        .open_spec(&spec, proj.path(), Size { cols: 80, rows: 24 }, q)
        .unwrap();
    let c = Colours {
        bg: "#1e1e2e".into(),
        fg: "#cdd6f4".into(),
        muted: "#a6adc8".into(),
        border: "#6c7086".into(),
        accent: "#cba6f7".into(),
    };
    let t0 = std::time::Instant::now();
    assert_eq!(
        d.tmux_overlay(tab.id, "hidden", &c),
        None,
        "nothing to report"
    );
    assert!(
        t0.elapsed() < Duration::from_secs(3),
        "{:?}: waited past the exit",
        t0.elapsed()
    );
    assert_eq!(seen.lock().unwrap().len(), 1, "the script was started");
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
}

#[test]
fn the_overlay_mode_follows_the_settings() {
    let t = tempfile::tempdir().unwrap();
    let ui = crate::ui_store::UiStore::open(Some(t.path().to_path_buf()));
    let cfg = crate::config::ConfigService::open(Some(t.path().to_path_buf())).view();
    assert_eq!(overlay_mode(&ui.view(&cfg)), "hidden");
    ui.set(&cfg, "tmux_status", &serde_json::Value::from("keep"))
        .unwrap();
    assert_eq!(overlay_mode(&ui.view(&cfg)), "keep");
    ui.set(&cfg, "tmux_overlay", &serde_json::Value::from(false))
        .unwrap();
    assert_eq!(overlay_mode(&ui.view(&cfg)), "off");
}

#[test]
fn the_command_table_is_the_one_the_capability_grants() {
    let cap: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/main.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let granted: Vec<String> = cap["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| {
            p.as_str()?
                .strip_prefix("allow-")
                .map(|s| s.replace('-', "_"))
        })
        .collect();
    let mut want: Vec<String> = commands::COMMANDS.iter().map(|s| s.to_string()).collect();
    let mut got = granted.clone();
    want.sort();
    got.sort();
    assert_eq!(got, want);
}
