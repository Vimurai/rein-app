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
    /// Lines of the last handoff's "Next action needed" holding the word operator.
    pub mentions: Vec<String>,
}

/// True when `line` holds the word "operator" (or "operators"), in any case, as a word.
pub fn names_operator(line: &str) -> bool {
    line.split(|c: char| !c.is_alphanumeric())
        .any(|w| w.eq_ignore_ascii_case("operator") || w.eq_ignore_ascii_case("operators"))
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
        .filter(|l| names_operator(l))
        .take(MAX_MENTIONS)
        .map(|l| shown(l, LINE, false))
        .collect();
    Attention {
        tasks_read: tasks.is_some(),
        blocked,
        in_progress,
        last_handoff: last.map(|h| h.timestamp.clone()),
        mentions,
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
        // Only the last handoff, only its "Next action needed", only the word as a word.
        assert_eq!(
            a.mentions,
            [
                "- **Operator:** choose for Windows.",
                "- Ask the OPERATORS about it."
            ]
        );
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
            }
        );
        let h = parse_handoffs(COMM, true);
        let a = attention(None, Some(&h));
        assert!(!a.tasks_read && a.blocked.is_empty() && a.mentions.len() == 2);
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

    #[test]
    fn the_word_is_matched_as_a_word() {
        assert!(names_operator("Operator: decide"));
        assert!(names_operator("the operator's call"));
        assert!(names_operator("ask the operators"));
        assert!(!names_operator("cooperator"));
        assert!(!names_operator("operatorship"));
        assert!(!names_operator("Engineer: build"));
    }
}
