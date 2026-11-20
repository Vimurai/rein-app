//! Notifications (status.md §Components 6 channels 1 and 2, §Data Model `Notification`,
//! D-062; E-78): the five events, raised on a **change**, once, never again while the state
//! stays. Pure: the caller passes each tab's observations and the clock; nothing here reads
//! the time, a file or the window.
//!
//! The rules of component 6:
//! - nothing for the current tab of a focused window (the caller passes that tab as `front`);
//! - nothing in the first [`SILENCE`] after a tab is first seen (opened, or restored — V12
//!   restores many at once);
//! - `stopped` is not raised for a stop a `needs_you` was raised for.
//!
//! A change that is not raised (silence, the front tab, a setting) is still remembered, so it
//! is never raised later: a project that was already ended when it was restored does not
//! notify 30 s afterwards.
//!
//! The sentences are fixed; the only text in them that is not the app's own is the project's
//! folder name and a number (status.md §Security).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::attention::Kind;

/// No notification in this long after a tab is first seen.
pub const SILENCE: Duration = Duration::from_secs(30);
/// `stopped`: at least this much work before the stop (D-062 addendum 3: 15 s, not 60)…
pub const STOP_WORKED_S: u32 = 15;
/// …then no output for this long.
pub const STOP_QUIET_S: u32 = 30;

/// The closed list of events (component 6), in the table's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    NeedsYou,
    Stopped,
    Stuck,
    Ended,
    Handoff,
}

impl Event {
    pub const ALL: [Event; 5] = [
        Event::NeedsYou,
        Event::Stopped,
        Event::Stuck,
        Event::Ended,
        Event::Handoff,
    ];

    /// The name `[notifications] events` uses.
    pub fn name(self) -> &'static str {
        match self {
            Event::NeedsYou => "needs_you",
            Event::Stopped => "stopped",
            Event::Stuck => "stuck",
            Event::Ended => "ended",
            Event::Handoff => "handoff",
        }
    }

    pub fn from_name(s: &str) -> Option<Event> {
        Event::ALL.into_iter().find(|e| e.name() == s)
    }
}

/// The event names as `[notifications] events` takes them.
pub const EVENT_NAMES: [&str; 5] = ["needs_you", "stopped", "stuck", "ended", "handoff"];

/// What the notifier reads of the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `[notifications] enabled`: false raises nothing.
    pub enabled: bool,
    /// `[notifications] events`: the events that are raised.
    pub events: Vec<Event>,
    /// `[notifications] system`: the operating system's own notification (channel 3, E-79).
    pub system: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            events: Event::ALL.to_vec(),
            system: true,
        }
    }
}

/// At most one system notification per project in this long (component 6 limits).
pub const SYSTEM_EVERY: Duration = Duration::from_secs(60);

/// The system notification's limit (channel 3, E-79): one per project per [`SYSTEM_EVERY`];
/// the events raised later in that minute become one merged sentence when it ends. Keyed by
/// the project's path, so two tabs of one project share it.
#[derive(Debug, Default)]
pub struct SystemLimit {
    projects: HashMap<String, Held>,
}

#[derive(Debug)]
struct Held {
    sent: Instant,
    later: u32,
    folder: String,
}

/// One raised notification as channel 3 sees it: whose project, its folder name, its sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raised {
    pub project: String,
    pub folder: String,
    pub sentence: String,
}

impl SystemLimit {
    /// The texts to show now for `raised` (this poll's notifications) and for any minute that
    /// has ended with events held. `show`: the window is not focused or is hidden, and
    /// `[notifications] system` is on — otherwise nothing is shown and nothing is held.
    pub fn step(&mut self, raised: &[Raised], show: bool, now: Instant) -> Vec<String> {
        if !show {
            self.projects.clear();
            return Vec::new();
        }
        let mut out = Vec::new();
        // A minute that has ended: its later events, merged.
        for h in self.projects.values_mut() {
            if h.later > 0 && now.duration_since(h.sent) >= SYSTEM_EVERY {
                out.push(merged(&h.folder, h.later));
                h.later = 0;
                h.sent = now;
            }
        }
        for r in raised {
            match self.projects.get_mut(&r.project) {
                Some(h) if now.duration_since(h.sent) < SYSTEM_EVERY => h.later += 1,
                _ => {
                    out.push(r.sentence.clone());
                    self.projects.insert(
                        r.project.clone(),
                        Held {
                            sent: now,
                            later: 0,
                            folder: r.folder.clone(),
                        },
                    );
                }
            }
        }
        out
    }
}

