//! One status per open project (status.md §Components 2, §Data Model `ProjectStatus`, D-062;
//! E-76): a pure function of what the core observes, first match wins. The web side draws the
//! state it is sent and decides nothing (E-77).

use serde::Serialize;

use crate::activity::WORK_GAP_S;
use crate::attention::Item;

/// The closed list of states, in the order the table tries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Setup,
    Ended,
    Starting,
    NeedsYou,
    Working,
    Quiet,
    Unknown,
}

/// What the table reads about one tab.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// The folder is not set up (a set-up tab), or the tmux setup was refused.
    pub setup: bool,
    /// `rein start` ended in the tab (its process is gone).
    pub start_ended: bool,
    /// Every agent seen running in the tab has exited and none runs now.
    pub agents_ended: bool,
    /// The tab is opening: its process runs and its tmux client has not been seen yet.
    pub starting: bool,
    /// The tab's "Now" items (component 4).
    pub now_items: usize,
    /// An agent runs in the window the tab shows.
    pub agent_running: bool,
    /// Seconds since that window's last output; `None` when not read.
    pub quiet_s: Option<u32>,
}

/// The table (component 2). First match wins.
pub fn state(f: &Facts) -> State {
    if f.setup {
        State::Setup
    } else if f.start_ended || f.agents_ended {
        State::Ended
    } else if f.starting {
        State::Starting
    } else if f.now_items > 0 {
        State::NeedsYou
    } else if f.agent_running {
        match f.quiet_s {
            Some(q) if u64::from(q) < WORK_GAP_S => State::Working,
            Some(_) => State::Quiet,
            None => State::Unknown,
        }
    } else {
        State::Unknown
    }
}

/// `ProjectStatus` as the `project_status` event and `project_status_all()` send it. The
/// fields of the Home card (the task in progress, whose turn, the last handoff) come with the
/// Home task (E-80), from the summary source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectStatus {
    /// The tab's core id.
    pub tab: u32,
    pub state: State,
    pub quiet_s: Option<u32>,
    pub worked_s: u32,
    /// Reserved for rein's waiting signal (D-062 addendum 1): always null until then.
    pub waiting: Option<bool>,
    pub now: Vec<Item>,
    pub info: Vec<Item>,
}

impl ProjectStatus {
    /// Whether `next` is a change worth sending: anything but the quiet time counting up
    /// within the same step (15 s under a minute, then whole minutes) and the worked time
    /// within the same minute.
    pub fn differs(&self, next: &ProjectStatus) -> bool {
        let minute = |q: Option<u32>| q.map(|q| if q < 60 { q / 15 } else { 4 + q / 60 });
        self.state != next.state
            || minute(self.quiet_s) != minute(next.quiet_s)
            || self.worked_s / 60 != next.worked_s / 60
            || self.now != next.now
            || self.info != next.info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_table_every_row_and_its_order() {
        use State::*;
        let base = Facts::default();
        let with = |f: &dyn Fn(&mut Facts)| {
            let mut x = base.clone();
            f(&mut x);
            state(&x)
        };
        // Each row on its own.
        assert_eq!(with(&|f| f.setup = true), Setup);
        assert_eq!(with(&|f| f.start_ended = true), Ended);
        assert_eq!(with(&|f| f.agents_ended = true), Ended);
        assert_eq!(with(&|f| f.starting = true), Starting);
        assert_eq!(with(&|f| f.now_items = 2), NeedsYou);
        let agent = |q: Option<u32>| {
            with(&|f| {
                f.agent_running = true;
                f.quiet_s = q;
            })
        };
        assert_eq!(agent(Some(0)), Working);
        assert_eq!(agent(Some(14)), Working, "under 15 s");
        assert_eq!(agent(Some(15)), Quiet, "at 15 s");
        assert_eq!(agent(None), Unknown, "an agent, but no activity read");
        assert_eq!(state(&base), Unknown, "nothing can be told");
        assert_eq!(with(&|f| f.quiet_s = Some(0)), Unknown, "output, no agent");
        // The order: each row wins over every row below it.
        let all = Facts {
            setup: true,
            start_ended: true,
            agents_ended: true,
            starting: true,
            now_items: 1,
            agent_running: true,
            quiet_s: Some(0),
        };
        let mut f = all.clone();
        let mut seen = vec![state(&f)];
        f.setup = false;
        seen.push(state(&f));
        f.start_ended = false;
        seen.push(state(&f));
        f.agents_ended = false;
        seen.push(state(&f));
        f.starting = false;
        seen.push(state(&f));
        f.now_items = 0;
        seen.push(state(&f));
        f.quiet_s = Some(40);
        seen.push(state(&f));
        f.agent_running = false;
        seen.push(state(&f));
        assert_eq!(
            seen,
            [
                Setup, Ended, Ended, Starting, NeedsYou, Working, Quiet, Unknown
            ]
        );
    }

    #[test]
    fn a_change_is_sent_and_the_clock_counting_up_is_not() {
        let s = |state: State, q: Option<u32>| ProjectStatus {
            tab: 1,
            state,
            quiet_s: q,
            worked_s: 30,
            waiting: None,
            now: vec![],
            info: vec![],
        };
        assert!(!s(State::Quiet, Some(20)).differs(&s(State::Quiet, Some(25))));
        assert!(
            s(State::Quiet, Some(25)).differs(&s(State::Quiet, Some(30))),
            "15 s steps under a minute"
        );
        assert!(!s(State::Quiet, Some(130)).differs(&s(State::Quiet, Some(175))));
        assert!(
            s(State::Quiet, Some(175)).differs(&s(State::Quiet, Some(185))),
            "a new minute"
        );
        assert!(s(State::Working, Some(1)).differs(&s(State::Quiet, Some(1))));
        let shape = serde_json::to_value(s(State::NeedsYou, None)).unwrap();
        assert_eq!(
            shape,
            serde_json::json!({"tab": 1, "state": "needs_you", "quiet_s": null, "worked_s": 30,
                               "waiting": null, "now": [], "info": []})
        );
    }
}
