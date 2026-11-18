//! "Needs you", triaged (status.md §Components 4, D-063; E-75): the rules, in the core. Two
//! lists — **Now** (counted in the rail badge) and **For information** (listed collapsed, not
//! counted). The web side sends only what it alone knows about each open tab (closed booleans,
//! by tab-bar index); the core adds each tab's `.ai/` facts (`panel_attention.rs`), whether
//! its process ended, its bells (reported by `tab_bell`, E-76), its agents and their window's
//! activity, and the dismissals. The answer names tabs by index, never by title or path: the
//! web side puts its own tab titles on it.
//!
//! `stuck` (E-76) replaced the old "no handoff for 30 minutes while a task is in progress".
//! Not built here: `waiting` (rein's signal, E-81).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::dismissed::{self, Dismissal, Dismissed};
use crate::ipc::{Desktop, IpcError, err};
use crate::panel_attention::{Attention, Blocked};

/// The most items in both lists together (panels.md §Data Model).
pub const MAX_ITEMS: usize = 200;
/// The most open tabs one request may describe (16 tabs plus set-up tabs, with room).
pub const MAX_TABS_IN: usize = 64;
/// The fixed sentence for a dismiss of a key the core did not send.
pub const NOT_IN_LIST: &str = "that item is not in the list";

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Ok,
    Warn,
    Fail,
    Unknown,
}

/// What only the web side knows about one open tab.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TabIn {
    /// The tab's place in the tab bar (0-based).
    pub index: u32,
    /// The tab's core id, when it has a terminal.
    pub tab: Option<u32>,
    pub refused: bool,
    pub update: bool,
    /// The tab needs set-up though it has a terminal (D-062 addendum 4, E-95; a failed start
    /// that needs `rein init`, D-068): its status is `setup`. Absent: false.
    #[serde(default)]
    pub setup: bool,
}

/// What the core knows about the same tab.
#[derive(Debug, Clone, Default)]
pub struct TabCore {
    /// The canonical project path (dismissals are kept per project).
    pub project: Option<String>,
    pub facts: Option<Attention>,
    pub agents_running: bool,
    /// The tab's process (`rein start`) has ended.
    pub ended: bool,
    /// How many bells have rung while the tab was out of sight (0: none marked now). A new
    /// bell changes the number, so a dismissed bell comes back.
    pub bell: u32,
    /// The tab's window: seconds since its last output, and that output's time.
    pub quiet: Option<(u32, u64)>,
    /// `stuck` from this many seconds quiet ([notifications] stuck_after_min).
    pub stuck_after_s: u32,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Ended,
    Refused,
    Bell,
    Stuck,
    Mention,
    HealthFail,
    Blocked,
    Update,
    HealthWarn,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Item {
    /// Unique within its tab; with `tab`, what a dismiss names.
    pub key: String,
    pub kind: Kind,
    /// The tab-bar index it is about, or none (the installation's health).
    pub index: Option<u32>,
    /// The tab's core id, for a dismiss.
    pub tab: Option<u32>,
    /// The app's own sentence.
    pub text: String,
    /// Text from a project file, shown as a quote (a mention line).
    pub quote: Option<String>,
    /// The blocked tasks a `blocked` row opens to.
    pub list: Vec<Blocked>,
    pub dismissable: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct Lists {
    pub now: Vec<Item>,
    pub info: Vec<Item>,
}

/// "2026-11-13 12:20 UTC" in seconds since the epoch; anything else is not guessed at. The
/// old quiet rule's reader, kept for the Home card's last-handoff age (status.md §Home, E-80).
#[allow(dead_code)]
pub fn handoff_seconds(ts: &str) -> Option<u64> {
    let b = ts.trim().as_bytes();
    if b.len() != 20
        || &b[16..] != b" UTC"
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b' '
        || b[13] != b':'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = std::str::from_utf8(&b[r]).ok()?;
        s.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse().ok())?
    };
    let (y, mo, d, h, mi) = (
        num(0..4)?,
        num(5..7)?,
        num(8..10)?,
        num(11..13)?,
        num(14..16)?,
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + h * 3600 + mi * 60).ok()
}

