//! "Needs you", triaged (status.md §Components 4, D-063; E-75): the rules, in the core. Two
//! lists — **Now** (counted in the rail badge) and **For information** (listed collapsed, not
//! counted). The web side sends only what it alone knows about each open tab (closed booleans
//! and a bell count, by tab-bar index); the core adds each tab's `.ai/` facts
//! (`panel_attention.rs`), whether its agents are running, and the dismissals. The answer
//! names tabs by index, never by title or path: the web side puts its own tab titles on it.
//!
//! Not built here: `stuck` and `waiting` (the status task). Until then the old "no handoff for
//! 30 minutes while a task is in progress" stays, but only for a tab whose agents are seen
//! running.

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
/// The old quiet rule: no handoff for this long while a task is in progress.
pub const QUIET_S: u64 = 30 * 60;
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
    pub exited: bool,
    pub refused: bool,
    /// How many bells have rung while the tab was out of sight (0: none marked now). A new
    /// bell changes the number, so a dismissed bell comes back.
    pub bell: u32,
    pub update: bool,
}

/// What the core knows about the same tab.
#[derive(Debug, Clone, Default)]
pub struct TabCore {
    /// The canonical project path (dismissals are kept per project).
    pub project: Option<String>,
    pub facts: Option<Attention>,
    pub agents_running: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Ended,
    Refused,
    Bell,
    Quiet,
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

/// "2026-11-13 12:20 UTC" in seconds since the epoch; anything else is not guessed at.
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

/// The hash a dismissal keeps for `item` (with the bell count, so a new bell is new).
fn item_hash(item: &Item, bell: u32) -> String {
    dismissed::text_hash(&[
        &item.key,
        &item.text,
        item.quote.as_deref().unwrap_or(""),
        &bell.to_string(),
    ])
}

/// A dismissable item the core sent: (core tab, key, hash).
pub type Sent = (u32, String, String);

/// The rules (component 4), first the installation, then each tab in tab order. A dismissed
/// item is left out while its text is unchanged. Both lists together hold at most 200.
pub fn lists(
    tabs: &[(TabIn, TabCore)],
    health: Health,
    now: u64,
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
        } else if t.exited {
            now_items.push(item(
                Kind::Ended,
                "ended".into(),
                "rein start has ended in this tab.".into(),
                None,
                false,
            ));
        }
        if t.bell > 0 {
            now_items.push(item(
                Kind::Bell,
                "bell".into(),
                "The terminal rang its bell.".into(),
                None,
                true,
            ));
        }
        if let Some(f) = &core.facts {
            // The old quiet rule, kept only while the tab's agents are seen running.
            let at = f.last_handoff.as_deref().and_then(handoff_seconds);
            if let Some(at) = at.filter(|&at| {
                f.in_progress && core.agents_running && now.saturating_sub(at) > QUIET_S
            }) {
                now_items.push(item(
                    Kind::Quiet,
                    "quiet".into(),
                    format!(
                        "A task is in progress and there has been no handoff for {}.",
                        span(now - at)
                    ),
                    None,
                    false,
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
                let hash = item_hash(&it, t.bell);
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

/// The dismissals, and the dismissable keys last sent per core tab (a dismiss must name one).
pub struct AttentionState {
    pub dismissed: Dismissed,
    sent: Mutex<HashMap<u32, HashMap<String, String>>>,
}

impl AttentionState {
    pub fn new(dismissed: Dismissed) -> Self {
        Self {
            dismissed,
            sent: Mutex::new(HashMap::new()),
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
/// panels are) and whether its agents were last seen running.
fn core_of(d: &Desktop, t: &TabIn) -> TabCore {
    let Some(tab) = t.tab else {
        return TabCore::default();
    };
    let Some(project) = d.tab_project(tab) else {
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
    TabCore {
        project: Some(project.to_string_lossy().into_owned()),
        facts,
        agents_running: d.agents.running(tab),
    }
}

/// `attention_items(tabs, health)` → `{now, info}`, the core's answer for every open tab.
pub fn answer(d: &Desktop, a: &AttentionState, mut tabs: Vec<TabIn>, health: Health) -> Lists {
    tabs.truncate(MAX_TABS_IN);
    let with: Vec<(TabIn, TabCore)> = tabs
        .into_iter()
        .map(|t| {
            let core = core_of(d, &t);
            // A core id the core does not know is no core id.
            let t = TabIn {
                tab: t.tab.filter(|_| core.project.is_some()),
                ..t
            };
            (t, core)
        })
        .collect();
    let (lists, sent) = lists(&with, health, now_s(), &|p, k, h| a.dismissed.has(p, k, h));
    a.remember(sent);
    lists
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
    tabs: Vec<TabIn>,
    health: Health,
) -> Lists {
    answer(&state, &attention, tabs, health)
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
            exited: false,
            refused: false,
            bell: 0,
            update: false,
        }
    }
    fn facts() -> Attention {
        Attention {
            tasks_read: true,
            blocked: vec![],
            in_progress: false,
            last_handoff: None,
            mentions: vec![],
        }
    }
    fn core(f: Attention) -> TabCore {
        TabCore {
            project: Some("/p".into()),
            facts: Some(f),
            agents_running: false,
        }
    }
    fn run(tabs: &[(TabIn, TabCore)], health: Health) -> Lists {
        lists(tabs, health, NOW, &|_, _, _| false).0
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
        let old = Attention {
            in_progress: true,
            last_handoff: Some("2025-11-17 10:00 UTC".into()),
            ..facts()
        };
        /// The input, the health, and the kinds expected in Now and in For information.
        type Row = (TabIn, TabCore, Health, Vec<Kind>, Vec<Kind>);
        let rows: Vec<Row> = vec![
            (
                TabIn {
                    exited: true,
                    ..tab(0)
                },
                core(facts()),
                Health::Ok,
                vec![Ended],
                vec![],
            ),
            (
                TabIn {
                    refused: true,
                    exited: true,
                    ..tab(0)
                },
                core(facts()),
                Health::Ok,
                vec![Refused],
                vec![],
            ),
            (
                TabIn { bell: 1, ..tab(0) },
                core(facts()),
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
                    ..tab(0)
                },
                core(facts()),
                Health::Ok,
                vec![],
                vec![Update],
            ),
            (tab(0), core(blocked), Health::Ok, vec![], vec![Blocked]),
            // The old quiet rule: only with the agents running.
            (tab(0), core(old.clone()), Health::Ok, vec![], vec![]),
            (
                tab(0),
                TabCore {
                    agents_running: true,
                    ..core(old)
                },
                Health::Ok,
                vec![Quiet],
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

    /// Moved from the web side's suite (E-61's "each attention rule on and off"): the edges
    /// of the quiet rule, facts not read, unique keys.
    #[test]
    fn the_quiet_rule_edges_and_missing_facts() {
        let at = |min: u64| {
            let s = NOW - min * 60;
            let days = s / 86_400;
            let (h, m) = ((s % 86_400) / 3600, (s % 3600) / 60);
            // NOW is 2025-11-17 12:00 UTC; the minutes used here stay on that day.
            assert_eq!(days, NOW / 86_400);
            format!("2025-11-17 {h:02}:{m:02} UTC")
        };
        let quiet = |ts: Option<String>, in_progress: bool| {
            let c = TabCore {
                agents_running: true,
                ..core(Attention {
                    in_progress,
                    last_handoff: ts,
                    ..facts()
                })
            };
            kinds(&run(&[(tab(0), c)], Health::Ok).now)
        };
        assert_eq!(quiet(Some(at(31)), true), [Kind::Quiet]);
        assert!(
            run(
                &[(
                    tab(0),
                    TabCore {
                        agents_running: true,
                        ..core(Attention {
                            in_progress: true,
                            last_handoff: Some(at(31)),
                            ..facts()
                        })
                    }
                )],
                Health::Ok
            )
            .now[0]
                .text
                .contains("31 min")
        );
        assert!(quiet(Some(at(29)), true).is_empty(), "29 min is not yet");
        assert!(quiet(Some(at(31)), false).is_empty(), "nothing in progress");
        assert!(
            quiet(Some("yesterday".into()), true).is_empty(),
            "not guessed at"
        );
        assert!(
            quiet(None, true).is_empty(),
            "no handoff: no time to measure"
        );
        // Facts not read: only the app's own rules.
        let l = run(
            &[(
                TabIn {
                    exited: true,
                    ..tab(0)
                },
                TabCore::default(),
            )],
            Health::Ok,
        );
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
                    bell: 1,
                    exited: true,
                    update: true,
                    ..tab(0)
                },
                busy,
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
                        bell: 2,
                        exited: true,
                        update: true,
                        ..tab(0)
                    },
                    core(b(12)),
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
                    bell: 1,
                    tab: None,
                    ..tab(0)
                },
                TabCore::default(),
            )],
            Health::Ok,
        );
        assert!(!no_core.now[0].dismissable);
    }

    #[test]
    fn a_dismissed_item_returns_when_its_text_changes() {
        let m = |line: &str| {
            core(Attention {
                mentions: vec![line.into()],
                ..facts()
            })
        };
        let t = TabIn { bell: 1, ..tab(0) };
        let (_, sent) = lists(
            &[(t.clone(), m("operator: sign it"))],
            Health::Ok,
            NOW,
            &|_, _, _| false,
        );
        assert_eq!(
            sent.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(),
            ["bell", "mention:0"]
        );
        let gone: Vec<(String, String)> = sent.iter().map(|s| (s.1.clone(), s.2.clone())).collect();
        let hidden = |_: &str, k: &str, h: &str| gone.iter().any(|(gk, gh)| gk == k && gh == h);
        // The same words: hidden. A new bell (count 2) and a new line: back.
        assert!(
            lists(
                &[(t.clone(), m("operator: sign it"))],
                Health::Ok,
                NOW,
                &hidden
            )
            .0
            .now
            .is_empty()
        );
        let back = lists(
            &[(TabIn { bell: 2, ..t }, m("operator: now the tag"))],
            Health::Ok,
            NOW,
            &hidden,
        )
        .0;
        assert_eq!(kinds(&back.now), [Kind::Bell, Kind::Mention]);
    }

    #[test]
    fn only_a_dismissable_key_the_core_sent_can_be_dismissed() {
        let a = AttentionState::new(Dismissed::open(None));
        let t = TabIn {
            bell: 1,
            exited: true,
            ..tab(0)
        };
        let c = core(Attention {
            mentions: vec!["operator".into()],
            ..facts()
        });
        let (_, sent) = lists(&[(t, c)], Health::Ok, NOW, &|_, _, _| false);
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
