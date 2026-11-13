//! The panel data sources (desktop panels.md §Components 4, §Data Model, D-056; E-59): the
//! project's tasks from `.ai/state.json`, its handoffs from the tail of `.ai/COMM.md`, and a
//! summary of both. Pure and bounded: the input is untrusted text (agents write it, a cloned
//! repo can ship it), so every list and every text field has a cap, a shape this version does
//! not read is "not available" — never a partial guess — and every character that could hide
//! or reorder text is shown as a visible placeholder. The web side puts the result on the page
//! as text nodes only (E-60).

use std::fmt;

use serde::de::{self, Deserializer, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

use crate::project_file::Unavailable;

/// Tasks kept (the newest by `updated_at`).
pub const MAX_TASKS: usize = 500;
/// More tasks than this in `state.json` → not available (bounded memory; security review P2-2).
pub const MAX_TASKS_READ: usize = 5_000;
/// `blocked_by` entries kept per task.
pub const MAX_BLOCKED_BY: usize = 32;
/// Characters kept of a short field (id, status, owner, tier, timestamps, names).
pub const SHORT: usize = 64;
/// Characters kept of a task's description.
pub const DESCRIPTION: usize = 1024;
/// Handoffs kept (the last ones in the file).
pub const MAX_HANDOFFS: usize = 30;
/// Bytes of text kept per handoff, all sections together (panels.md §Data Model).
pub const HANDOFF_TEXT: usize = 8 * 1024;
/// The marker a cut field ends with.
pub const CUT: &str = "…";

// ── text ────────────────────────────────────────────────────────────────────────────────

/// Characters that are invisible or reorder text: the path rule's set (`project_path`) plus
/// every other format character (security review of E-59, P2-3).
pub fn is_hidden_char(c: char) -> bool {
    crate::project_path::is_unsafe_char(c)
        || matches!(
            c,
            '\u{00ad}'
                | '\u{034f}'
                | '\u{115f}'
                | '\u{1160}'
                | '\u{180e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{206a}'..='\u{206f}'
                | '\u{3164}'
                | '\u{ffa0}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0000}'..='\u{e007f}'
                | '\u{e0100}'..='\u{e01ef}'
        )
}

/// Text as the panels show it: at most `max` characters taken (on a character boundary),
/// then every hidden character made visible — a C0 control as its Unicode picture (␛ for
/// ESC), anything else as ⟨U+XXXX⟩ — and the result cut again to `max` characters, ending in
/// [`CUT`] when anything was dropped. `\n` and `\t` are kept when `lines` is true.
pub fn shown(s: &str, max: usize, lines: bool) -> String {
    let mut out = String::new();
    let mut n = 0usize;
    let mut cut = false;
    for c in s.chars() {
        let piece: String = if (c == '\n' || c == '\t') && lines {
            c.to_string()
        } else if c == '\n' || c == '\t' {
            " ".into()
        } else if is_hidden_char(c) {
            match c as u32 {
                v @ 0..=0x1f => char::from_u32(0x2400 + v).unwrap_or('?').to_string(),
                0x7f => "␡".into(),
                v => format!("⟨U+{v:04X}⟩"),
            }
        } else {
            c.to_string()
        };
        let len = piece.chars().count();
        if n + len > max {
            cut = true;
            break;
        }
        n += len;
        out.push_str(&piece);
    }
    if cut {
        out.push_str(CUT);
    }
    out
}

// ── state.json → tasks ──────────────────────────────────────────────────────────────────

/// A task as the panels show it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub status: String,
    pub owner: String,
    pub tier: String,
    pub description: String,
    pub blocked_by: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Tasks {
    pub tasks: Vec<Task>,
    /// More tasks were in the file than [`MAX_TASKS`]; the oldest were left out.
    pub cut: bool,
    /// `state.project.focus`, if any.
    pub focus: String,
}

/// A string, a number, a boolean or null — taken as text. Anything else (an object, a list)
/// is not a field this version reads.
#[derive(Default)]
struct Lenient(Option<String>);

impl<'de> Deserialize<'de> for Lenient {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Lenient;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string, number, boolean or null")
            }
            fn visit_str<E>(self, v: &str) -> Result<Lenient, E> {
                Ok(Lenient(Some(v.chars().take(SHORT * 32).collect())))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Lenient, E> {
                Ok(Lenient(Some(v.to_string())))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Lenient, E> {
                Ok(Lenient(Some(v.to_string())))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Lenient, E> {
                Ok(Lenient(Some(v.to_string())))
            }
            fn visit_bool<E>(self, v: bool) -> Result<Lenient, E> {
                Ok(Lenient(Some(v.to_string())))
            }
            fn visit_unit<E>(self) -> Result<Lenient, E> {
                Ok(Lenient(None))
            }
            fn visit_none<E>(self) -> Result<Lenient, E> {
                Ok(Lenient(None))
            }
        }
        d.deserialize_any(V)
    }
}

/// `blocked_by`: at most [`MAX_BLOCKED_BY`] entries kept, the rest skipped without storing.
#[derive(Default)]
struct BlockedBy(Vec<String>);

impl<'de> Deserialize<'de> for BlockedBy {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = BlockedBy;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list or null")
            }
            fn visit_unit<E>(self) -> Result<BlockedBy, E> {
                Ok(BlockedBy::default())
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<BlockedBy, A::Error> {
                let mut out = Vec::new();
                while out.len() < MAX_BLOCKED_BY {
                    match seq.next_element::<Lenient>()? {
                        Some(Lenient(Some(s))) => out.push(s),
                        Some(Lenient(None)) => {}
                        None => return Ok(BlockedBy(out)),
                    }
                }
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(BlockedBy(out))
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Deserialize)]
struct RawTask {
    id: Lenient,
    status: Lenient,
    #[serde(default)]
    owner: Lenient,
    #[serde(default)]
    tier: Lenient,
    #[serde(default)]
    description: Lenient,
    #[serde(default)]
    blocked_by: BlockedBy,
    #[serde(default)]
    updated_at: Lenient,
}

/// The `tasks` array, read element by element: more than [`MAX_TASKS_READ`] stops the read.
struct RawTasks(Vec<RawTask>);

impl<'de> Deserialize<'de> for RawTasks {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = RawTasks;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list of tasks")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<RawTasks, A::Error> {
                let mut out = Vec::new();
                while let Some(t) = seq.next_element::<RawTask>()? {
                    if out.len() == MAX_TASKS_READ {
                        return Err(de::Error::custom(TOO_MANY));
                    }
                    out.push(t);
                }
                Ok(RawTasks(out))
            }
        }
        d.deserialize_seq(V)
    }
}

