//! A project on the app's own tmux server lives only while its tab does (desktop.md §Look and
//! feel v2 V11, D-059, E-66). Closing a started tab closes its project (V9's close); quitting
//! ends the app's server with one constant `tmux -L rein-app kill-server`; the first tab of a
//! run sweeps a server left from an earlier one (in the V8 setup process, `ipc.rs`). Nothing
//! here touches the user's own server: a tab on it closes as before, its session running.

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::agents::Close;
use crate::ipc::{Desktop, IpcError};
use crate::tmux_server::Server;

/// Told to the web side when a native close was held for its confirm.
pub const EVENT_QUIT_REQUESTED: &str = "quit_requested";

/// Close `tab` (V11 component 1). On the app's server: V9's close — `{confirm: roles}` when an
/// agent is running or not known and `confirm_running` is false, else `rein start --kill
/// --yes`, then the tab detaches. On the user's server, or for a set-up tab: it detaches.
/// The tab goes even when the kill failed (the next run's sweep ends that session); the
/// failure is the answer's error.
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

/// `app_quit`'s answer when it does not exit.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct QuitConfirm {
    /// How many projects run on the app's server.
    pub confirm: usize,
}

/// End the app's server once for this quit (V11 component 2): at most 3 s, whatever the
/// answer. Only when a tab runs on it — with `tmux.server = "user"` nothing is touched.
pub fn end_server(d: &Desktop) {
    if d.app_tabs() > 0 && !d.server_ended.swap(true, Ordering::AcqRel) {
        let _ = d.platform.tmux_kill_server();
    }
}

/// Quit: `{confirm: n}` when `n` projects run on the app's server and the operator has not
/// confirmed; else the server is ended and the app exits.
pub fn quit_answer(d: &Desktop, confirm_running: bool) -> Option<QuitConfirm> {
    let n = d.app_tabs();
    (n > 0 && !confirm_running).then_some(QuitConfirm { confirm: n })
}

#[tauri::command(async)]
pub fn tab_close(
    state: State<'_, Desktop>,
    tab: u32,
    confirm_running: bool,
) -> Result<Close, IpcError> {
    close_tab(&state, tab, confirm_running)
}

/// `app_quit(confirm_running)` → `{confirm: n}`, or the app exits (V11 "Interface contracts").
#[tauri::command(async)]
pub fn app_quit(
    app: AppHandle,
    state: State<'_, Desktop>,
    confirm_running: bool,
) -> Option<QuitConfirm> {
    if let Some(ask) = quit_answer(&state, confirm_running) {
        return Some(ask);
    }
    end_server(&state);
    state.detach_all();
    app.exit(0);
    None
}

/// A native close (title bar, window manager): with projects on the app's server, the first
/// one is held and the web side asks (`quit_requested`); a second one while that is pending
/// goes through, so a stuck page cannot keep the window open. `true`: let it close.
pub fn on_close_requested(app: &AppHandle, d: &Desktop) -> bool {
    if d.app_tabs() == 0 || d.close_held.swap(true, Ordering::AcqRel) {
        return true;
    }
    let _ = app.emit(EVENT_QUIT_REQUESTED, ());
    false
}

/// The web side answered the held close with Cancel: the next native close is held again.
#[tauri::command(async)]
pub fn quit_cancelled(state: State<'_, Desktop>) {
    state.close_held.store(false, Ordering::Release);
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::pty::{ChildSpec, Events, PtyId};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    /// Records the lifecycle calls; every child is a short `sleep`.
    #[derive(Default)]
    struct Rec(Arc<Mutex<Vec<String>>>);

    impl crate::host::Host for Rec {
        fn project_dir(&self, input: &str) -> Result<(PathBuf, crate::host::Found), IpcError> {
            Ok((PathBuf::from(input), crate::host::Found::Project))
        }
        fn rein_start(&self, project: &Path, _: Server) -> Result<ChildSpec, IpcError> {
            Ok(ChildSpec {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "sleep 30".into()],
                cwd: Some(project.into()),
                env_remove: crate::local_host::ENV_REMOVED,
                env_set: crate::local_host::ENV_SET,
                tty_line: false,
            })
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
        fn read_project_file(
            &self,
            _: &Path,
            _: crate::project_file::ProjectFile,
            _: bool,
        ) -> Result<crate::project_file::FileBytes, crate::project_file::Unavailable> {
            unreachable!()
        }
        fn tmux_setup(&self, sweep: bool) -> Result<(), &'static str> {
            self.0.lock().unwrap().push(format!("setup sweep={sweep}"));
            Ok(())
        }
        fn tmux_kill_server(&self) -> bool {
            self.0.lock().unwrap().push("kill-server".into());
            true
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

    struct Quiet;
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
    fn the_app_server_lives_with_its_tabs_and_the_users_is_never_touched() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let d = Desktop::new(Box::new(Rec(Arc::clone(&log))));
        let take = || std::mem::take(&mut *log.lock().unwrap());
        let open = |p: &str, s: Server| opened(d.open(p, 80, 24, Arc::new(Quiet), s).unwrap());

        // The user's server: no setup, no sweep, no kill on close, nothing at quit.
        let u = open("/tmp/user-proj", Server::User);
        assert_eq!(
            close_tab(&d, u.id, false).unwrap(),
            Close::Closed { closed: true }
        );
        assert_eq!(quit_answer(&d, false), None);
        end_server(&d);
        assert_eq!(take(), Vec::<String>::new());

        // The app's server: the first tab sweeps in its setup, the second does not.
        let a = open("/tmp/a", Server::App);
        let b = open("/tmp/b", Server::App);
        assert_eq!(take(), ["setup sweep=true", "setup sweep=false"]);

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

        // Quit: asks with the count, then ends the app's server once.
        assert_eq!(quit_answer(&d, false), Some(QuitConfirm { confirm: 1 }));
        assert_eq!(quit_answer(&d, true), None);
        end_server(&d);
        end_server(&d); // the exit after a confirmed quit: not again
        assert_eq!(take(), ["kill-server"]);
        d.detach_all();
    }

    /// The constants: `kill-server` only ever with the app's socket name.
    #[test]
    fn kill_server_names_the_apps_socket_only() {
        for script in [
            crate::tmux_server::KILL_SERVER,
            include_str!("tmux_setup.sh"),
        ] {
            let code: Vec<&str> = script
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect();
            let uses: Vec<&&str> = code.iter().filter(|l| l.contains("kill-server")).collect();
            assert!(!uses.is_empty());
            for l in uses {
                assert!(l.contains("tmux -L rein-app kill-server"), "{l}");
            }
            assert!(!code.iter().any(|l| l.contains("kill-session")));
        }
        assert_eq!(
            crate::tmux_server::QUIT_LIMIT,
            std::time::Duration::from_secs(3)
        );
    }
}