/// A span as the panel says it: minutes up to two hours, then hours, then days.
pub fn span(s: u64) -> String {
    let m = s / 60;
    if m < 120 {
        format!("{m} min")
    } else if m < 48 * 60 {
        format!("{} h", m / 60)
    } else {
        format!("{} d", m / 1440)
    }
}

/// The hash a dismissal keeps for `item`: with the bell count, so a new bell is new; for
/// `stuck` the time of the last output instead of the text (which counts the minutes up), so
/// only quiet again after new work brings it back.
fn item_hash(item: &Item, core: &TabCore) -> String {
    let (text, salt) = match item.kind {
        Kind::Stuck => ("", core.quiet.map_or(0, |q| q.1).to_string()),
        _ => (item.text.as_str(), core.bell.to_string()),
    };
    dismissed::text_hash(&[&item.key, text, item.quote.as_deref().unwrap_or(""), &salt])
}

/// A dismissable item the core sent: (core tab, key, hash).
pub type Sent = (u32, String, String);

/// The rules (component 4), first the installation, then each tab in tab order. A dismissed
/// item is left out while its text is unchanged. Both lists together hold at most 200.
pub fn lists(
    tabs: &[(TabIn, TabCore)],
    health: Health,
    dismissed: &dyn Fn(&str, &str, &str) -> bool,
) -> (Lists, Vec<Sent>) {
    let mut out = Lists::default();
    let mut sent = Vec::new();
    match health {
        Health::Fail => out.now.push(Item {
            key: "health".into(),
            kind: Kind::HealthFail,
            index: None,
            tab: None,
            text: "The installation health check is failing.".into(),
            quote: None,
            list: vec![],
            dismissable: false,
        }),
        Health::Warn => out.info.push(Item {
            key: "health".into(),
            kind: Kind::HealthWarn,
            index: None,
            tab: None,
            text: "The installation health check has warnings.".into(),
            quote: None,
            list: vec![],
            dismissable: false,
        }),
        Health::Ok | Health::Unknown => {}
    }
    for (t, core) in tabs {
        let item =
            |kind: Kind, key: String, text: String, quote: Option<String>, dismissable: bool| {
                Item {
                    key,
                    kind,
                    index: Some(t.index),
                    tab: t.tab,
                    text,
                    quote,
                    list: vec![],
                    dismissable: dismissable && t.tab.is_some() && core.project.is_some(),
                }
            };
        let mut now_items = Vec::new();
        if t.refused {
            now_items.push(item(
                Kind::Refused,
                "refused".into(),
                "The tmux setup was refused; the tab says why.".into(),
                None,
                false,
            ));
        } else if core.ended {
            now_items.push(item(
                Kind::Ended,
                "ended".into(),
                "rein start has ended in this tab.".into(),
                None,
                false,
            ));
        }
        if core.bell > 0 {
            now_items.push(item(
                Kind::Bell,
                "bell".into(),
                "The terminal rang its bell.".into(),
                None,
                true,
            ));
        }
        if let Some(f) = &core.facts {
            // stuck: an agent runs, a task is in progress, and its window has been quiet.
            if let Some((q, _)) = core
                .quiet
                .filter(|&(q, _)| f.in_progress && core.agents_running && q >= core.stuck_after_s)
            {
                now_items.push(item(
                    Kind::Stuck,
                    "stuck".into(),
                    format!(
                        "An agent is running and a task is in progress, but there has been no output for {}.",
                        span(u64::from(q))
                    ),
                    None,
                    true,
                ));
            }
            for (i, m) in f.mentions.iter().enumerate() {
                now_items.push(item(
                    Kind::Mention,
                    format!("mention:{i}"),
                    "The last handoff mentions the operator:".into(),
                    Some(m.clone()),
                    true,
                ));
            }
        }
        for it in now_items {
            if it.dismissable {
                let (Some(tab), Some(project)) = (t.tab, core.project.as_deref()) else {
                    continue;
                };
                let hash = item_hash(&it, core);
                if dismissed(project, &it.key, &hash) {
                    continue;
                }
                sent.push((tab, it.key.clone(), hash));
            }
            out.now.push(it);
        }
        if t.update {
            out.info.push(item(
                Kind::Update,
                "update".into(),
                "rein says the project is out of date.".into(),
                None,
                false,
            ));
        }
        if let Some(f) = &core.facts {
            let n = f.blocked.len();
            if n > 0 {
                let mut row = item(
                    Kind::Blocked,
                    "blocked".into(),
                    if n == 1 {
                        "1 task is blocked".into()
                    } else {
                        format!("{n} tasks are blocked")
                    },
                    None,
                    false,
                );
                row.list = f.blocked.clone();
                out.info.push(row);
            }
        }
    }
    out.now.truncate(MAX_ITEMS);
    out.info.truncate(MAX_ITEMS - out.now.len());
    (out, sent)
}

