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
use crate::attention::{AttentionState, Lists};
use crate::config::ConfigService;
use crate::ipc::Desktop;
use crate::status::{Facts, ProjectStatus, State as Status, state};

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

/// Every open tab's status from what the core holds now (no status run).
pub fn statuses(
    d: &Desktop,
    a: &AttentionState,
    w: &StatusWatch,
    stuck_after_s: u32,
    now: Instant,
) -> Vec<ProjectStatus> {
    let lists = crate::attention::answer_all(d, a, stuck_after_s);
    let running = d.running_tabs();
    let mut open: Vec<u32> = running.iter().map(|t| t.0).collect();
    let mut out: Vec<ProjectStatus> = running
        .into_iter()
        .map(|(tab, pty, _)| {
            let seen = d.agents.seen(tab);
            let alive = d.host.alive(pty);
            let running = seen.agents.iter().any(|x| x.state == AgentState::Running);
            let (now_items, info) = items_of(&lists, tab);
            let facts = Facts {
                setup: a.reported.refused(tab),
                start_ended: !alive,
                agents_ended: !running && seen.agents.iter().any(|x| x.state == AgentState::Ended),
                starting: alive && !seen.client_ever && w.age(tab, now) < START_LIMIT,
                now_items: now_items.len(),
                agent_running: running,
                quiet_s: seen.activity.quiet_s,
            };
            ProjectStatus {
                tab,
                state: state(&facts),
                quiet_s: seen.activity.quiet_s,
                worked_s: seen.activity.worked_s,
                waiting: None,
                now: now_items,
                info,
            }
        })
        .collect();
    // Set-up tabs have no process: the folder is not set up.
    for tab in d.setup_tabs() {
        open.push(tab);
        out.push(ProjectStatus {
            tab,
            state: Status::Setup,
            quiet_s: None,
            worked_s: 0,
            waiting: None,
            now: vec![],
            info: vec![],
        });
    }
    w.forget_gone(&open);
    a.reported.retain(&open);
    out.sort_by_key(|s| s.tab);
    out
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
}
