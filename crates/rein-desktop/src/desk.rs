//! The Desk's data (desk.md §Components 1 and 3, §Data Model, D-074; E-106): for one open
//! project, the rows that need the operator (`for_you`), its tasks as a board and a feed of
//! what changed. Pure: the facts come in (`TabFacts`), the rows go out, and the core alone
//! decides each row's sentence and the actions it offers — the web side sends back a row key
//! and one of those actions (`desk_ipc`). Every project text is already cleaned and cut by the
//! parsers (`panel_sources`, `stamps`); here it is only cut again to a row's length.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::agents::Agent;
use crate::attention::{civil_seconds, handoff_seconds, span};
use crate::dismissed::text_hash;
use crate::panel_attention::{NEXT_ACTION, after_operator_address, mention_text};
use crate::panel_sources::{Handoffs, Task, Tasks, shown};
use crate::stamps::Stamp;

/// Characters of a row's sentence, an event's title.
pub const ROW_TEXT: usize = 120;
/// Characters of a card's title (the task-title rule).
pub const TITLE: usize = 90;
/// Characters of a project name in a sentence.
pub const NAME: usize = 40;
/// To-do rows from one project (T1).
pub const MAX_TODO: usize = 20;
/// Rows in one answer.
pub const MAX_ROWS: usize = 200;
/// Cards in one board column; DONE cards shown.
pub const MAX_CARDS: usize = 200;
pub const DONE_CARDS: usize = 5;
/// Ids a blocked card names.
pub const WAITS_ON: usize = 3;
/// Events in the feed, and how far back they go (also T2's window).
pub const MAX_EVENTS: usize = 50;
pub const RECENT_S: u64 = 14 * 86_400;

/// The row kinds, in the order the inbox shows them (component 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    /// Sent once rein's waiting signal is read (E-81, E-111).
    #[allow(dead_code)]
    Waiting,
    Ended,
    Setup,
    ReviewFailed,
    Stalled,
    Bell,
    Todo,
    Update,
    Outdated,
    Health,
}

/// The closed list of actions a row may offer (primary and `more`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Answer,
    Restart,
    Setup,
    Why,
    Open,
    Done,
    Update,
    Ask,
    Check,
    Dismiss,
    Details,
    Copy,
    Later,
}

/// What the reader opens on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ref {
    Task(String),
    /// The index into the tab's `handoffs` source.
    Handoff(usize),
    /// The index into the tab's stamps (the `stamps` source).
    Stamp(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// Unique in one answer; what `desk_act` names.
    pub key: String,
    pub kind: RowKind,
    /// The tab's core id (none: the installation or the app).
    pub tab: Option<u32>,
    pub text: String,
    pub age_s: Option<u32>,
    pub primary: Action,
    pub more: Vec<Action>,
    #[serde(rename = "ref")]
    pub reference: Option<Ref>,
    /// The dismissed store's (key, hash) for Done and Dismiss — never sent.
    #[serde(skip)]
    pub dismissal: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub tier: Option<u8>,
    pub age_s: Option<u32>,
    pub waits_on: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub next: usize,
    pub doing: usize,
    pub blocked: usize,
    pub done: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Board {
    pub next: Vec<Card>,
    pub doing: Vec<Card>,
    pub blocked: Vec<Card>,
    pub done: Vec<Card>,
    pub counts: Counts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    TaskDone,
    ReviewFailed,
    Accepted,
    Handoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub key: String,
    pub kind: EventKind,
    /// Seconds since the epoch.
    pub at: u64,
    pub title: String,
    pub task: Option<String>,
    pub commit: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub new: bool,
    #[serde(rename = "ref")]
    pub reference: Option<Ref>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Desk {
    pub for_you: Vec<Row>,
    pub lanes: Vec<Agent>,
    /// None when the tasks are not available (`unavailable` says why).
    pub board: Option<Board>,
    pub activity: Vec<Event>,
    pub unavailable: Option<&'static str>,
}

/// What the core knows about one open project.
pub struct TabFacts<'a> {
    pub tab: u32,
    /// The project folder's name (cleaned and cut by the caller).
    pub name: String,
    pub tasks: Result<&'a Tasks, &'static str>,
    pub stamps: &'a [Stamp],
    pub handoffs: Option<&'a Handoffs>,
    /// The folder needs set-up, or its tmux setup was refused.
    pub setup: bool,
    pub ended: bool,
    /// rein says the project is out of date.
    pub outdated: bool,
    pub bell: u32,
    /// Seconds quiet and the time of the last output.
    pub quiet: Option<(u32, u64)>,
    pub agents_running: bool,
    pub stuck_after_s: u32,
    /// The last time the operator marked this project's changes read.
    pub seen: Option<u64>,
    pub lanes: Vec<Agent>,
}

// ── time and words ──────────────────────────────────────────────────────────────────────

/// An ISO-8601 UTC time ("2026-11-23T17:41:03.442Z", or with a space, no zone or
/// "+00:00") in seconds since the epoch; any other zone or shape is not guessed at.
pub fn iso_seconds(ts: &str) -> Option<u64> {
    let b = ts.trim().as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b' ') {
        return None;
    }
    if b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = std::str::from_utf8(&b[r]).ok()?;
        s.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse().ok())?
    };
    let mut rest = &b[19..];
    if let Some(r) = rest.strip_prefix(b".") {
        let n = r.iter().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 {
            return None;
        }
        rest = &r[n..];
    }
    if !matches!(rest, b"" | b"Z" | b"z" | b"+00:00") {
        return None;
    }
    civil_seconds(
        num(0..4)?,
        num(5..7)?,
        num(8..10)?,
        num(11..13)?,
        num(14..16)?,
        num(17..19)?,
    )
}

