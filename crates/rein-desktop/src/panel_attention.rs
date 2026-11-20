//! The `attention` source (desktop panels.md §Components 6, D-056; E-61): what one tab's
//! project files say the operator may be needed for — its `BLOCKED` tasks, whether a task is
//! in progress, the last handoff's time, and the lines of its "Next action needed" section
//! that name the operator. The rules over these facts and the app's own state are the web
//! side's pure function (`attention.ts`); this is only the facts, small enough to be asked
//! for every 5 s for every open tab. Either file may be missing: its facts are then empty —
//! never a guess.

use serde::Serialize;

use crate::panel_sources::{Handoffs, Tasks, shown};

/// The most `BLOCKED` tasks given per tab (the panel's cap is 200 items in all).
pub const MAX_BLOCKED: usize = 200;
/// The most mention lines given from one handoff.
pub const MAX_MENTIONS: usize = 20;
/// Characters kept of a mention line or a task description here.
pub const LINE: usize = 300;
/// The section whose lines are looked at for the word "operator".
pub const NEXT_ACTION: &str = "next action needed";

/// Characters kept of a task id or a role name here (display only; the web side shows a task
/// id only in the `[A-Z]-[0-9]{1,6}` shape).
pub const WORD: usize = 40;

/// Characters kept of the last handoff's trigger (status.md §Home H3, E-80).
pub const TRIGGER: usize = 120;

/// The task counts a Home card shows (status.md §Home H3, E-80).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct TaskCounts {
    pub open: usize,
    pub in_progress: usize,
    pub blocked: usize,
}

/// The first task in progress: its id and description (the web side cuts the title).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InProgress {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Blocked {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Attention {
    /// `state.json` was read (false: missing, odd or team mode — no task facts).
    pub tasks_read: bool,
    pub blocked: Vec<Blocked>,
    pub in_progress: bool,
    /// The last handoff's timestamp as written (the web side parses it, or does not guess).
    pub last_handoff: Option<String>,
    /// Lines of the last handoff's "Next action needed" addressed to the operator, cleaned
    /// (R1, R2 — D-071).
    pub mentions: Vec<String>,
    /// The first task `IN_PROGRESS` (status.md §Data Model `in_progress`, E-95).
    pub in_progress_task: Option<InProgress>,
    /// The last handoff's "to": whose turn it is, as the file says (`turn`, E-95).
    pub turn: Option<String>,
    /// Open / in progress / blocked task counts; null when `state.json` was not read (E-80).
    pub counts: Option<TaskCounts>,
    /// The last handoff's trigger, one line, at most [`TRIGGER`] characters (E-80).
    pub trigger: Option<String>,
}

/// `line` without one leading list marker (`-`, `*`, `+`, or digits with `.` or `)`, each
/// followed by white space) and the white space before and after it.
fn without_list_marker(line: &str) -> &str {
    let s = line.trim_start();
    let rest = match s.strip_prefix(['-', '*', '+']) {
        Some(r) => r,
        None => {
            let r = s.trim_start_matches(|c: char| c.is_ascii_digit());
            if r.len() == s.len() {
                return s;
            }
            match r.strip_prefix(['.', ')']) {
                Some(r) => r,
                None => return s,
            }
        }
    };
    if rest.starts_with(char::is_whitespace) {
        rest.trim_start()
    } else {
        s
    }
}

/// Emphasis marks and spaces a line may open with.
fn is_mark(c: char) -> bool {
    matches!(c, '*' | '_' | '`') || c.is_whitespace()
}

/// `s` without one leading task label: an id in the `[A-Z]-[0-9]{1,6}` shape, emphasis
/// marks, then `:`. `s` itself when it does not open with one.
fn without_task_label(s: &str) -> &str {
    let mut it = s.char_indices();
    let shaped = matches!(it.next(), Some((_, c)) if c.is_ascii_uppercase())
        && matches!(it.next(), Some((_, '-')));
    if !shaped {
        return s;
    }
    let digits = &s[2..];
    let n = digits.len()
        - digits
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .len();
    if !(1..=6).contains(&n) {
        return s;
    }
    match digits[n..].trim_start_matches(is_mark).strip_prefix(':') {
        Some(rest) => rest,
        None => s,
    }
}

