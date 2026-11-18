//! Closing a project and quitting (desktop.md §Look and feel v2 V11 component 1, V12
//! component 8; D-059, D-061). Closing a started tab on the app's own tmux server closes its
//! project (V9's close). Quitting is not an event for the projects (V12): no confirm, nothing
//! killed — the ptys end, tmux detaches the app's clients and the sessions keep running, to
//! be attached to again (`ipc.rs`, the live lookup). Nothing here touches the user's own
//! server: a tab on it closes as before, its session running.

use tauri::{AppHandle, State};

use crate::agents::Close;
use crate::ipc::{Desktop, IpcError};
use crate::tmux_server::Server;

/// Close `tab` (V11 component 1). On the app's server: V9's close — `{confirm: roles}` when an
/// agent is running or not known and `confirm_running` is false, else `rein start --kill
/// --yes`, then the tab detaches. On the user's server, or for a set-up tab: it detaches.
/// The tab goes even when the kill failed (that session keeps running and is attached to
/// again when the project is next opened); the failure is the answer's error.
pub fn close_tab(d: &Desktop, tab: u32, confirm_running: bool) -> Result<Close, IpcError> {
    let on_app = d
        .tab_target(tab)
        .is_some_and(|(_, server, _)| server == Server::App);
    if on_app {
        match crate::agents::close(d, tab, confirm_running) {
            Ok(Close::Confirm { confirm }) => return Ok(Close::Confirm { confirm }),
            Ok(Close::Closed { .. }) => {}
            Err(e) => {
                d.detach(tab);
                return Err(e);
            }
        }
    }
    d.detach(tab);
    Ok(Close::Closed { closed: true })
}

#[tauri::command(async)]
pub fn tab_close(
    state: State<'_, Desktop>,
    tab: u32,
    confirm_running: bool,
) -> Result<Close, IpcError> {
    close_tab(&state, tab, confirm_running)
}

