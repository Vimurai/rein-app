//! One status per open project (status.md §Components 2, §Data Model `ProjectStatus`, D-062;
//! E-76): a pure function of what the core observes, first match wins. The web side draws the
//! state it is sent and decides nothing (E-77).

use serde::Serialize;

use crate::activity::WORK_GAP_S;
use crate::agents::Agent;
use crate::attention::{Item, handoff_seconds};
use crate::panel_attention::{ActiveTask, Attention, InProgress, TaskCounts, WORD};

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

/// `ProjectStatus` as the `project_status` event and `project_status_all()` send it.
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
    /// The first task in progress (D-062 addendum 4, E-95); null without one or without a
    /// readable `state.json`.
    pub in_progress: Option<InProgress>,
    /// Whose turn it is: the last handoff's "to"; null without a readable `COMM.md`.
    pub turn: Option<String>,
    /// Seconds since the last handoff's time; null when its time is not in the written shape
    /// or lies ahead of the clock.
    pub last_handoff_s: Option<u32>,
    /// The Home card's fields (status.md §Home H3 and §Data model `ProjectCard`, E-80): the
    /// task counts (null: tasks not available), the last handoff's trigger, and the agents
    /// by role from the status script.
    pub counts: Option<TaskCounts>,
    pub trigger: Option<String>,
    pub agents: Vec<Agent>,
    /// The active tasks the card lists (status.md "Home, second revision" R4, D-071): at most
    /// three; empty when `state.json` was not read.
    pub active: Vec<ActiveTask>,
}

/// The Home card's task counts, trigger and active tasks from one tab's facts (none: null or
/// empty, never a guess).
pub fn home_fields(f: Option<&Attention>) -> (Option<TaskCounts>, Option<String>, Vec<ActiveTask>) {
    f.map_or((None, None, Vec::new()), |f| {
        (f.counts, f.trigger.clone(), f.active.clone())
    })
}

/// The agents as a card shows them: each role cut and cleaned like every other project text.
pub fn card_agents(agents: &[Agent]) -> Vec<Agent> {
    agents
        .iter()
        .map(|a| Agent {
            role: crate::panel_sources::shown(&a.role, WORD, false),
            state: a.state,
        })
        .collect()
}

/// The three fields from one tab's facts, at `now` (seconds since the epoch).
pub fn card_fields(
    f: Option<&Attention>,
    now: u64,
) -> (Option<InProgress>, Option<String>, Option<u32>) {
    let Some(f) = f else {
        return (None, None, None);
    };
    let age = f
        .last_handoff
        .as_deref()
        .and_then(handoff_seconds)
        .and_then(|t| now.checked_sub(t))
        .map(|s| u32::try_from(s).unwrap_or(u32::MAX));
    (f.in_progress_task.clone(), f.turn.clone(), age)
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
            || self.in_progress != next.in_progress
            || self.turn != next.turn
            || self.last_handoff_s.map(|s| s / 60) != next.last_handoff_s.map(|s| s / 60)
            || self.counts != next.counts
            || self.trigger != next.trigger
            || self.agents != next.agents
            || self.active != next.active
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
            in_progress: None,
            turn: None,
            last_handoff_s: None,
            counts: None,
            trigger: None,
            agents: vec![],
            active: vec![],
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
        // E-95: the card fields — the task and the turn on any change, the handoff's age by
        // the minute.
        let with = |f: &dyn Fn(&mut ProjectStatus)| {
            let mut x = s(State::Quiet, Some(20));
            f(&mut x);
            x
        };
        let base = s(State::Quiet, Some(20));
        assert!(base.differs(&with(&|x| x.turn = Some("Architect".into()))));
        assert!(base.differs(&with(&|x| {
            x.in_progress = Some(InProgress {
                id: "E-1".into(),
                title: "t".into(),
            })
        })));
        assert!(base.differs(&with(&|x| {
            x.active = vec![ActiveTask {
                id: "E-2".into(),
                title: "t".into(),
                status: crate::panel_attention::ActiveStatus::Open,
            }]
        })));
        assert!(
            !with(&|x| x.last_handoff_s = Some(61))
                .differs(&with(&|x| x.last_handoff_s = Some(119)))
        );
        assert!(
            with(&|x| x.last_handoff_s = Some(119))
                .differs(&with(&|x| x.last_handoff_s = Some(120)))
        );
        let shape = serde_json::to_value(s(State::NeedsYou, None)).unwrap();
        assert_eq!(
            shape,
            serde_json::json!({"tab": 1, "state": "needs_you", "quiet_s": null, "worked_s": 30,
                               "waiting": null, "now": [], "info": [], "in_progress": null,
                               "turn": null, "last_handoff_s": null,
                               "counts": null, "trigger": null, "agents": [], "active": []})
        );
    }

    /// E-95: the card fields from one tab's facts, with the clock passed in.
    #[test]
    fn the_card_fields_come_from_the_facts_and_are_not_guessed() {
        const NOW: u64 = 1_763_380_800; // 2025-11-17 12:00 UTC
        let facts = Attention {
            tasks_read: true,
            blocked: vec![],
            in_progress: true,
            last_handoff: Some("2025-11-17 10:00 UTC".into()),
            mentions: vec![],
            in_progress_task: Some(InProgress {
                id: "E-77".into(),
                title: "Show each project's status.".into(),
            }),
            turn: Some("Engineer".into()),
            counts: None,
            trigger: None,
            active: vec![],
        };
        assert_eq!(
            card_fields(Some(&facts), NOW),
            (
                facts.in_progress_task.clone(),
                Some("Engineer".into()),
                Some(7200)
            )
        );
        assert_eq!(card_fields(None, NOW), (None, None, None), "no facts");
        let at = |ts: &str| {
            let f = Attention {
                last_handoff: Some(ts.into()),
                ..facts.clone()
            };
            card_fields(Some(&f), NOW).2
        };
        assert_eq!(at("2025-11-17 13:00 UTC"), None, "ahead of the clock");
        assert_eq!(at("yesterday"), None, "not in the written shape");
        assert_eq!(at("2025-11-17 12:00 UTC"), Some(0));
    }
}