const TOO_MANY: &str = "rein-app: too many tasks";

#[derive(Deserialize)]
struct RawProject {
    #[serde(default)]
    focus: Lenient,
}

#[derive(Deserialize)]
struct RawState {
    tasks: Option<RawTasks>,
    #[serde(default)]
    project: Option<RawProject>,
}

/// The tasks of a `state.json`. A file without a `tasks` list is a team-mode project; a task
/// whose `id` or `status` is missing, or any shape this version does not read, makes the whole
/// source not available.
pub fn parse_tasks(bytes: &[u8]) -> Result<Tasks, Unavailable> {
    let raw: RawState = serde_json::from_slice(bytes).map_err(|e| {
        if e.to_string().contains(TOO_MANY) {
            Unavailable::TooManyTasks
        } else {
            Unavailable::Shape
        }
    })?;
    let Some(RawTasks(list)) = raw.tasks else {
        return Err(Unavailable::TeamMode);
    };
    let mut tasks = Vec::with_capacity(list.len().min(MAX_TASKS));
    for t in list {
        let (Some(id), Some(status)) = (t.id.0, t.status.0) else {
            return Err(Unavailable::Shape);
        };
        let s = |v: Option<String>| shown(v.as_deref().unwrap_or(""), SHORT, false);
        tasks.push(Task {
            id: shown(&id, SHORT, false),
            status: shown(&status, SHORT, false),
            owner: s(t.owner.0),
            tier: s(t.tier.0),
            description: shown(t.description.0.as_deref().unwrap_or(""), DESCRIPTION, false),
            blocked_by: t
                .blocked_by
                .0
                .iter()
                .map(|b| shown(b, SHORT, false))
                .collect(),
            updated_at: s(t.updated_at.0),
        });
    }
    let cut = tasks.len() > MAX_TASKS;
    if cut {
        // The newest kept: ISO timestamps compare as text; the file's order breaks ties.
        let mut idx: Vec<usize> = (0..tasks.len()).collect();
        idx.sort_by(|&a, &b| {
            tasks[b]
                .updated_at
                .cmp(&tasks[a].updated_at)
                .then(b.cmp(&a))
        });
        let mut keep: Vec<usize> = idx.into_iter().take(MAX_TASKS).collect();
        keep.sort_unstable();
        let mut i = 0usize;
        tasks.retain(|_| {
            let k = keep.binary_search(&i).is_ok();
            i += 1;
            k
        });
    }
    let focus = raw
        .project
        .and_then(|p| p.focus.0)
        .map(|f| shown(&f, DESCRIPTION, false))
        .unwrap_or_default();
    Ok(Tasks { tasks, cut, focus })
}