/// The merged sentence: "<project>: <n> things need you." Only the folder name and a number.
pub fn merged(folder: &str, n: u32) -> String {
    if n == 1 {
        format!("{folder}: 1 thing needs you.")
    } else {
        format!("{folder}: {n} things need you.")
    }
}

/// One tab as the status poll saw it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    /// The tab's core id.
    pub tab: u32,
    /// The project's folder name (printable only; see [`folder_name`]).
    pub project: String,
    /// The kinds of the tab's "Now" items (component 4), dismissed ones left out.
    pub now: Vec<Kind>,
    /// How long output had been arriving before it stopped.
    pub worked_s: u32,
    /// Seconds since the window's last output.
    pub quiet_s: Option<u32>,
    /// That output's time (tmux's clock): which stop a `stopped` is about.
    pub last_output: Option<u64>,
    /// The last handoff's time (seconds since the epoch), when its time is readable.
    pub handoff_at: Option<u64>,
}

/// `Notification` of the data model; the `notify` event's payload (`{event, tab, sentence}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notification {
    pub event: Event,
    pub tab: u32,
    pub sentence: String,
}

/// What the notifier remembers of one tab.
#[derive(Debug, Clone)]
struct Memory {
    first_seen: Instant,
    now: Vec<Kind>,
    /// The `last_output` of the last stop counted (raised or not).
    stop: Option<u64>,
    /// The `last_output` when the last `needs_you` appeared (`Some(None)`: none read then).
    needs_you: Option<Option<u64>>,
    /// The last handoff seen.
    handoff_at: Option<u64>,
}

/// The notifier: one memory per open tab.
#[derive(Debug, Default)]
pub struct Notifier {
    tabs: HashMap<u32, Memory>,
}

impl Notifier {
    /// One poll: the events of `seen` that are changes and may be raised. `front`: the current
    /// tab of a focused window (none when the window is not focused). A tab not in `seen` is
    /// forgotten.
    pub fn step(
        &mut self,
        seen: &[Seen],
        now: Instant,
        front: Option<u32>,
        settings: &Settings,
    ) -> Vec<Notification> {
        self.tabs.retain(|t, _| seen.iter().any(|s| s.tab == *t));
        let mut out = Vec::new();
        for s in seen {
            let m = self.tabs.entry(s.tab).or_insert_with(|| Memory {
                first_seen: now,
                now: Vec::new(),
                stop: None,
                needs_you: None,
                handoff_at: None,
            });
            let appeared = |k: Kind| s.now.contains(&k) && !m.now.contains(&k);
            let mut events = Vec::new();
            if appeared(Kind::Bell) {
                events.push(Event::NeedsYou);
                m.needs_you = Some(s.last_output);
            }
            if let Some(lo) = s
                .last_output
                .filter(|_| s.worked_s >= STOP_WORKED_S)
                .filter(|_| s.quiet_s.is_some_and(|q| q >= STOP_QUIET_S))
                .filter(|lo| m.stop != Some(*lo))
            {
                m.stop = Some(lo);
                // Not for a stop a `needs_you` was raised for: one is still in the list, or
                // one appeared at or after the stop's last output.
                let told = s.now.contains(&Kind::Bell)
                    || m.needs_you.is_some_and(|n| n.is_none_or(|n| n >= lo));
                if !told {
                    events.push(Event::Stopped);
                }
            }
            if appeared(Kind::Stuck) {
                events.push(Event::Stuck);
            }
            if appeared(Kind::Ended) {
                events.push(Event::Ended);
            }
            if s.now.contains(&Kind::Mention)
                && s.handoff_at.is_some()
                && s.handoff_at > m.handoff_at
            {
                events.push(Event::Handoff);
            }
            if s.handoff_at.is_some() {
                m.handoff_at = m.handoff_at.max(s.handoff_at);
            }
            m.now.clone_from(&s.now);
            let quiet = now.duration_since(m.first_seen) < SILENCE || front == Some(s.tab);
            if quiet || !settings.enabled {
                continue;
            }
            out.extend(
                events
                    .into_iter()
                    .filter(|e| settings.events.contains(e))
                    .map(|event| Notification {
                        event,
                        tab: s.tab,
                        sentence: sentence(event, &s.project, s.quiet_s.unwrap_or(0)),
                    }),
            );
        }
        out
    }
}