/// R1 (status.md "Home, second revision", D-071): true when `line` is addressed to the
/// operator — after its list marker, emphasis marks, one task label and an optional "the",
/// the next word is "operator" or "operators" (any case) and the word ends there. A line
/// that only holds the word ("E-79 (operator)", "the operator's keys") is not.
pub fn addressed_to_operator(line: &str) -> bool {
    let s = without_list_marker(line).trim_start_matches(is_mark);
    let s = without_task_label(s).trim_start_matches(is_mark);
    let s = match s.get(..4) {
        Some(the) if the.eq_ignore_ascii_case("the ") => s[4..].trim_start_matches(is_mark),
        _ => s,
    };
    let end = s.find(|c: char| !c.is_alphanumeric()).unwrap_or(s.len());
    let (word, after) = s.split_at(end);
    (word.eq_ignore_ascii_case("operator") || word.eq_ignore_ascii_case("operators"))
        && !after.starts_with(['\'', '\u{2019}'])
}

/// R2: a mention's quote — the line without its list marker, with `**`, `__` and backticks
/// removed (the words stay). Characters are removed; markdown is never interpreted.
pub fn mention_text(line: &str) -> String {
    without_list_marker(line)
        .replace("**", "")
        .replace("__", "")
        .replace('`', "")
}

pub fn attention(tasks: Option<&Tasks>, handoffs: Option<&Handoffs>) -> Attention {
    let (blocked, in_progress) = match tasks {
        Some(t) => (
            t.tasks
                .iter()
                .filter(|x| x.status == "BLOCKED")
                .take(MAX_BLOCKED)
                .map(|x| Blocked {
                    id: x.id.clone(),
                    description: shown(&x.description, LINE, false),
                })
                .collect(),
            t.tasks.iter().any(|x| x.status == "IN_PROGRESS"),
        ),
        None => (Vec::new(), false),
    };
    let last = handoffs.and_then(|h| h.handoffs.last());
    let mentions = last
        .into_iter()
        .flat_map(|h| h.sections.iter())
        .filter(|s| s.name.trim().eq_ignore_ascii_case(NEXT_ACTION))
        .flat_map(|s| s.text.lines())
        .map(str::trim)
        .filter(|l| addressed_to_operator(l))
        .take(MAX_MENTIONS)
        .map(|l| shown(&mention_text(l), LINE, false))
        .collect();
    let in_progress_task = tasks
        .and_then(|t| t.tasks.iter().find(|x| x.status == "IN_PROGRESS"))
        .map(|x| InProgress {
            id: shown(&x.id, WORD, false),
            title: shown(&x.description, LINE, false),
        });
    let turn = last
        .map(|h| h.to.trim())
        .filter(|t| !t.is_empty())
        .map(|t| shown(t, WORD, false));
    let counts = tasks.map(|t| {
        let n = |st: &str| t.tasks.iter().filter(|x| x.status == st).count();
        TaskCounts {
            open: n("OPEN"),
            in_progress: n("IN_PROGRESS"),
            blocked: n("BLOCKED"),
        }
    });
    let trigger = last
        // `shown` adds "…" after the characters it keeps: one fewer keeps the whole ≤ TRIGGER.
        .map(|h| shown(&h.trigger, TRIGGER - 1, false))
        .filter(|t| !t.trim().is_empty());
    Attention {
        counts,
        trigger,
        tasks_read: tasks.is_some(),
        blocked,
        in_progress,
        last_handoff: last.map(|h| h.timestamp.clone()),
        mentions,
        in_progress_task,
        turn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_sources::{parse_handoffs, parse_tasks};

    const STATE: &[u8] = br#"{"tasks":[
        {"id":"E-1","status":"DONE"},
        {"id":"E-2","status":"BLOCKED","description":"waits on rein"},
        {"id":"E-3","status":"IN_PROGRESS"},
        {"id":"E-4","status":"BLOCKED","description":"<script>alert(1)</script>"}]}"#;

    const COMM: &[u8] = b"## Handoff \xe2\x80\x94 2026-11-16 09:00 UTC