// ── COMM.md → handoffs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Handoff {
    /// What follows "## Handoff — ", e.g. "2026-11-13 12:20 UTC".
    pub timestamp: String,
    pub from: String,
    pub to: String,
    pub trigger: String,
    pub sections: Vec<Section>,
    /// The entry had more text than [`HANDOFF_TEXT`]; the rest was left out.
    pub cut: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Handoffs {
    /// Newest last, as in the file.
    pub handoffs: Vec<Handoff>,
    /// Older handoffs were left out to keep the answer within its size (E-59 review).
    pub cut: bool,
}

const HEADING: &str = "## Handoff";

/// The handoffs in the tail of a `COMM.md`. `from_start`: the bytes begin at the file's first
/// byte; otherwise the first (partial) line is dropped, so a cut that lands inside a line can
/// never make up an entry (security review of E-59). The last [`MAX_HANDOFFS`] entries kept.
pub fn parse_handoffs(bytes: &[u8], from_start: bool) -> Handoffs {
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    let text = if from_start {
        &text[..]
    } else {
        text.split_once('\n').map_or("", |(_, rest)| rest)
    };
    // Entries begin at a line starting "## Handoff"; any other "## " line ends one.
    let mut entries: Vec<Vec<&str>> = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in text.lines() {
        if line.starts_with("## ") {
            if let Some(e) = current.take() {
                entries.push(e);
            }
            if line.starts_with(HEADING) {
                current = Some(vec![line]);
            }
        } else if let Some(e) = current.as_mut() {
            e.push(line);
        }
    }
    if let Some(e) = current.take() {
        entries.push(e);
    }
    let skip = entries.len().saturating_sub(MAX_HANDOFFS);
    Handoffs {
        handoffs: entries
            .into_iter()
            .skip(skip)
            .map(|e| handoff(&e))
            .collect(),
        cut: false,
    }
}

fn handoff(lines: &[&str]) -> Handoff {
    let heading = lines.first().copied().unwrap_or("");
    let timestamp = heading
        .trim_start_matches(HEADING)
        .trim_start_matches([' ', '—', '-'])
        .trim();
    let (mut from, mut to, mut trigger) = (String::new(), String::new(), String::new());
    let mut sections: Vec<Section> = Vec::new();
    let mut budget = HANDOFF_TEXT;
    let mut cut = false;
    let mut open: Option<(String, String)> = None;
    let close = |open: &mut Option<(String, String)>, sections: &mut Vec<Section>| {
        if let Some((name, body)) = open.take() {
            sections.push(Section {
                name,
                text: body.trim_end().to_string(),
            });
        }
    };
    for line in lines.iter().skip(1) {
        if let Some(v) = field(line, "**From**:") {
            let (a, b) = v
                .split_once('→')
                .or_else(|| v.split_once("->"))
                .unwrap_or((v, ""));
            from = shown(a.trim(), SHORT, false);
            to = shown(b.trim(), SHORT, false);
        } else if let Some(v) = field(line, "**Trigger**:") {
            trigger = shown(v.trim(), 512, false);
        } else if let Some(name) = line.strip_prefix("### ") {
            close(&mut open, &mut sections);
            // Section names count toward the entry's budget too (E-59 review), in bytes.
            let name = shown(name.trim(), SHORT, false);
            if name.len() > budget {
                cut = true;
                break;
            }
            budget -= name.len();
            open = Some((name, String::new()));
        } else if let Some((_, body)) = open.as_mut() {
            if line.trim() == "---" {
                continue; // the entry separator, not text
            }
            // The budget is bytes; a line is cut on a character boundary to what is left.
            let l = shown(line, budget, true);
            if l.len() + 1 > budget || l.ends_with(CUT) {
                cut = true;
                body.push_str(cut_bytes(&l, budget.saturating_sub(CUT.len())));
                body.push_str(CUT);
                break;
            }
            budget -= l.len() + 1;
            body.push_str(&l);
            body.push('\n');
        }
    }
    close(&mut open, &mut sections);
    Handoff {
        timestamp: shown(timestamp, SHORT, false),
        from,
        to,
        trigger,
        sections,
        cut,
    }
}