/// The dismissals, the dismissable keys last sent per core tab (a dismiss must name one), and
/// what only the web side can tell the core (status.md §API, E-76).
pub struct AttentionState {
    pub dismissed: Dismissed,
    sent: Mutex<HashMap<u32, HashMap<String, String>>>,
    /// What the web side reported (E-76).
    pub reported: crate::reported::Reported,
}

impl AttentionState {
    pub fn new(dismissed: Dismissed) -> Self {
        Self {
            dismissed,
            sent: Mutex::new(HashMap::new()),
            reported: crate::reported::Reported::default(),
        }
    }

    fn remember(&self, sent: Vec<Sent>) {
        let mut m = self.sent.lock().unwrap_or_else(|e| e.into_inner());
        m.clear();
        for (tab, key, hash) in sent {
            m.entry(tab).or_default().insert(key, hash);
        }
    }

    /// The hash the core sent with `key` for `tab`, taken out (one dismiss per sending), or
    /// the fixed refusal: a key the core did not send, or not a dismissable one.
    fn take_sent(&self, tab: u32, key: &str) -> Result<String, IpcError> {
        self.sent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&tab)
            .and_then(|k| k.remove(key))
            .ok_or_else(|| err("not_in_list", NOT_IN_LIST))
    }
}

fn now_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The core's half of each tab: its project, its facts (read through the panel cache, as the
/// panels are), whether its process ended, its bells, and its agents and their activity.
fn core_of(d: &Desktop, a: &AttentionState, t: &TabIn, stuck_after_s: u32) -> TabCore {
    let Some(tab) = t.tab else {
        return TabCore::default();
    };
    let Some((pty, _, project)) = d.tab_target(tab) else {
        return TabCore::default();
    };
    let facts =
        match crate::panel_ipc::panel_reply(d, tab, crate::panel_ipc::Source::Attention, now_s()) {
            crate::panel_ipc::Reply::Ok { ok, .. } => match *ok {
                crate::panel_ipc::Data::Attention(a) => Some(a),
                _ => None,
            },
            crate::panel_ipc::Reply::Unavailable { .. } => None,
        };
    let seen = d.agents.seen(tab);
    TabCore {
        project: Some(project.to_string_lossy().into_owned()),
        facts,
        agents_running: seen
            .agents
            .iter()
            .any(|x| x.state == crate::agents::AgentState::Running),
        ended: !d.host.alive(pty),
        bell: a.reported.bells(tab),
        quiet: seen.activity.quiet_s.zip(seen.activity.last_output),
        stuck_after_s,
    }
}