**From**: Architect \xe2\x86\x92 Engineer
**Trigger**: old

### Next action needed
- Operator: an old line, not the last handoff.
---
## Handoff \xe2\x80\x94 2026-11-17 14:40 UTC
**From**: Architect \xe2\x86\x92 Engineer
**Trigger**: review

### Open risks
- The operator is named here too, but not in Next action needed.

### Next action needed
- **Engineer:** E-61.
- **Operator:** choose for Windows.
- Ask the OPERATORS about it.
- cooperators and operatorship are other words.
---
";

    #[test]
    fn the_facts_come_from_both_files() {
        let t = parse_tasks(STATE).unwrap();
        let h = parse_handoffs(COMM, true);
        let a = attention(Some(&t), Some(&h));
        assert!(a.tasks_read);
        assert_eq!(
            a.blocked.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
            ["E-2", "E-4"]
        );
        // Text stays text: the web side puts it on the page as a text node.
        assert_eq!(a.blocked[1].description, "<script>alert(1)</script>");
        assert!(a.in_progress);
        assert_eq!(a.last_handoff.as_deref(), Some("2026-11-17 14:40 UTC"));
        // E-95: the first task in progress and whose turn it is (the last handoff's "to").
        assert_eq!(
            a.in_progress_task,
            Some(InProgress {
                id: "E-3".into(),
                title: String::new()
            })
        );
        assert_eq!(a.turn.as_deref(), Some("Engineer"));
        // Only the last handoff, only its "Next action needed", only a line addressed to the
        // operator (R1), cleaned (R2).
        assert_eq!(a.mentions, ["Operator: choose for Windows."]);
    }

    /// E-80 (status.md §Home H3): the task counts and the last handoff's trigger for a
    /// readable project; none for a team-mode `state.json` (no `tasks` list) — never a guess.
    #[test]
    fn the_home_card_counts_and_trigger_are_read_or_null() {
        let t = parse_tasks(STATE).unwrap();
        let h = parse_handoffs(COMM, true);
        let a = attention(Some(&t), Some(&h));
        assert_eq!(
            a.counts,
            Some(TaskCounts {
                open: 0,
                in_progress: 1,
                blocked: 2
            })
        );
        let trigger = a.trigger.clone().expect("the last handoff's trigger");
        assert!(trigger.chars().count() <= TRIGGER && !trigger.contains('\n'));
        let long = format!(
            "## Handoff \u{2014} 2026-11-16 09:00 UTC\n**From**: A \u{2192} E\n**Trigger**: {}\n---\n",
            "x".repeat(400)
        );
        let h2 = parse_handoffs(long.as_bytes(), true);
        assert_eq!(
            attention(Some(&t), Some(&h2))
                .trigger
                .map(|s| s.chars().count()),
            Some(TRIGGER),
            "cut at 120"
        );
        assert!(
            parse_tasks(br#"{"project":{"team":true}}"#).is_err(),
            "team mode"
        );
        let team = attention(None, Some(&h));
        assert_eq!(team.counts, None);
        assert!(team.trigger.is_some(), "COMM.md is still read");
    }

    #[test]
    fn a_missing_file_gives_no_facts_and_no_guess() {
        let a = attention(None, None);
        assert_eq!(
            a,
            Attention {
                tasks_read: false,
                blocked: vec![],
                in_progress: false,
                last_handoff: None,
                mentions: vec![],
                in_progress_task: None,
                turn: None,
                counts: None,
                trigger: None,
            }
        );
        let h = parse_handoffs(COMM, true);
        let a = attention(None, Some(&h));
        assert!(!a.tasks_read && a.blocked.is_empty() && a.mentions.len() == 1);
    }

    #[test]
    fn the_lists_are_capped() {
        let many = (0..300)
            .map(|i| format!(r#"{{"id":"E-{i}","status":"BLOCKED"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let t = parse_tasks(format!(r#"{{"tasks":[{many}]}}"#).as_bytes()).unwrap();
        assert_eq!(attention(Some(&t), None).blocked.len(), MAX_BLOCKED);
        let lines = "- the operator\n".repeat(50);
        let comm = format!(
            "## Handoff \u{2014} 2026-11-17 14:40 UTC\n**From**: A \u{2192} B\n\n### Next action needed\n{lines}---\n"
        );
        let h = parse_handoffs(comm.as_bytes(), true);
        assert_eq!(attention(None, Some(&h)).mentions.len(), MAX_MENTIONS);
    }

    /// R1 (status.md "Home, second revision", D-071): every example of the blueprint and its
    /// Tests list. A mention is a line addressed to the operator, not a line holding the word.
    #[test]
    fn a_mention_is_a_line_addressed_to_the_operator() {
        let long = format!("- **Operator**: {}", "x".repeat(2000));
        let table: &[(&str, bool)] = &[
            ("- **Operator**: merge #61\u{2013}#64 after #60", true),
            ("Operator \u{2014} sign the tag", true),
            ("- **E-79:** the operator installs the build", true),
            ("- **E-79**: the operator installs the build", true),
            ("1. Operator: tag v0.6.0", true),
            ("2) operator decides", true),
            ("- ***Operator***: decide", true),
            ("Operators: both of you", true),
            ("- `operator` runs it", true),
            ("- The operator signs", true),
            (&long, true),
            ("E-79 (operator)", false),
            ("tasks that wait on the operator's keys", false),
            ("operator's keys first on the line", false),
            ("operator\u{2019}s keys", false),
            (
                "- **Close E-99, then E-100.** After it the queue holds only tasks that wait: E-79 (operator), E-81 (rein's E-433), E-86..E-88 and E-6 (operator's keys).",
                false,
            ),
            ("- Ask the OPERATORS about it.", false),
            ("- **Engineer:** E-61.", false),
            ("cooperator", false),
            ("operatorship", false),
            ("E-1234567: operator", false),
            ("the", false),
            ("", false),
            ("-", false),
        ];
        for (line, want) in table {
            assert_eq!(addressed_to_operator(line), *want, "{line:?}");
        }
    }

    /// R2: the quote loses its list marker, `**`, `__` and backticks; the words stay; cut at
    /// [`LINE`] through `attention`.
    #[test]
    fn a_mention_is_cleaned_and_cut() {
        let t = mention_text("- **Operator**: merge #61\u{2013}#64 after #60");
        assert_eq!(t, "Operator: merge #61\u{2013}#64 after #60");
        assert!(!t.contains('-'), "no list dash");
        assert!(!t.contains('*'));
        assert_eq!(
            mention_text("3. __Operator__: run `rein doctor`"),
            "Operator: run rein doctor"
        );
        assert_eq!(mention_text("**Operator**: keep"), "Operator: keep");
        let comm = format!(
            "## Handoff \u{2014} 2026-11-17 14:40 UTC\n**From**: A \u{2192} B\n\n### Next action needed\n- **Operator**: {}\n---\n",
            "y".repeat(2000)
        );
        let h = parse_handoffs(comm.as_bytes(), true);
        let m = attention(None, Some(&h)).mentions;
        assert_eq!(m.len(), 1);
        assert!(m[0].starts_with("Operator: yyy") && m[0].chars().count() <= LINE + 1);
    }
}