/// `s` cut to at most `max` bytes, on a character boundary.
fn cut_bytes(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.trim_start().strip_prefix(name)
}

// ── summary ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Count {
    pub status: String,
    pub n: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LastHandoff {
    pub timestamp: String,
    pub from: String,
    pub to: String,
    pub trigger: String,
}

/// What the Dashboard shows (panels.md §Components 4 `summary`). Health and the tmux server
/// are added by the web side from what the app already holds.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Summary {
    /// Counts by status, in first-seen order.
    pub counts: Vec<Count>,
    /// The first task IN_PROGRESS, if any.
    pub in_progress: Option<Task>,
    pub last_handoff: Option<LastHandoff>,
    /// The last handoff's "to": whose turn it is, as the file says (display only).
    pub whose_turn: String,
    pub focus: String,
}

pub fn summary(tasks: &Tasks, handoffs: &Handoffs) -> Summary {
    let mut counts: Vec<Count> = Vec::new();
    for t in &tasks.tasks {
        match counts.iter_mut().find(|c| c.status == t.status) {
            Some(c) => c.n += 1,
            None => counts.push(Count {
                status: t.status.clone(),
                n: 1,
            }),
        }
    }
    let last = handoffs.handoffs.last();
    Summary {
        counts,
        in_progress: tasks
            .tasks
            .iter()
            .find(|t| t.status == "IN_PROGRESS")
            .cloned(),
        last_handoff: last.map(|h| LastHandoff {
            timestamp: h.timestamp.clone(),
            from: h.from.clone(),
            to: h.to.clone(),
            trigger: h.trigger.clone(),
        }),
        whose_turn: last.map(|h| h.to.clone()).unwrap_or_default(),
        focus: tasks.focus.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(file: &str) -> Vec<u8> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the repo root")
            .to_path_buf();
        std::fs::read(root.join(".ai").join(file)).expect("this repo's own .ai file")
    }

    #[test]
    fn this_repos_own_state_json_is_read() {
        let t = parse_tasks(&repo("state.json")).unwrap();
        assert!(t.tasks.len() >= 50, "{}", t.tasks.len());
        assert!(t.tasks.iter().any(|x| x.id == "E-59"));
        assert!(
            t.tasks
                .iter()
                .all(|x| !x.id.is_empty() && !x.status.is_empty())
        );
        assert!(
            t.tasks
                .iter()
                .all(|x| x.description.chars().count() <= DESCRIPTION + 1)
        );
    }

    #[test]
    fn this_repos_own_comm_md_is_read() {
        let bytes = repo("COMM.md");
        let h = parse_handoffs(&bytes, true);
        assert_eq!(h.handoffs.len(), MAX_HANDOFFS, "the last 30 of many");
        assert!(h.handoffs.iter().all(|x| !x.timestamp.is_empty()));
        assert!(
            h.handoffs
                .iter()
                .any(|x| x.from == "Engineer" && x.to == "Architect")
        );
        assert!(
            h.handoffs
                .iter()
                .any(|x| x.sections.iter().any(|s| s.name == "Next action needed"))
        );
        // The same file read from a cut in the middle of a line gives the same last entry.
        let tail = parse_handoffs(&bytes[bytes.len() / 2..], false);
        assert_eq!(tail.handoffs.last(), h.handoffs.last());
    }

    #[test]
    fn team_mode_and_odd_shapes_are_not_available() {
        assert_eq!(
            parse_tasks(br#"{"project":{}}"#).unwrap_err(),
            Unavailable::TeamMode
        );
        assert_eq!(parse_tasks(b"not json").unwrap_err(), Unavailable::Shape);
        assert_eq!(
            parse_tasks(br#"{"tasks":{}}"#).unwrap_err(),
            Unavailable::Shape
        );
        assert_eq!(
            parse_tasks(br#"{"tasks":[{"id":"E-1"}]}"#).unwrap_err(),
            Unavailable::Shape,
            "no status"
        );
        assert_eq!(
            parse_tasks(br#"{"tasks":[{"id":"E-1","status":{}}]}"#).unwrap_err(),
            Unavailable::Shape
        );
        // Lenient where it is safe: a number tier, null fields, unknown fields.
        let t = parse_tasks(
            br#"{"tasks":[{"id":"E-1","status":"OPEN","tier":3,"owner":null,"x":[1,{"y":2}]}]}"#,
        )
        .unwrap();
        assert_eq!(t.tasks[0].tier, "3");
        assert_eq!(t.tasks[0].owner, "");
    }

    #[test]
    fn caps_hold() {
        // More than 5000 tasks: refused, not truncated silently.
        let many = format!(
            r#"{{"tasks":[{}]}}"#,
            vec![r#"{"id":"E","status":"OPEN"}"#; MAX_TASKS_READ + 1].join(",")
        );
        assert_eq!(
            parse_tasks(many.as_bytes()).unwrap_err(),
            Unavailable::TooManyTasks
        );
        // 600 tasks: the newest 500 kept, in file order.
        let list: Vec<String> = (0..600)
            .map(|i| {
                format!(
                    r#"{{"id":"E-{i}","status":"DONE","updated_at":"2026-01-01T00:{:02}:{:02}Z"}}"#,
                    i / 60,
                    i % 60
                )
            })
            .collect();
        let t = parse_tasks(format!(r#"{{"tasks":[{}]}}"#, list.join(",")).as_bytes()).unwrap();
        assert!(t.cut);
        assert_eq!(t.tasks.len(), MAX_TASKS);
        assert_eq!(t.tasks.first().unwrap().id, "E-100");
        assert_eq!(t.tasks.last().unwrap().id, "E-599");
        // blocked_by: 32 kept of a million; a long description cut with the marker.
        let blocked = vec!["\"E-1\""; 1_000_000].join(",");
        let desc = "d".repeat(100_000);
        let t = parse_tasks(format!(r#"{{"tasks":[{{"id":"E-1","status":"OPEN","blocked_by":[{blocked}],"description":"{desc}"}}]}}"#).as_bytes()).unwrap();
        assert_eq!(t.tasks[0].blocked_by.len(), MAX_BLOCKED_BY);
        assert!(t.tasks[0].description.ends_with(CUT));
        assert_eq!(t.tasks[0].description.chars().count(), DESCRIPTION + 1);
        // A handoff with a million lines keeps 8 KiB of text.
        let body = "line of text\n".repeat(1_000_000);
        let h = parse_handoffs(
            format!("## Handoff — t\n**From**: A → B\n### Next\n{body}").as_bytes(),
            true,
        );
        assert!(h.handoffs[0].cut);
        assert!(h.handoffs[0].sections[0].text.len() <= HANDOFF_TEXT);
        // Hostile: a thousand long section names and wide characters stay within the budget.
        let names: String = (0..1000)
            .map(|i| format!("### {}{i}\n界界界\n", "名".repeat(60)))
            .collect();
        let h = parse_handoffs(format!("## Handoff — t\n{names}").as_bytes(), true);
        let total: usize = h.handoffs[0]
            .sections
            .iter()
            .map(|s| s.name.len() + s.text.len() + 1)
            .sum();
        assert!(
            h.handoffs[0].cut && total <= HANDOFF_TEXT + CUT.len(),
            "{total}"
        );
    }

    /// panels.md §Security: control and bidi characters shown as visible placeholders, and
    /// markup stays text (the web side renders text nodes; nothing here interprets it).
    #[test]
    fn hidden_characters_are_shown_and_markup_stays_text() {
        assert_eq!(
            shown("a\u{202e}b\u{1b}[31mc\u{200b}d\u{e0041}", 64, false),
            "a⟨U+202E⟩b␛[31mc⟨U+200B⟩d⟨U+E0041⟩"
        );
        assert_eq!(
            shown("x\u{2028}y\u{feff}\u{ad}", 64, false),
            "x⟨U+2028⟩y⟨U+FEFF⟩⟨U+00AD⟩"
        );
        assert_eq!(shown("a\nb\tc", 64, false), "a b c");
        assert_eq!(shown("a\nb", 64, true), "a\nb");
        // The cap counts the placeholders: never more than `max` characters plus the marker.
        let s = shown(&"\u{202e}".repeat(100), 20, false);
        assert!(s.chars().count() <= 21 && s.ends_with(CUT), "{s}");
        let h = parse_handoffs(
            b"## Handoff \xe2\x80\x94 t\n**From**: <script>x</script> \xe2\x86\x92 [a](http://x)\n### S\n<img src=x onerror=1>\n",
            true,
        );
        assert_eq!(h.handoffs[0].from, "<script>x</script>");
        assert_eq!(h.handoffs[0].to, "[a](http://x)");
        assert_eq!(h.handoffs[0].sections[0].text, "<img src=x onerror=1>");
    }

    #[test]
    fn a_tail_cut_inside_a_line_makes_up_no_entry() {
        let bytes = b"xx ## Handoff \xe2\x80\x94 fake\nnot a heading\n## Handoff \xe2\x80\x94 real\n**From**: A \xe2\x86\x92 B\n";
        let h = parse_handoffs(bytes, false);
        assert_eq!(h.handoffs.len(), 1);
        assert_eq!(h.handoffs[0].timestamp, "real");
        // CR LF line ends read the same.
        let crlf = parse_handoffs(b"## Handoff \xe2\x80\x94 t\r\n**Trigger**: go\r\n", true);
        assert_eq!(crlf.handoffs[0].trigger, "go");
    }

    #[test]
    fn the_summary_is_counts_progress_and_whose_turn() {
        let t = parse_tasks(br#"{"project":{"focus":"E-59"},"tasks":[{"id":"E-1","status":"DONE"},{"id":"E-2","status":"IN_PROGRESS"},{"id":"E-3","status":"DONE"}]}"#).unwrap();
        let h = parse_handoffs(
            "## Handoff — 1\n**From**: Architect → Engineer\n**Trigger**: plan\n".as_bytes(),
            true,
        );
        let s = summary(&t, &h);
        assert_eq!(
            s.counts,
            [
                Count {
                    status: "DONE".into(),
                    n: 2
                },
                Count {
                    status: "IN_PROGRESS".into(),
                    n: 1
                }
            ]
        );
        assert_eq!(s.in_progress.unwrap().id, "E-2");
        assert_eq!(s.whose_turn, "Engineer");
        assert_eq!(s.focus, "E-59");
    }

    /// panels.md §Execution Constraints: a 2 MiB state.json and a 512 KiB COMM.md tail parse
    /// well inside 100 ms (measured here, printed; the bound is generous for CI runners).
    #[test]
    fn parsing_is_fast_enough() {
        let task = r#"{"id":"E-1","owner":"Engineer (Claude)","status":"DONE","tier":"2","description":"DESC","created_at":"2026-11-13T11:44:45.253Z","blocked_by":[],"updated_at":"2026-11-13T11:44:45.253Z"}"#
            .replace("DESC", &"x".repeat(1800));
        let n = 2 * 1024 * 1024 / task.len();
        let state = format!(r#"{{"tasks":[{}]}}"#, vec![task.as_str(); n].join(","));
        let comm = "## Handoff — t\n**From**: A → B\n### S\nsome text in a section\n"
            .repeat(512 * 1024 / 60);
        let t0 = std::time::Instant::now();
        let _ = parse_tasks(state.as_bytes());
        let _ = parse_handoffs(comm.as_bytes(), true);
        let ms = t0.elapsed().as_millis();
        println!(
            "parse of {} KiB state.json + {} KiB COMM.md: {ms} ms",
            state.len() / 1024,
            comm.len() / 1024
        );
        assert!(ms < 1000, "{ms} ms");
    }
}