/// `app_quit()` (V12 component 8): every tab detached, every pty child reaped, then the app
/// exits. No confirm, no `kill-server`: the sessions outlive the app.
#[tauri::command(async)]
pub fn app_quit(app: AppHandle, state: State<'_, Desktop>) {
    state.detach_all();
    app.exit(0);
}

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use super::*;
    use crate::pty::{ChildSpec, Events, PtyId};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    /// Records the lifecycle calls; every child is a short `sleep`. The lookup answers `live`
    /// for the tags in `.1`.
    #[derive(Default)]
    pub(crate) struct Rec(pub Arc<Mutex<Vec<String>>>, pub Vec<String>);

    fn sleeper(project: &Path) -> ChildSpec {
        ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 30".into()],
            cwd: Some(project.into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        }
    }

    impl crate::host::Host for Rec {
        fn project_dir(&self, input: &str) -> Result<(PathBuf, crate::host::Found), IpcError> {
            if input.starts_with("/gone/") {
                return Err(crate::ipc::err("not_a_directory", "not a directory"));
            }
            Ok((PathBuf::from(input), crate::host::Found::Project))
        }
        fn rein_start(&self, project: &Path, s: Server) -> Result<ChildSpec, IpcError> {
            self.0
                .lock()
                .unwrap()
                .push(format!("start {} {}", project.display(), s.word()));
            Ok(sleeper(project))
        }
        fn rein_setup(&self, _: &Path, _: Server) -> Result<ChildSpec, IpcError> {
            unreachable!()
        }
        fn rein_command(
            &self,
            _: &Path,
            _: crate::project_setup::ReinRun,
        ) -> Result<std::process::Command, IpcError> {
            unreachable!()
        }
        fn rein_team_add(
            &self,
            _: &Path,
            _: &crate::team::Add,
        ) -> Result<std::process::Command, IpcError> {
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
        /// `.1`: the app server's sessions — a tag (a tagged session) or `@<path>` (an
        /// untagged session started in that folder, E-90).
        fn tmux_setup(
            &self,
            tag: &str,
        ) -> Result<Vec<crate::tmux_server::SessionRow>, &'static str> {
            let live = self.1.iter().any(|t| t == tag);
            self.0
                .lock()
                .unwrap()
                .push(format!("setup {}", if live { "live" } else { "none" }));
            Ok(self
                .1
                .iter()
                .enumerate()
                .map(|(i, t)| crate::tmux_server::SessionRow {
                    id: format!("${}", i + 1),
                    tag: if t.starts_with('@') {
                        String::new()
                    } else {
                        t.clone()
                    },
                    path: t.strip_prefix('@').unwrap_or("/tagged").to_string(),
                })
                .collect())
        }
        fn rein_attach(
            &self,
            project: &Path,
            tag: &str,
            adopt: Option<&str>,
        ) -> Result<ChildSpec, IpcError> {
            assert_eq!(tag, crate::session_tag::tag(None, project));
            self.0.lock().unwrap().push(match adopt {
                Some(id) => format!("attach {} {id}", project.display()),
                None => format!("attach {}", project.display()),
            });
            Ok(sleeper(project))
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
            // The status is not known: the close asks.
            Err(crate::agents::NO_CLIENT)
        }
        fn rein_kill(&self, project: &Path, server: Server) -> Result<(), &'static str> {
            self.0
                .lock()
                .unwrap()
                .push(format!("kill {} {}", project.display(), server.word()));
            Ok(())
        }
        fn dialog_start(&self) -> Option<PathBuf> {
            None
        }
    }

    pub(crate) struct Quiet;
    impl Events for Quiet {
        fn output(&self, _: PtyId, _: Vec<u8>) {}
        fn exit(&self, _: PtyId, _: u32) {}
    }

    fn opened(o: crate::project_setup::Opened) -> crate::ipc::Tab {
        match o {
            crate::project_setup::Opened::Tab(t) => t,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_live_session_is_attached_to_and_quit_kills_nothing() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let live = crate::session_tag::tag(None, Path::new("/tmp/b"));
        let d = Desktop::new(Box::new(Rec(Arc::clone(&log), vec![live])));
        let take = || std::mem::take(&mut *log.lock().unwrap());
        let open = |p: &str, s: Server| opened(d.open(p, 80, 24, Arc::new(Quiet), s).unwrap());

        // The user's server: no setup, no lookup, no kill on close.
        let u = open("/tmp/user-proj", Server::User);
        assert_eq!(take(), ["start /tmp/user-proj user"]);
        assert_eq!(
            close_tab(&d, u.id, false).unwrap(),
            Close::Closed { closed: true }
        );
        assert_eq!(take(), Vec::<String>::new());

        // The app's server: no session tagged for /tmp/a → rein start; /tmp/b's session is
        // live → attached to, and rein start is never run on it (V12 components 2-3).
        let a = open("/tmp/a", Server::App);
        assert_eq!(take(), ["setup none", "start /tmp/a app"]);
        let b = open("/tmp/b", Server::App);
        assert_eq!(take(), ["setup live", "attach /tmp/b"]);

        // Tab close with the agents' status not known: asks, kills nothing, keeps the tab.
        assert_eq!(
            close_tab(&d, a.id, false).unwrap(),
            Close::Confirm { confirm: vec![] }
        );
        assert!(d.tab_project(a.id).is_some());
        assert_eq!(take(), Vec::<String>::new());
        // Confirmed: that project only, on the app's server; the other tab stays.
        assert_eq!(
            close_tab(&d, a.id, true).unwrap(),
            Close::Closed { closed: true }
        );
        assert_eq!(take(), ["kill /tmp/a app"]);
        assert!(d.tab_project(a.id).is_none() && d.tab_project(b.id).is_some());

        // Quit (V12 component 8): the tabs detach, nothing is asked or killed.
        d.detach_all();
        assert_eq!(take(), Vec::<String>::new());
    }

    /// V12 "Security": with the sweep and the quit kill gone, no script the app runs holds
    /// `kill-server` — every `.sh` beside the source and every script built in Rust.
    #[test]
    fn no_script_holds_kill_server() {
        fn shell_files(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    shell_files(&p, out);
                } else if p.extension().is_some_and(|x| x == "sh") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        shell_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        assert!(files.len() >= 5, "{files:?}");
        let mut scripts: Vec<(String, String)> = files
            .iter()
            .map(|f| (f.display().to_string(), std::fs::read_to_string(f).unwrap()))
            .collect();
        for (name, text) in [
            ("setup", crate::tmux_server::setup_script()),
            ("start", crate::tmux_server::start_script().to_string()),
            ("start wsl", crate::tmux_server::start_script_wsl()),
            ("attach", crate::tmux_server::attach_script()),
            ("attach wsl", crate::tmux_server::attach_script_wsl()),
            ("kill", crate::tmux_server::kill_script()),
        ] {
            scripts.push((name.into(), text));
        }
        for (name, text) in scripts {
            assert!(!text.contains("kill-server"), "{name} holds kill-server");
        }
    }
}
