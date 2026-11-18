//! The status watch (status.md §Components 1–2, §API, §Execution Constraints, D-062; E-76):
//! the core asks every open tab's agents and activity in one status run, every 5 s while the
//! window can be seen and every 15 s while it is hidden or minimised (a hidden web view's
//! timers are slowed or stopped, so this lives in the core), computes each tab's status and
//! sends `project_status` for a tab whose status changed. With no tab open nothing runs.
//!
//! The app's own events (a bell, the current tab, focus) recompute the statuses at once with
//! no status run.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::agents::AgentState;
use crate::attention::{AttentionState, Lists, TabIn};
use crate::config::ConfigService;
use crate::ipc::Desktop;
use crate::panel_attention::Attention;
use crate::status::{Facts, ProjectStatus, State as Status, card_fields, state};

/// The poll while the window can be seen (V9: one short process per 5 s).
pub const VISIBLE_EVERY: Duration = Duration::from_secs(5);
/// The poll while the window is hidden or minimised.
pub const HIDDEN_EVERY: Duration = Duration::from_secs(15);
/// The longest a tab whose tmux client has not been seen reads as `starting` (V7: 20 s).
pub const START_LIMIT: Duration = Duration::from_secs(20);

/// `[notifications] stuck_after_min` in seconds.
pub fn stuck_after_s(config: &ConfigService) -> u32 {
    config
        .view()
        .config
        .notifications
        .stuck_after_min
        .saturating_mul(60)
}

/// The statuses last sent, when each tab was first seen, and the wake-up line of the poll.
pub struct StatusWatch {
    sent: Mutex<HashMap<u32, ProjectStatus>>,
    first_seen: Mutex<HashMap<u32, Instant>>,
    kick: Mutex<Sender<()>>,
    woken: Mutex<Option<Receiver<()>>>,
}

impl Default for StatusWatch {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            sent: Mutex::new(HashMap::new()),
            first_seen: Mutex::new(HashMap::new()),
            kick: Mutex::new(tx),
            woken: Mutex::new(Some(rx)),
        }
    }
}

impl StatusWatch {
    /// Recompute now (a bell, the current tab, focus): no status run.
    pub fn kick(&self) {
        let _ = self.kick.lock().unwrap_or_else(|e| e.into_inner()).send(());
    }

    /// The statuses that changed since they were last sent; they count as sent. A tab that is
    /// gone is forgotten.
    pub fn changed(&self, all: Vec<ProjectStatus>) -> Vec<ProjectStatus> {
        let mut sent = self.sent.lock().unwrap_or_else(|e| e.into_inner());
        sent.retain(|tab, _| all.iter().any(|s| s.tab == *tab));
        all.into_iter()
            .filter(|s| {
                let new = sent.get(&s.tab).is_none_or(|old| old.differs(s));
                if new {
                    sent.insert(s.tab, s.clone());
                }
                new
            })
            .collect()
    }

    /// How long ago the watch first saw `tab` (close to when it opened).
    fn age(&self, tab: u32, now: Instant) -> Duration {
        let mut f = self.first_seen.lock().unwrap_or_else(|e| e.into_inner());
        now.duration_since(*f.entry(tab).or_insert(now))
    }

    fn forget_gone(&self, open: &[u32]) {
        self.first_seen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|t, _| open.contains(t));
    }
}

/// The items of `lists` that are about `tab`.
fn items_of(lists: &Lists, tab: u32) -> (Vec<crate::attention::Item>, Vec<crate::attention::Item>) {
    let of =
        |v: &[crate::attention::Item]| v.iter().filter(|i| i.tab == Some(tab)).cloned().collect();
    (of(&lists.now), of(&lists.info))
}