/// The fixed sentence of `event` (component 6). The only inputs are the event, the folder
/// name and a number: no text from a project file can reach it.
pub fn sentence(event: Event, project: &str, quiet_s: u32) -> String {
    match event {
        Event::NeedsYou => format!("{project} needs you."),
        Event::Stopped => format!("{project} has stopped — it is finished or waiting."),
        Event::Stuck => format!(
            "{project} has been quiet for {} min with a task in progress.",
            quiet_s / 60
        ),
        Event::Ended => format!("{project}: an agent has ended."),
        Event::Handoff => format!("{project}: the last handoff names you."),
    }
}

/// The project's folder name as a sentence shows it: control characters left out, at most 64
/// characters (D-024 keeps names printable; this holds whatever the path is).
pub fn folder_name(project: &std::path::Path) -> String {
    let name = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let clean: String = name.chars().filter(|c| !c.is_control()).take(64).collect();
    if clean.is_empty() {
        "A project".into()
    } else {
        clean
    }
}

/// The count of "Now" items shown on the app's icon (channel 2), where the system offers one.
pub fn badge(now_items: usize) -> Option<i64> {
    (now_items > 0).then(|| i64::try_from(now_items).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: u32 = 7;

    fn seen(now: &[Kind]) -> Seen {
        Seen {
            tab: T,
            project: "qrtable".into(),
            now: now.to_vec(),
            ..Seen::default()
        }
    }

    /// A notifier whose tab `T` was first seen at `t0`, past its silence at `t0 + 31 s`.
    fn opened(n: &mut Notifier, t0: Instant) -> Instant {
        assert!(
            n.step(&[seen(&[])], t0, None, &Settings::default())
                .is_empty()
        );
        t0 + Duration::from_secs(31)
    }

    fn events(v: &[Notification]) -> Vec<Event> {
        v.iter().map(|x| x.event).collect()
    }

    /// Each event raised once on its change and not again while the state stays; again only
    /// after the state went and came back.
    #[test]
    fn each_event_is_raised_once_and_not_again() {
        let s = Settings::default();
        for (kind, event) in [
            (Kind::Bell, Event::NeedsYou),
            (Kind::Stuck, Event::Stuck),
            (Kind::Ended, Event::Ended),
        ] {
            let mut n = Notifier::default();
            let t = opened(&mut n, Instant::now());
            let first = n.step(&[seen(&[kind])], t, None, &s);
            assert_eq!(events(&first), [event], "{kind:?}");
            assert_eq!(first[0].tab, T);
            for i in 1..5 {
                let again = n.step(&[seen(&[kind])], t + Duration::from_secs(5 * i), None, &s);
                assert!(again.is_empty(), "{kind:?} repeated");
            }
            assert!(
                n.step(&[seen(&[])], t + Duration::from_secs(30), None, &s)
                    .is_empty()
            );
            let back = n.step(&[seen(&[kind])], t + Duration::from_secs(35), None, &s);
            assert_eq!(events(&back), [event], "{kind:?} after it went");
        }
    }

    fn working(worked_s: u32, quiet_s: u32, last_output: u64) -> Seen {
        Seen {
            worked_s,
            quiet_s: Some(quiet_s),
            last_output: Some(last_output),
            ..seen(&[])
        }
    }

    /// `stopped`: 15 s of work, then 30 s quiet; once per stop; not for less work or a shorter
    /// quiet; again for the next stop.
    #[test]
    fn stopped_after_work_then_thirty_seconds_quiet_once_per_stop() {
        let s = Settings::default();
        let mut n = Notifier::default();
        let t = opened(&mut n, Instant::now());
        assert!(
            n.step(&[working(40, 29, 1000)], t, None, &s).is_empty(),
            "29 s"
        );
        let v = n.step(&[working(40, 30, 1000)], t, None, &s);
        assert_eq!(events(&v), [Event::Stopped]);
        assert_eq!(
            v[0].sentence,
            "qrtable has stopped — it is finished or waiting."
        );
        assert!(
            n.step(&[working(40, 90, 1000)], t, None, &s).is_empty(),
            "same stop"
        );
        // Work again, stop again: a new stop.
        assert!(n.step(&[working(20, 2, 1200)], t, None, &s).is_empty());
        assert_eq!(
            events(&n.step(&[working(20, 31, 1200)], t, None, &s)),
            [Event::Stopped]
        );
        // Under 15 s of work (a resize redraw, a one-line print): nothing.
        assert!(n.step(&[working(14, 60, 1400)], t, None, &s).is_empty());
    }

    /// `stopped` is not raised when `needs_you` was raised for the same stop.
    #[test]
    fn no_stopped_for_a_stop_that_raised_needs_you() {
        let s = Settings::default();
        let mut n = Notifier::default();
        let t = opened(&mut n, Instant::now());
        let bell = |q, lo| Seen {
            now: vec![Kind::Bell],
            ..working(40, q, lo)
        };
        assert_eq!(
            events(&n.step(&[bell(5, 1000)], t, None, &s)),
            [Event::NeedsYou]
        );
        assert!(n.step(&[bell(31, 1000)], t, None, &s).is_empty());
        // The bell was looked at (gone), still the same stop: nothing.
        assert!(n.step(&[working(40, 60, 1000)], t, None, &s).is_empty());
        // A bell that rang before more work does not cover the next stop.
        assert!(n.step(&[working(40, 1, 1100)], t, None, &s).is_empty());
        assert_eq!(
            events(&n.step(&[working(40, 30, 1100)], t, None, &s)),
            [Event::Stopped]
        );
    }

    /// `handoff`: a mention for a handoff newer than the last one seen; not for the same
    /// handoff again, nor for an older one.
    #[test]
    fn handoff_for_a_newer_handoff_that_names_the_operator() {
        let s = Settings::default();
        let mut n = Notifier::default();
        let t0 = Instant::now();
        let h = |at: Option<u64>, mention: bool| Seen {
            handoff_at: at,
            ..seen(if mention { &[Kind::Mention] } else { &[] })
        };
        assert!(n.step(&[h(Some(100), false)], t0, None, &s).is_empty());
        let t = t0 + Duration::from_secs(31);
        assert!(n.step(&[h(Some(100), false)], t, None, &s).is_empty());
        let v = n.step(&[h(Some(200), true)], t, None, &s);
        assert_eq!(events(&v), [Event::Handoff]);
        assert_eq!(v[0].sentence, "qrtable: the last handoff names you.");
        assert!(n.step(&[h(Some(200), true)], t, None, &s).is_empty());
        assert!(
            n.step(&[h(Some(150), true)], t, None, &s).is_empty(),
            "older"
        );
        assert!(n.step(&[h(None, true)], t, None, &s).is_empty(), "no time");
        // A newer handoff that does not name the operator: nothing, and it is seen.
        assert!(n.step(&[h(Some(300), false)], t, None, &s).is_empty());
        assert!(n.step(&[h(Some(300), true)], t, None, &s).is_empty());
        assert_eq!(
            events(&n.step(&[h(Some(400), true)], t, None, &s)),
            [Event::Handoff]
        );
    }

    /// Nothing in the first 30 s after a tab is first seen, and what was there then is not
    /// raised afterwards (a restored project that had already ended).
    #[test]
    fn the_thirty_second_silence_after_a_tab_opens() {
        let s = Settings::default();
        let mut n = Notifier::default();
        let t0 = Instant::now();
        assert!(n.step(&[seen(&[Kind::Ended])], t0, None, &s).is_empty());
        let t29 = t0 + Duration::from_secs(29);
        assert!(
            n.step(&[seen(&[Kind::Ended, Kind::Bell])], t29, None, &s)
                .is_empty(),
            "a change inside the silence"
        );
        let t31 = t0 + Duration::from_secs(31);
        assert!(
            n.step(&[seen(&[Kind::Ended, Kind::Bell])], t31, None, &s)
                .is_empty(),
            "not raised later"
        );
        assert_eq!(
            events(&n.step(
                &[seen(&[Kind::Ended, Kind::Bell, Kind::Stuck])],
                t31,
                None,
                &s
            )),
            [Event::Stuck]
        );
        // A tab closed and opened again (a new memory) starts a new silence.
        assert!(n.step(&[], t31, None, &s).is_empty());
        assert!(n.step(&[seen(&[])], t31, None, &s).is_empty());
        assert!(n.step(&[seen(&[Kind::Stuck])], t31, None, &s).is_empty());
    }

    /// Nothing for the current tab of a focused window; the change is remembered, so going to
    /// another tab does not raise it.
    #[test]
    fn nothing_for_the_tab_in_front_of_the_operator() {
        let s = Settings::default();
        let mut n = Notifier::default();
        let t = opened(&mut n, Instant::now());
        assert!(n.step(&[seen(&[Kind::Ended])], t, Some(T), &s).is_empty());
        assert!(n.step(&[seen(&[Kind::Ended])], t, None, &s).is_empty());
        assert_eq!(
            events(&n.step(&[seen(&[Kind::Ended, Kind::Stuck])], t, Some(T + 1), &s)),
            [Event::Stuck]
        );
    }

    /// Each event switched off by its setting, and all by `enabled = false`; a switched-off
    /// change is not raised when the setting comes back on.
    #[test]
    fn settings_switch_each_event_off() {
        let all = [Kind::Bell, Kind::Stuck, Kind::Ended, Kind::Mention];
        let full = |n: &mut Notifier, t: Instant, s: &Settings| {
            n.step(
                &[Seen {
                    handoff_at: Some(500),
                    ..working(40, 40, 900)
                }],
                t,
                None,
                s,
            )
        };
        let mut n = Notifier::default();
        let t = opened(&mut n, Instant::now());
        // The bell covers the stop, so every event but `stopped` with all four items…
        let with = |n: &mut Notifier, s: &Settings| {
            let v = n.step(
                &[Seen {
                    now: all.to_vec(),
                    handoff_at: Some(600),
                    ..working(40, 40, 1000)
                }],
                t,
                None,
                s,
            );
            events(&v)
        };
        assert_eq!(
            with(&mut n, &Settings::default()),
            [Event::NeedsYou, Event::Stuck, Event::Ended, Event::Handoff]
        );
        // …and `stopped` alone without them.
        let mut n = Notifier::default();
        let t = opened(&mut n, t);
        assert_eq!(
            events(&full(&mut n, t, &Settings::default())),
            [Event::Stopped]
        );

        for off in Event::ALL {
            let s = Settings {
                enabled: true,
                events: Event::ALL.into_iter().filter(|e| *e != off).collect(),
                system: true,
            };
            let mut n = Notifier::default();
            let t1 = opened(&mut n, t);
            let mut got = events(&n.step(
                &[Seen {
                    now: all.to_vec(),
                    handoff_at: Some(600),
                    ..working(40, 40, 1000)
                }],
                t1,
                None,
                &s,
            ));
            let mut m = Notifier::default();
            let t2 = opened(&mut m, t);
            got.extend(events(&full(&mut m, t2, &s)));
            assert!(!got.contains(&off), "{off:?} switched off");
            assert_eq!(got.len(), 4, "the other four stay on ({off:?})");
        }
        let none = Settings {
            enabled: false,
            ..Settings::default()
        };
        let mut n = Notifier::default();
        let t = opened(&mut n, t);
        assert!(full(&mut n, t, &none).is_empty());
        assert!(
            full(&mut n, t, &Settings::default()).is_empty(),
            "not raised when switched back on"
        );
    }

    fn raised(project: &str, sentence: &str) -> Raised {
        Raised {
            project: project.into(),
            folder: project.trim_start_matches('/').into(),
            sentence: sentence.into(),
        }
    }

    /// E-79: one system notification per project per 60 s; the later events of that minute
    /// become one merged sentence when it ends; another project has its own minute.
    #[test]
    fn the_sixty_second_limit_and_the_merged_sentence() {
        let mut l = SystemLimit::default();
        let t = Instant::now();
        let s = |d: u64| t + Duration::from_secs(d);
        assert_eq!(
            l.step(&[raised("/a", "a needs you.")], true, t),
            ["a needs you."]
        );
        assert!(
            l.step(&[raised("/a", "a: an agent has ended.")], true, s(10))
                .is_empty()
        );
        assert!(
            l.step(&[raised("/a", "a needs you.")], true, s(40))
                .is_empty()
        );
        assert_eq!(
            l.step(&[raised("/b", "b needs you.")], true, s(41)),
            ["b needs you."],
            "another project's minute"
        );
        assert!(
            l.step(&[], true, s(59)).is_empty(),
            "the minute has not ended"
        );
        assert_eq!(l.step(&[], true, s(60)), ["a: 2 things need you."]);
        assert!(l.step(&[], true, s(70)).is_empty(), "merged once");
        // The merged one starts a minute of its own.
        assert!(
            l.step(&[raised("/a", "a needs you.")], true, s(100))
                .is_empty()
        );
        assert_eq!(l.step(&[], true, s(120)), ["a: 1 thing needs you."]);
        assert_eq!(
            l.step(&[raised("/a", "a needs you.")], true, s(181)),
            ["a needs you."]
        );
    }

    /// E-79: `system = false` (or the window in front) shows nothing and holds nothing.
    #[test]
    fn nothing_is_shown_while_switched_off_or_in_front() {
        let mut l = SystemLimit::default();
        let t = Instant::now();
        assert!(l.step(&[raised("/a", "a needs you.")], false, t).is_empty());
        assert_eq!(
            l.step(
                &[raised("/a", "a needs you.")],
                true,
                t + Duration::from_secs(1)
            ),
            ["a needs you."],
            "nothing was held while off"
        );
        assert!(
            l.step(&[raised("/a", "x")], true, t + Duration::from_secs(2))
                .is_empty()
        );
        assert!(
            l.step(&[], false, t + Duration::from_secs(61)).is_empty(),
            "a held event is dropped once the window is in front"
        );
        assert_eq!(
            l.step(
                &[raised("/a", "a needs you.")],
                true,
                t + Duration::from_secs(62)
            ),
            ["a needs you."]
        );
    }

    /// E-79 (status.md §Security): no notification text can hold text from a project file.
    /// The builders take only the event, the folder name and a number; the folder name is the
    /// only outside text, and every event's sentence is its fixed template around it.
    /// D-069 condition 3: `<`, `&` and `"` in a folder name reach the text as they are (the
    /// Windows toast sets it with `SetInnerText`, never as markup).
    #[test]
    fn the_system_text_holds_only_the_folder_name_and_a_number() {
        let hostile = "<b &amp; \"x\">&lt;i";
        let folder = folder_name(std::path::Path::new(&format!("/p/{hostile}")));
        assert_eq!(folder, hostile);
        for e in Event::ALL {
            let text = sentence(e, &folder, 600);
            let template = sentence(e, "\u{1}", 600);
            assert_eq!(text, template.replace('\u{1}', &folder), "{e:?}");
        }
        assert_eq!(merged(&folder, 3), format!("{hostile}: 3 things need you."));
        // A task title, a handoff line or a quote never has a way in: `Seen` carries the
        // folder name, kinds, numbers and times only. A field added to it breaks this match
        // until someone has looked at what it carries.
        let Seen {
            tab: _,
            project: _,
            now: _,
            worked_s: _,
            quiet_s: _,
            last_output: _,
            handoff_at: _,
        }: Seen = Seen::default();
    }

    /// The sentences: fixed text, the folder name and a number only.
    #[test]
    fn the_fixed_sentences() {
        assert_eq!(sentence(Event::NeedsYou, "p", 0), "p needs you.");
        assert_eq!(
            sentence(Event::Stuck, "p", 754),
            "p has been quiet for 12 min with a task in progress."
        );
        assert_eq!(sentence(Event::Ended, "p", 0), "p: an agent has ended.");
        let p = std::path::Path::new("/home/x/qr\u{7}table\n");
        assert_eq!(folder_name(p), "qrtable");
        assert_eq!(folder_name(std::path::Path::new("/")), "A project");
        assert_eq!(
            folder_name(&std::path::Path::new("/a").join("x".repeat(99))).len(),
            64
        );
        assert_eq!((badge(0), badge(3)), (None, Some(3)));
        for e in Event::ALL {
            assert_eq!(Event::from_name(e.name()), Some(e));
            assert!(EVENT_NAMES.contains(&e.name()));
        }
    }
}