fn age(now: u64, at: Option<u64>) -> Option<u32> {
    at.and_then(|t| now.checked_sub(t))
        .map(|s| u32::try_from(s).unwrap_or(u32::MAX))
}

/// `s` cut to `max` characters with "…" (the whole within `max`).
fn cut(s: &str, max: usize) -> String {
    shown(s, max.saturating_sub(1), false)
}

/// The task-title rule (`panel-text.ts` `taskTitle`): the description up to the first sentence
/// end or line break, at most [`TITLE`] characters, cut at a word.
pub fn task_title(description: &str) -> String {
    first_sentence(description, TITLE)
}

fn first_sentence(text: &str, max: usize) -> String {
    let line = text.split('\n').next().unwrap_or("").trim();
    let chars: Vec<char> = line.chars().collect();
    let end = chars.iter().enumerate().position(|(i, c)| {
        matches!(c, '.' | '!' | '?') && chars.get(i + 1).is_none_or(|n| n.is_whitespace())
    });
    let t: Vec<char> = match end {
        Some(i) => chars[..=i].to_vec(),
        None => chars,
    };
    if t.len() <= max {
        return t.into_iter().collect();
    }
    let room: String = t[..max - 1].iter().collect();
    let cut_at = room
        .char_indices()
        .filter(|&(_, c)| c == ' ')
        .map(|(i, _)| i)
        .next_back()
        .filter(|&i| room[..i].chars().count() >= max / 2);
    let kept = cut_at.map_or(room.as_str(), |i| &room[..i]);
    format!(
        "{}…",
        kept.trim_end_matches(|c: char| c.is_whitespace() || ",;:(—-".contains(c))
    )
}

/// A review family's name as a sentence says it, from a stamp type's prefix.
pub fn family(kind: &str) -> Option<(&'static str, &str)> {
    let (fam, outcome) = kind.split_once('_')?;
    let name = match fam {
        "ACCEPT" => "Acceptance",
        "ARCH" => "Architecture",
        "SEC" => "Security",
        "TESTS" => "Tests",
        "ALIGN" => "Blueprint",
        "CLEAN" => "Clean code",
        "VIBE" => "Look",
        _ => return None,
    };
    Some((name, outcome))
}

// ── T1: the to-do lines ─────────────────────────────────────────────────────────────────

fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn is_list_line(line: &str) -> bool {
    let s = line.trim_start();
    if let Some(r) = s.strip_prefix(['-', '*', '+']) {
        return r.starts_with(char::is_whitespace);
    }
    let r = s.trim_start_matches(|c: char| c.is_ascii_digit());
    r.len() < s.len()
        && r.strip_prefix(['.', ')'])
            .is_some_and(|x| x.starts_with(char::is_whitespace))
}