/// A `setup` status: no activity, no items; the card fields when the core has them.
fn setup_status(tab: u32, facts: Option<&Attention>, now_s: u64) -> ProjectStatus {
    let (in_progress, turn, last_handoff_s) = card_fields(facts, now_s);
    ProjectStatus {
        tab,
        state: Status::Setup,
        quiet_s: None,
        worked_s: 0,
        waiting: None,
        now: vec![],
        info: vec![],
        in_progress,
        turn,
        last_handoff_s,
    }
}

/// The tabs the web side reported as needing set-up (or refused) that have no status yet:
/// not running, not a set-up tab, and known to the core (D-062 addendum 4, E-95).
pub fn reported_setup(sent: &[TabIn], open: &[u32], known: &dyn Fn(u32) -> bool) -> Vec<u32> {
    let mut v: Vec<u32> = sent
        .iter()
        .filter(|t| t.setup || t.refused)
        .filter_map(|t| t.tab)
        .filter(|tab| !open.contains(tab) && known(*tab))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Every open tab's status from what the core holds now (no status run). `now_s`: the wall
/// clock in seconds, for the last handoff's age.
pub fn statuses(
    d: &Desktop,
    a: &AttentionState,
    w: &StatusWatch,
    stuck_after_s: u32,
    now: Instant,
    now_s: u64,
) -> Vec<ProjectStatus> {
    let (lists, facts) = crate::attention::answer_all(d, a, stuck_after_s);
    let running = d.running_tabs();
    let mut open: Vec<u32> = running.iter().map(|t| t.0).collect();
    let mut out: Vec<ProjectStatus> = running
        .into_iter()
        .map(|(tab, pty, _)| {
            let seen = d.agents.seen(tab);
            let alive = d.host.alive(pty);
            let running = seen.agents.iter().any(|x| x.state == AgentState::Running);
            let (now_items, info) = items_of(&lists, tab);
            let f = Facts {
                setup: a.reported.refused(tab) || a.reported.setup(tab),
                start_ended: !alive,
                agents_ended: !running && seen.agents.iter().any(|x| x.state == AgentState::Ended),
                starting: alive && !seen.client_ever && w.age(tab, now) < START_LIMIT,
                now_items: now_items.len(),
                agent_running: running,
                quiet_s: seen.activity.quiet_s,
            };
            let (in_progress, turn, last_handoff_s) = card_fields(facts.get(&tab), now_s);
            ProjectStatus {
                tab,
                state: state(&f),
                quiet_s: seen.activity.quiet_s,
                worked_s: seen.activity.worked_s,
                waiting: None,
                now: now_items,
                info,
                in_progress,
                turn,
                last_handoff_s,
            }
        })
        .collect();
    // Set-up tabs have no process: the folder is not set up.
    for tab in d.setup_tabs() {
        open.push(tab);
        out.push(setup_status(tab, None, now_s));
    }
    // Tabs the web side says need set-up and the core has no status for.
    let (sent, _) = a.reported.last_input();
    for tab in reported_setup(&sent, &open, &|t| d.tab_project(t).is_some()) {
        open.push(tab);
        out.push(setup_status(tab, facts.get(&tab), now_s));
    }
    w.forget_gone(&open);
    a.reported.retain(&open);
    out.sort_by_key(|s| s.tab);
    out
}

/// Seconds since the epoch.
fn wall_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn hidden(app: &AppHandle) -> bool {
    app.get_webview_window(crate::ipc::MAIN_WINDOW)
        .is_none_or(|w| !w.is_visible().unwrap_or(true) || w.is_minimized().unwrap_or(false))
}

/// The poll: one status run per period, statuses sent on change; a kick recomputes at once.
pub fn spawn(app: AppHandle) {
    let Some(woken) = app
        .state::<StatusWatch>()
        .woken
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    else {
        return;
    };
    std::thread::spawn(move || {
        let mut ran: Option<Instant> = None;
        loop {
            let every = if hidden(&app) {
                HIDDEN_EVERY
            } else {
                VISIBLE_EVERY
            };
            let d = app.state::<Desktop>();
            if ran.is_none_or(|t| t.elapsed() >= every) {
                ran = Some(Instant::now());
                // A failed run leaves the last answers; the next period asks again.
                let _ = crate::agents::poll_all(&d);
            }
            let stuck = stuck_after_s(&app.state::<std::sync::Arc<ConfigService>>());
            let w = app.state::<StatusWatch>();
            let all = statuses(
                &d,
                &app.state::<AttentionState>(),
                &w,
                stuck,
                Instant::now(),
                wall_s(),
            );
            for s in w.changed(all) {
                let _ = app.emit("project_status", s);
            }
            let wait = ran.map_or(every, |t| every.saturating_sub(t.elapsed()));
            match woken.recv_timeout(wait.max(Duration::from_millis(100))) {
                // A burst of bells is one recompute.
                Ok(()) => while woken.try_recv().is_ok() {},
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });
}

/// `project_status_all()`: every open tab's status, for a web view that has just loaded.
#[tauri::command(async)]
pub fn project_status_all(
    state: State<'_, Desktop>,
    attention: State<'_, AttentionState>,
    watch: State<'_, StatusWatch>,
    config: State<'_, std::sync::Arc<ConfigService>>,
) -> Vec<ProjectStatus> {
    statuses(
        &state,
        &attention,
        &watch,
        stuck_after_s(&config),
        Instant::now(),
        wall_s(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(tab: u32, state: Status, q: Option<u32>) -> ProjectStatus {
        ProjectStatus {
            tab,
            state,
            quiet_s: q,
            worked_s: 0,
            waiting: None,
            now: vec![],
            info: vec![],
            in_progress: None,
            turn: None,
            last_handoff_s: None,
        }
    }

    #[test]
    fn a_status_is_sent_on_change_only_and_a_closed_tab_is_forgotten() {
        let w = StatusWatch::default();
        let first = w.changed(vec![
            st(1, Status::Working, Some(1)),
            st(2, Status::Starting, None),
        ]);
        assert_eq!(first.len(), 2, "the first status of each tab is sent");
        assert!(
            w.changed(vec![
                st(1, Status::Working, Some(3)),
                st(2, Status::Starting, None)
            ])
            .is_empty()
        );
        let next = w.changed(vec![
            st(1, Status::Quiet, Some(16)),
            st(2, Status::Starting, None),
        ]);
        assert_eq!(next.iter().map(|s| s.tab).collect::<Vec<_>>(), [1]);
        // Tab 2 closed, then a tab 2 again would be new.
        assert!(w.changed(vec![st(1, Status::Quiet, Some(20))]).is_empty());
        assert_eq!(
            w.changed(vec![
                st(1, Status::Quiet, Some(20)),
                st(2, Status::Unknown, None)
            ])
            .len(),
            1
        );
    }

    fn tab_in(tab: Option<u32>, refused: bool, setup: bool) -> TabIn {
        TabIn {
            index: 0,
            tab,
            refused,
            update: false,
            setup,
        }
    }

    /// E-95 (D-062 addendum 4): a reported tab without a pty and with the setup flag (or
    /// refused) gets `setup`, once, if the core knows it; a running tab is not counted twice.
    #[test]
    fn a_reported_tab_without_a_pty_that_needs_set_up_is_setup() {
        let sent = [
            tab_in(Some(7), false, true),
            tab_in(Some(8), true, false),
            tab_in(Some(9), false, false),
            tab_in(Some(3), false, true),
            tab_in(Some(11), false, true),
            tab_in(None, false, true),
            tab_in(Some(7), false, true),
        ];
        let known = |t: u32| t != 11;
        assert_eq!(reported_setup(&sent, &[3], &known), [7, 8]);
        let s = setup_status(7, None, 0);
        assert_eq!((s.state, s.quiet_s, s.now.len()), (Status::Setup, None, 0));
    }

    #[cfg(unix)]
    mod desktop {
        use super::*;
        use crate::attention::Health;
        use crate::dismissed::Dismissed;
        use crate::local_host::LocalHost;
        use crate::pty::{ChildSpec, Events, PtyId, Size};
        use std::sync::Arc;

        struct Quiet;
        impl Events for Quiet {
            fn output(&self, _: PtyId, _: Vec<u8>) {}
            fn exit(&self, _: PtyId, _: u32) {}
        }

        fn tab_on(d: &Desktop, project: &std::path::Path) -> u32 {
            let spec = ChildSpec {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "sleep 5".into()],
                cwd: Some(project.into()),
                env_remove: crate::local_host::ENV_REMOVED,
                env_set: crate::local_host::ENV_SET,
                tty_line: false,
            };
            d.open_spec(&spec, project, Size { cols: 80, rows: 24 }, Arc::new(Quiet))
                .unwrap()
                .id
        }

        const STATE: &str = r#"{"tasks":[{"id":"E-76","status":"DONE"},
            {"id":"E-77","status":"IN_PROGRESS","description":"Show each project's status."}]}"#;
        const COMM: &str = "## Handoff \u{2014} 2025-11-17 10:00 UTC\n**From**: Architect \u{2192} Engineer\n**Trigger**: t\n---\n";
        const NOW: u64 = 1_763_380_800; // 2025-11-17 12:00 UTC

        fn project(state: Option<&[u8]>, comm: bool) -> tempfile::TempDir {
            let p = tempfile::tempdir().unwrap();
            std::fs::create_dir(p.path().join(".ai")).unwrap();
            if let Some(s) = state {
                std::fs::write(p.path().join(".ai/state.json"), s).unwrap();
            }
            if comm {
                std::fs::write(p.path().join(".ai/COMM.md"), COMM).unwrap();
            }
            p
        }

        /// E-95: the three fields from the project's own files through the panel sources;
        /// null when a file is missing or over its limit; the setup flag on a running tab.
        #[test]
        fn the_card_fields_are_filled_from_the_files_or_null() {
            let d = Desktop::new(Box::new(LocalHost::new(None)));
            let a = AttentionState::new(Dismissed::open(None));
            let w = StatusWatch::default();
            let full = project(Some(STATE.as_bytes()), true);
            let none = project(None, false);
            let big = vec![b' '; usize::try_from(crate::project_file::STATE_MAX).unwrap() + 1];
            let over = project(Some(&big), true);
            let (t_full, t_none, t_over) = (
                tab_on(&d, full.path()),
                tab_on(&d, none.path()),
                tab_on(&d, over.path()),
            );
            let all = statuses(&d, &a, &w, 600, Instant::now(), NOW);
            let of = |t: u32| all.iter().find(|s| s.tab == t).unwrap().clone();
            let f = of(t_full);
            assert_eq!(
                f.in_progress,
                Some(crate::panel_attention::InProgress {
                    id: "E-77".into(),
                    title: "Show each project's status.".into()
                })
            );
            assert_eq!(f.turn.as_deref(), Some("Engineer"));
            assert_eq!(f.last_handoff_s, Some(7200));
            let n = of(t_none);
            assert_eq!(
                (n.in_progress, n.turn, n.last_handoff_s),
                (None, None, None)
            );
            let o = of(t_over);
            assert_eq!(o.in_progress, None, "state.json over its limit");
            assert_eq!(o.turn.as_deref(), Some("Engineer"), "COMM.md still read");
            // The setup flag (a failed start that needs rein init, D-068) makes it `setup`.
            assert_ne!(f.state, Status::Setup);
            a.reported
                .input(vec![tab_in(Some(t_full), false, true)], Health::Unknown);
            let again = statuses(&d, &a, &w, 600, Instant::now(), NOW);
            let f = again.iter().find(|s| s.tab == t_full).unwrap();
            assert_eq!(f.state, Status::Setup);
            assert_eq!(f.turn.as_deref(), Some("Engineer"), "the fields stay");
            for t in [t_full, t_none, t_over] {
                d.detach(t);
            }
        }
    }
}