/// `attention_items(tabs, health)` → `{now, info}`, the core's answer for every open tab.
/// The input is kept: the status poll uses it for the tabs' closed booleans.
pub fn answer(
    d: &Desktop,
    a: &AttentionState,
    mut tabs: Vec<TabIn>,
    health: Health,
    stuck_after_s: u32,
) -> Lists {
    tabs.truncate(MAX_TABS_IN);
    a.reported.input(tabs.clone(), health);
    let (lists, sent, _) = lists_of(d, a, tabs, health, stuck_after_s);
    // Only what the web side was shown can be dismissed (the status poll's lists are not).
    a.remember(sent);
    lists
}

/// The same lists for every tab with a process, as the status poll needs them: each tab's
/// closed booleans as the web side last sent them (none sent: all false).
/// The lists for every running tab, and each tab's facts by core id (the status's task and
/// turn, E-95) — one read of the two files per tab for both.
pub fn answer_all(
    d: &Desktop,
    a: &AttentionState,
    stuck_after_s: u32,
) -> (Lists, HashMap<u32, Attention>) {
    let (sent, health) = a.reported.last_input();
    let tabs = d
        .running_tabs()
        .into_iter()
        .enumerate()
        .map(|(i, (tab, _, _))| {
            sent.iter()
                .find(|t| t.tab == Some(tab))
                .cloned()
                .unwrap_or(TabIn {
                    index: u32::try_from(i).unwrap_or(u32::MAX),
                    tab: Some(tab),
                    refused: false,
                    update: false,
                    setup: false,
                })
        })
        .collect();
    let (lists, _, facts) = lists_of(d, a, tabs, health, stuck_after_s);
    (lists, facts)
}

fn lists_of(
    d: &Desktop,
    a: &AttentionState,
    tabs: Vec<TabIn>,
    health: Health,
    stuck_after_s: u32,
) -> (Lists, Vec<Sent>, HashMap<u32, Attention>) {
    let with: Vec<(TabIn, TabCore)> = tabs
        .into_iter()
        .map(|t| {
            let core = core_of(d, a, &t, stuck_after_s);
            // A core id the core does not know is no core id.
            let t = TabIn {
                tab: t.tab.filter(|_| core.project.is_some()),
                ..t
            };
            (t, core)
        })
        .collect();
    let facts = with
        .iter()
        .filter_map(|(t, c)| Some((t.tab?, c.facts.clone()?)))
        .collect();
    let (lists, sent) = lists(&with, health, &|p, k, h| a.dismissed.has(p, k, h));
    (lists, sent, facts)
}

/// `attention_dismiss(tab, key)`: only a dismissable key the core last sent for that tab.
pub fn dismiss(d: &Desktop, a: &AttentionState, tab: u32, key: &str) -> Result<(), IpcError> {
    let hash = a.take_sent(tab, key)?;
    let project = d
        .tab_project(tab)
        .ok_or_else(|| err("not_in_list", NOT_IN_LIST))?;
    a.dismissed
        .add(
            Dismissal {
                project: project.to_string_lossy().into_owned(),
                key: key.into(),
                text_hash: hash,
                at: 0,
            },
            now_s(),
        )
        .map_err(|_| err("io", "the dismissal could not be saved"))
}

#[tauri::command(async)]
pub fn attention_items(
    state: State<'_, Desktop>,
    attention: State<'_, AttentionState>,
    config: State<'_, std::sync::Arc<crate::config::ConfigService>>,
    tabs: Vec<TabIn>,
    health: Health,
) -> Lists {
    let stuck = crate::status_watch::stuck_after_s(&config);
    answer(&state, &attention, tabs, health, stuck)
}