fn todo_text(s: &str) -> String {
    let t = mention_text(s);
    let t = t.trim();
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// T1: the to-do lines — (cleaned whole line, handoff index) — from the newest handoff of each
/// sender, newest sender first, at most [`MAX_TODO`], one per cleaned text.
pub fn todo_lines(h: &Handoffs) -> Vec<(String, usize)> {
    let mut senders: Vec<String> = Vec::new();
    let mut out: Vec<(String, usize)> = Vec::new();
    let push = |text: String, i: usize, out: &mut Vec<(String, usize)>| {
        if !text.is_empty() && out.len() < MAX_TODO && !out.iter().any(|(t, _)| *t == text) {
            out.push((text, i));
        }
    };
    for (i, handoff) in h.handoffs.iter().enumerate().rev() {
        let who = handoff.from.trim().to_lowercase();
        if senders.contains(&who) {
            continue;
        }
        senders.push(who);
        let sections = handoff
            .sections
            .iter()
            .filter(|s| s.name.trim().eq_ignore_ascii_case(NEXT_ACTION));
        for s in sections {
            let lines: Vec<&str> = s.text.lines().collect();
            let mut j = 0;
            while j < lines.len() {
                let line = lines[j];
                j += 1;
                let Some(rest) = after_operator_address(line) else {
                    continue;
                };
                let text = todo_text(rest);
                if !text.is_empty() {
                    push(text, i, &mut out);
                    continue;
                }
                // The address alone: its list is the deeper lines right under it.
                let base = indent(line);
                while j < lines.len() && !lines[j].trim().is_empty() && indent(lines[j]) > base {
                    if is_list_line(lines[j]) {
                        push(todo_text(lines[j]), i, &mut out);
                    }
                    j += 1;
                }
            }
        }
    }
    out
}

// ── T2: failed reviews ──────────────────────────────────────────────────────────────────

/// One failed review: (task id, family name, time, stamp index).
pub type Failed = (String, &'static str, u64, usize);

/// T2: per task, the newest failing family whose newest stamp is `…_FAIL`, under 14 days old,
/// unless the task is DONE with an `ACCEPT_PASS` later than that failure. Oldest first.
pub fn failed_reviews(stamps: &[Stamp], tasks: &Tasks, now: u64) -> Vec<Failed> {
    // (task, family) → (time, index, failed) of the newest stamp; later in the file wins a tie.
    let mut newest: HashMap<(&str, &'static str), (u64, usize, bool)> = HashMap::new();
    let mut accepted: HashMap<&str, u64> = HashMap::new();
    for (i, s) in stamps.iter().enumerate() {
        let (Some((fam, outcome)), Some(at)) = (family(&s.kind), iso_seconds(&s.timestamp)) else {
            continue;
        };
        if s.kind == "ACCEPT_PASS" {
            let a = accepted.entry(&s.task_id).or_insert(at);
            *a = (*a).max(at);
        }
        let e = newest
            .entry((&s.task_id, fam))
            .or_insert((at, i, outcome == "FAIL"));
        if at >= e.0 {
            *e = (at, i, outcome == "FAIL");
        }
    }
    let done = |id: &str| tasks.tasks.iter().any(|t| t.id == id && t.status == "DONE");
    let mut per_task: HashMap<&str, Failed> = HashMap::new();
    for ((id, fam), (at, i, failed)) in newest {
        if !failed || now.saturating_sub(at) > RECENT_S {
            continue;
        }
        if done(id) && accepted.get(id).is_some_and(|&a| a > at) {
            continue;
        }
        let row = (id.to_string(), fam, at, i);
        match per_task.get(id) {
            Some(old) if (old.2, old.3) >= (at, i) => {}
            _ => {
                per_task.insert(id, row);
            }
        }
    }
    let mut out: Vec<Failed> = per_task.into_values().collect();
    out.sort_by(|a, b| a.2.cmp(&b.2).then(a.3.cmp(&b.3)));
    out
}

// ── the board ───────────────────────────────────────────────────────────────────────────

fn card(t: &Task, at: &str, now: u64, waits_on: Vec<String>) -> Card {
    Card {
        id: cut(&t.id, NAME),
        title: task_title(&t.description),
        tier: t
            .tier
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|n| (1..=3).contains(n)),
        age_s: age(now, iso_seconds(at)),
        waits_on,
    }
}

/// The board: OPEN with every dependency done is next; OPEN with an open one, or BLOCKED, is
/// blocked (naming at most three it waits on); a dependency not in the list counts as done.
pub fn board(tasks: &Tasks, now: u64) -> Board {
    let status: HashMap<&str, &str> = tasks
        .tasks
        .iter()
        .map(|t| (t.id.as_str(), t.status.as_str()))
        .collect();
    let open_deps = |t: &Task| -> Vec<String> {
        t.depends_on
            .iter()
            .filter(|d| status.get(d.as_str()).is_some_and(|s| *s != "DONE"))
            .map(|d| cut(d, NAME))
            .collect()
    };
    let mut b = Board::default();
    let mut done: Vec<&Task> = Vec::new();
    for t in &tasks.tasks {
        match t.status.as_str() {
            "OPEN" => {
                let deps = open_deps(t);
                if deps.is_empty() {
                    b.counts.next += 1;
                    if b.next.len() < MAX_CARDS {
                        b.next.push(card(t, &t.created_at, now, vec![]));
                    }
                } else {
                    b.counts.blocked += 1;
                    if b.blocked.len() < MAX_CARDS {
                        let w = deps.into_iter().take(WAITS_ON).collect();
                        b.blocked.push(card(t, &t.created_at, now, w));
                    }
                }
            }
            "BLOCKED" => {
                b.counts.blocked += 1;
                if b.blocked.len() < MAX_CARDS {
                    let mut w = open_deps(t);
                    if w.is_empty() {
                        w = t.blocked_by.iter().map(|x| cut(x, NAME)).collect();
                    }
                    w.truncate(WAITS_ON);
                    b.blocked.push(card(t, &t.created_at, now, w));
                }
            }
            "IN_PROGRESS" => {
                b.counts.doing += 1;
                if b.doing.len() < MAX_CARDS {
                    b.doing.push(card(t, &t.updated_at, now, vec![]));
                }
            }
            "DONE" => {
                b.counts.done += 1;
                done.push(t);
            }
            _ => {}
        }
    }
    let when = |t: &Task| iso_seconds(&t.completed_at).or_else(|| iso_seconds(&t.updated_at));
    // Newest first; later in the file first on a tie.
    let mut idx: Vec<usize> = (0..done.len()).collect();
    idx.sort_by(|&a, &b| when(done[b]).cmp(&when(done[a])).then(b.cmp(&a)));
    b.done = idx
        .into_iter()
        .take(DONE_CARDS)
        .map(|i| {
            let t = done[i];
            let at = if t.completed_at.is_empty() {
                &t.updated_at
            } else {
                &t.completed_at
            };
            card(t, at, now, vec![])
        })
        .collect();
    b
}

// ── the feed ────────────────────────────────────────────────────────────────────────────

fn commit_of(sha: &str) -> Option<String> {
    let s = sha.trim();
    (s.len() >= 7 && s.bytes().all(|c| c.is_ascii_hexdigit())).then(|| s[..7].to_lowercase())
}

/// The feed: finished tasks, failed reviews, acceptances and handoffs of the last 14 days,
/// newest first, at most [`MAX_EVENTS`]; `new` when later than `seen` (never seen: all new).
pub fn activity(
    tasks: Option<&Tasks>,
    stamps: &[Stamp],
    handoffs: Option<&Handoffs>,
    seen: Option<u64>,
    now: u64,
) -> Vec<Event> {
    let recent = |at: u64| at <= now && now - at <= RECENT_S;
    let ev = |key: String, kind, at, title: String| Event {
        key,
        kind,
        at,
        title: cut(&title, ROW_TEXT),
        task: None,
        commit: None,
        from: None,
        to: None,
        new: seen.is_none_or(|s| at > s),
        reference: None,
    };
    let mut out = Vec::new();
    for t in tasks.map_or(&[][..], |t| &t.tasks[..]) {
        let Some(at) = iso_seconds(&t.completed_at).filter(|&a| recent(a)) else {
            continue;
        };
        if t.status != "DONE" {
            continue;
        }
        let what = if t.summary.trim().is_empty() {
            task_title(&t.description)
        } else {
            first_sentence(&t.summary, ROW_TEXT)
        };
        let mut e = ev(format!("done/{}", t.id), EventKind::TaskDone, at, what);
        e.task = Some(cut(&t.id, NAME));
        e.reference = Some(Ref::Task(t.id.clone()));
        out.push(e);
    }
    for (i, s) in stamps.iter().enumerate() {
        let Some(at) = iso_seconds(&s.timestamp).filter(|&a| recent(a)) else {
            continue;
        };
        let (kind, title) = match family(&s.kind) {
            Some((fam, "FAIL")) => (EventKind::ReviewFailed, format!("{fam} review failed")),
            _ if s.kind == "ACCEPT_PASS" => (EventKind::Accepted, "Accepted".to_string()),
            _ => continue,
        };
        let mut e = ev(format!("stamp/{i}/{at}"), kind, at, title);
        e.task = Some(cut(&s.task_id, NAME));
        e.commit = commit_of(&s.sha);
        e.reference = Some(Ref::Stamp(i));
        out.push(e);
    }
    for (i, h) in handoffs
        .map_or(&[][..], |h| &h.handoffs[..])
        .iter()
        .enumerate()
    {
        let Some(at) = handoff_seconds(&h.timestamp).filter(|&a| recent(a)) else {
            continue;
        };
        let title = if h.trigger.trim().is_empty() {
            "Handoff".to_string()
        } else {
            h.trigger.clone()
        };
        let mut e = ev(format!("handoff/{at}/{i}"), EventKind::Handoff, at, title);
        e.from = Some(cut(&h.from, NAME)).filter(|s| !s.is_empty());
        e.to = Some(cut(&h.to, NAME)).filter(|s| !s.is_empty());
        e.reference = Some(Ref::Handoff(i));
        out.push(e);
    }
    out.sort_by(|a, b| b.at.cmp(&a.at).then(a.kind.cmp(&b.kind)));
    out.truncate(MAX_EVENTS);
    out
}

// ── the inbox ───────────────────────────────────────────────────────────────────────────

/// Inbox order: by kind (component 3), then oldest first; a row without an age keeps its
/// place after the aged ones of its kind. At most [`MAX_ROWS`].
pub fn sort_rows(rows: &mut Vec<Row>) {
    rows.sort_by(|a, b| {
        a.kind.cmp(&b.kind).then_with(|| match (a.age_s, b.age_s) {
            (Some(x), Some(y)) => y.cmp(&x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        })
    });
    rows.truncate(MAX_ROWS);
}

struct RowSpec {
    kind: RowKind,
    suffix: String,
    text: String,
    age_s: Option<u32>,
    primary: Action,
    more: Vec<Action>,
    reference: Option<Ref>,
    /// The words a dismissal is kept for: changed words, a new row.
    hash_of: Option<Vec<String>>,
}

fn row(tab: Option<u32>, r: RowSpec) -> Row {
    let kind_key = serde_json::to_value(r.kind)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let dkey = if r.suffix.is_empty() {
        kind_key.clone()
    } else {
        format!("{kind_key}/{}", r.suffix)
    };
    // The dismissed store keeps keys of at most 64 bytes: a longer one is hashed.
    let dkey = if dkey.len() > 64 {
        format!("{kind_key}/{}", &text_hash(&[&dkey])[..16])
    } else {
        dkey
    };
    let dismissal = r.hash_of.map(|words| {
        let parts: Vec<&str> = std::iter::once(dkey.as_str())
            .chain(words.iter().map(String::as_str))
            .collect();
        (dkey.clone(), text_hash(&parts))
    });
    Row {
        key: match tab {
            Some(t) => format!("{t}/{dkey}"),
            None => format!("app/{dkey}"),
        },
        kind: r.kind,
        tab,
        text: cut(&r.text, ROW_TEXT),
        age_s: r.age_s,
        primary: r.primary,
        more: r.more,
        reference: r.reference,
        dismissal,
    }
}

/// One project's Desk at `now`. `dismissed(key, hash)`: the operator dismissed or ticked off
/// that row of this project while it said the same.
pub fn desk(f: &TabFacts, now: u64, dismissed: &dyn Fn(&str, &str) -> bool) -> Desk {
    let tab = Some(f.tab);
    let tasks = f.tasks.ok();
    let mut rows = Vec::new();
    let mut add = |r: RowSpec| rows.push(row(tab, r));
    let spec = |kind, text: String, primary, more: Vec<Action>| RowSpec {
        kind,
        suffix: String::new(),
        text,
        age_s: None,
        primary,
        more,
        reference: None,
        hash_of: None,
    };
    if f.setup {
        add(spec(
            RowKind::Setup,
            format!("{} needs set-up", f.name),
            Action::Setup,
            vec![Action::Open],
        ));
    } else if f.ended {
        add(spec(
            RowKind::Ended,
            format!("{} has stopped", f.name),
            Action::Restart,
            vec![Action::Open],
        ));
    }
    if let Some(t) = tasks {
        for (id, fam, at, i) in failed_reviews(f.stamps, t, now) {
            add(RowSpec {
                suffix: id.clone(),
                age_s: age(now, Some(at)),
                reference: Some(Ref::Stamp(i)),
                hash_of: Some(vec![fam.into(), at.to_string()]),
                ..spec(
                    RowKind::ReviewFailed,
                    format!("{id}: {fam} review failed"),
                    Action::Why,
                    vec![Action::Dismiss],
                )
            });
        }
        let doing = t.tasks.iter().find(|x| x.status == "IN_PROGRESS");
        if let (Some(task), Some((q, last))) = (doing, f.quiet)
            && f.agents_running
            && q >= f.stuck_after_s
        {
            add(RowSpec {
                age_s: Some(q),
                hash_of: Some(vec![last.to_string()]),
                ..spec(
                    RowKind::Stalled,
                    format!(
                        "{} in progress, no output for {}",
                        cut(&task.id, NAME),
                        span(u64::from(q))
                    ),
                    Action::Open,
                    vec![Action::Dismiss],
                )
            });
        }
    }
    if f.bell > 0 {
        add(RowSpec {
            hash_of: Some(vec![f.bell.to_string()]),
            ..spec(
                RowKind::Bell,
                "The terminal rang its bell".into(),
                Action::Open,
                vec![Action::Dismiss],
            )
        });
    }
    if let Some(h) = f.handoffs {
        for (text, i) in todo_lines(h) {
            let hash = text_hash(&[&text]);
            let at = h
                .handoffs
                .get(i)
                .and_then(|x| handoff_seconds(&x.timestamp));
            add(RowSpec {
                suffix: hash[..16].to_string(),
                age_s: age(now, at),
                reference: Some(Ref::Handoff(i)),
                hash_of: Some(vec![text.clone()]),
                ..spec(
                    RowKind::Todo,
                    text,
                    Action::Done,
                    vec![Action::Details, Action::Copy],
                )
            });
        }
    }
    if f.outdated {
        add(RowSpec {
            hash_of: Some(vec![]),
            ..spec(
                RowKind::Outdated,
                format!("{} is on an older Rein", f.name),
                Action::Ask,
                vec![Action::Dismiss],
            )
        });
    }
    rows.retain(|r| r.dismissal.as_ref().is_none_or(|(k, h)| !dismissed(k, h)));
    sort_rows(&mut rows);
    Desk {
        for_you: rows,
        lanes: f.lanes.clone(),
        board: tasks.map(|t| board(t, now)),
        activity: activity(tasks, f.stamps, f.handoffs, f.seen, now),
        unavailable: f.tasks.err(),
    }
}

/// The app's own rows: a newer window or Rein (not answered "Later"), a failing installation.
pub fn app_rows(app_newer: Option<&str>, rein_newer: Option<&str>, health_fail: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    let update = |suffix: &str, text: String| RowSpec {
        kind: RowKind::Update,
        suffix: suffix.into(),
        text,
        age_s: None,
        primary: Action::Update,
        more: vec![Action::Later],
        reference: None,
        hash_of: None,
    };
    if let Some(v) = app_newer {
        rows.push(row(
            None,
            update("app", format!("rein-app {v} is available")),
        ));
    }
    if let Some(v) = rein_newer {
        rows.push(row(None, update("rein", format!("Rein {v} is available"))));
    }
    if health_fail {
        rows.push(row(
            None,
            RowSpec {
                kind: RowKind::Health,
                suffix: String::new(),
                text: "The installation has a problem".into(),
                age_s: None,
                primary: Action::Check,
                more: vec![],
                reference: None,
                hash_of: None,
            },
        ));
    }
    rows
}

#[cfg(test)]
#[path = "desk_tests.rs"]
mod tests;