#[tauri::command(async)]
pub fn attention_dismiss(
    state: State<'_, Desktop>,
    attention: State<'_, AttentionState>,
    tab: u32,
    key: String,
) -> Result<(), IpcError> {
    dismiss(&state, &attention, tab, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_attention::Blocked as B;

    const NOW: u64 = 1_763_380_800; // 2025-11-17 12:00 UTC
    fn tab(index: u32) -> TabIn {
        TabIn {
            index,
            tab: Some(index + 10),
            refused: false,
            update: false,
            setup: false,
        }
    }
    fn facts() -> Attention {
        Attention {
            tasks_read: true,
            blocked: vec![],
            in_progress: false,
            last_handoff: None,
            mentions: vec![],
            in_progress_task: None,
            turn: None,
        }
    }
    fn core(f: Attention) -> TabCore {
        TabCore {
            project: Some("/p".into()),
            facts: Some(f),
            agents_running: false,
            ended: false,
            bell: 0,
            quiet: None,
            stuck_after_s: 600,
        }
    }
    fn ended(c: TabCore) -> TabCore {
        TabCore { ended: true, ..c }
    }
    fn bell(n: u32, c: TabCore) -> TabCore {
        TabCore { bell: n, ..c }
    }
    fn run(tabs: &[(TabIn, TabCore)], health: Health) -> Lists {
        lists(tabs, health, &|_, _, _| false).0
    }
    fn kinds(v: &[Item]) -> Vec<Kind> {
        v.iter().map(|i| i.kind).collect()
    }

    #[test]
    fn each_rule_lands_in_its_group() {
        use Kind::*;
        // Nothing: a quiet tab.
        assert_eq!(
            run(&[(tab(0), core(facts()))], Health::Ok),
            Lists::default()
        );
        // One table: the input that makes the row, the group it lands in.
        let blocked = Attention {
            blocked: vec![
                B {
                    id: "E-1".into(),
                    description: "a".into(),
                },
                B {
                    id: "E-2".into(),
                    description: "b".into(),
                },
            ],
            ..facts()
        };
        let mention = Attention {
            mentions: vec!["the operator signs".into()],
            ..facts()
        };
        let busy = Attention {
            in_progress: true,
            ..facts()
        };
        /// The input, the health, and the kinds expected in Now and in For information.
        type Row = (TabIn, TabCore, Health, Vec<Kind>, Vec<Kind>);
        let rows: Vec<Row> = vec![
            (
                tab(0),
                ended(core(facts())),
                Health::Ok,
                vec![Ended],
                vec![],
            ),
            (
                TabIn {
                    refused: true,
                    ..tab(0)
                },
                ended(core(facts())),
                Health::Ok,
                vec![Refused],
                vec![],
            ),
            (
                tab(0),
                bell(1, core(facts())),
                Health::Ok,
                vec![Bell],
                vec![],
            ),
            (tab(0), core(mention), Health::Ok, vec![Mention], vec![]),
            (
                tab(0),
                core(facts()),
                Health::Fail,
                vec![HealthFail],
                vec![],
            ),
            (
                tab(0),
                core(facts()),
                Health::Warn,
                vec![],
                vec![HealthWarn],
            ),
            (tab(0), core(facts()), Health::Unknown, vec![], vec![]),
            (
                TabIn {
                    update: true,
                    setup: false,
                    ..tab(0)
                },
                core(facts()),
                Health::Ok,
                vec![],
                vec![Update],
            ),
            (tab(0), core(blocked), Health::Ok, vec![], vec![Blocked]),
            // stuck: only with an agent running, a task in progress and the window quiet.
            (
                tab(0),
                TabCore {
                    quiet: Some((600, 1)),
                    ..core(busy.clone())
                },
                Health::Ok,
                vec![],
                vec![],
            ),
            (
                tab(0),
                TabCore {
                    agents_running: true,
                    quiet: Some((600, 1)),
                    ..core(busy)
                },
                Health::Ok,
                vec![Stuck],
                vec![],
            ),
        ];
        for (t, c, h, now, info) in rows {
            let l = run(&[(t.clone(), c.clone())], h);
            assert_eq!(
                (kinds(&l.now), kinds(&l.info)),
                (now, info),
                "{t:?} {c:?} {h:?}"
            );
        }
    }

    /// The edges of the stuck rule (replacing the old "no handoff for 30 minutes" rule and
    /// its edges, moved from the web side's suite in E-75), facts not read, unique keys.
    #[test]
    fn the_stuck_rule_edges_and_missing_facts() {
        let stuck = |quiet: Option<(u32, u64)>, in_progress: bool, running: bool| {
            let c = TabCore {
                agents_running: running,
                quiet,
                ..core(Attention {
                    in_progress,
                    // The last handoff's age no longer counts.
                    last_handoff: Some("2020-01-01 00:00 UTC".into()),
                    ..facts()
                })
            };
            run(&[(tab(0), c)], Health::Ok).now
        };
        let at = stuck(Some((600, 5)), true, true);
        assert_eq!(kinds(&at), [Kind::Stuck]);
        assert!(
            at[0].text.ends_with("no output for 10 min."),
            "{}",
            at[0].text
        );
        assert!(at[0].dismissable);
        assert!(
            stuck(Some((599, 5)), true, true).is_empty(),
            "under stuck_after_min"
        );
        assert!(
            stuck(Some((3600, 5)), false, true).is_empty(),
            "nothing in progress"
        );
        assert!(
            stuck(Some((3600, 5)), true, false).is_empty(),
            "no agent running"
        );
        assert!(stuck(None, true, true).is_empty(), "activity not read");
        // The threshold is the setting.
        let c = TabCore {
            agents_running: true,
            quiet: Some((180, 5)),
            stuck_after_s: 180,
            ..core(Attention {
                in_progress: true,
                ..facts()
            })
        };
        assert_eq!(kinds(&run(&[(tab(0), c)], Health::Ok).now), [Kind::Stuck]);
        // Facts not read: only the app's own rules.
        let l = run(&[(tab(0), ended(TabCore::default()))], Health::Ok);
        assert_eq!(kinds(&l.now), [Kind::Ended]);
        // Keys are unique within a tab.
        let busy = core(Attention {
            mentions: vec!["operator a".into(), "operator b".into()],
            blocked: vec![B {
                id: "E-1".into(),
                description: String::new(),
            }],
            ..facts()
        });
        let l = run(
            &[(
                TabIn {
                    update: true,
                    setup: false,
                    ..tab(0)
                },
                bell(1, ended(busy)),
            )],
            Health::Ok,
        );
        let mut keys: Vec<&str> = l
            .now
            .iter()
            .chain(&l.info)
            .map(|i| i.key.as_str())
            .collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n);
    }

    #[test]
    fn blocked_is_one_row_per_project_and_the_order_is_fixed() {
        let b = |n: usize| Attention {
            blocked: (0..n)
                .map(|i| B {
                    id: format!("E-{i}"),
                    description: String::new(),
                })
                .collect(),
            mentions: vec!["operator".into()],
            ..facts()
        };
        let l = run(
            &[
                (
                    TabIn {
                        update: true,
                        setup: false,
                        ..tab(0)
                    },
                    bell(2, ended(core(b(12)))),
                ),
                (tab(1), core(b(1))),
            ],
            Health::Fail,
        );
        assert_eq!(
            kinds(&l.now),
            [
                Kind::HealthFail,
                Kind::Ended,
                Kind::Bell,
                Kind::Mention,
                Kind::Mention
            ]
        );
        assert_eq!(
            l.now.iter().map(|i| i.index).collect::<Vec<_>>(),
            [None, Some(0), Some(0), Some(0), Some(1)]
        );
        assert_eq!(kinds(&l.info), [Kind::Update, Kind::Blocked, Kind::Blocked]);
        assert_eq!(l.info[1].text, "12 tasks are blocked");
        assert_eq!(l.info[1].list.len(), 12);
        assert_eq!(l.info[2].text, "1 task is blocked");
        // Only bell and mention rows can be dismissed, and only with a core tab and project.
        assert_eq!(l.now.iter().filter(|i| i.dismissable).count(), 3);
        let no_core = run(
            &[(
                TabIn {
                    tab: None,
                    ..tab(0)
                },
                bell(1, TabCore::default()),
            )],
            Health::Ok,
        );
        assert!(!no_core.now[0].dismissable);
    }

    #[test]
    fn a_dismissed_item_returns_when_its_text_changes() {
        let m = |line: &str, bells: u32, quiet: (u32, u64)| TabCore {
            agents_running: true,
            quiet: Some(quiet),
            bell: bells,
            ..core(Attention {
                mentions: vec![line.into()],
                in_progress: true,
                ..facts()
            })
        };
        let (_, sent) = lists(
            &[(tab(0), m("operator: sign it", 1, (600, 100)))],
            Health::Ok,
            &|_, _, _| false,
        );
        assert_eq!(
            sent.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(),
            ["bell", "stuck", "mention:0"]
        );
        let gone: Vec<(String, String)> = sent.iter().map(|s| (s.1.clone(), s.2.clone())).collect();
        let hidden = |_: &str, k: &str, h: &str| gone.iter().any(|(gk, gh)| gk == k && gh == h);
        // The same words, and stuck counting up from the same last output: hidden.
        let same = lists(
            &[(tab(0), m("operator: sign it", 1, (1500, 100)))],
            Health::Ok,
            &hidden,
        );
        assert!(same.0.now.is_empty(), "{:?}", same.0.now);
        // A new bell (count 2), quiet again after new output, and a new line: back.
        let back = lists(
            &[(tab(0), m("operator: now the tag", 2, (600, 900)))],
            Health::Ok,
            &hidden,
        )
        .0;
        assert_eq!(kinds(&back.now), [Kind::Bell, Kind::Stuck, Kind::Mention]);
    }

    #[test]
    fn only_a_dismissable_key_the_core_sent_can_be_dismissed() {
        let a = AttentionState::new(Dismissed::open(None));
        let c = core(Attention {
            mentions: vec!["operator".into()],
            ..facts()
        });
        let (_, sent) = lists(&[(tab(0), bell(1, ended(c)))], Health::Ok, &|_, _, _| false);
        a.remember(sent);
        assert!(a.take_sent(10, "bell").is_ok());
        let refused = |r: Result<String, IpcError>| r.unwrap_err().message;
        assert_eq!(
            refused(a.take_sent(10, "bell")),
            NOT_IN_LIST,
            "once per sending"
        );
        assert_eq!(
            refused(a.take_sent(10, "ended")),
            NOT_IN_LIST,
            "not dismissable"
        );
        assert_eq!(
            refused(a.take_sent(10, "mention:7")),
            NOT_IN_LIST,
            "never sent"
        );
        assert_eq!(
            refused(a.take_sent(99, "mention:0")),
            NOT_IN_LIST,
            "another tab"
        );
        assert!(a.take_sent(10, "mention:0").is_ok());
    }

    #[test]
    fn at_most_200_items_and_timestamps_are_not_guessed() {
        let many = Attention {
            mentions: (0..150).map(|i| format!("operator {i}")).collect(),
            blocked: vec![B {
                id: "E-1".into(),
                description: String::new(),
            }],
            ..facts()
        };
        let tabs: Vec<(TabIn, TabCore)> = (0..3).map(|i| (tab(i), core(many.clone()))).collect();
        let l = run(&tabs, Health::Ok);
        assert_eq!(l.now.len() + l.info.len(), MAX_ITEMS);
        assert_eq!(handoff_seconds("2025-11-17 12:00 UTC"), Some(NOW));
        assert_eq!(handoff_seconds("1970-01-01 00:00 UTC"), Some(0));
        for bad in [
            "2025-11-17 12:00",
            "2025-13-17 12:00 UTC",
            "2025-11-17T12:00 UTC",
            "",
            "yesterday",
        ] {
            assert_eq!(handoff_seconds(bad), None, "{bad}");
        }
        assert_eq!(span(59 * 60), "59 min");
        assert_eq!(span(3 * 3600), "3 h");
        assert_eq!(span(3 * 86_400), "3 d");
    }
}
