use super::*;
use crate::panel_sources::{Handoff, Section};

const NOW: u64 = 1_795_449_600; // 2026-11-23 16:00 UTC
const DAY: u64 = 86_400;

fn iso(t: u64) -> String {
    // Seconds → "YYYY-MM-DDTHH:MM:SS.000Z" via the civil algorithm's inverse.
    let days = (t / DAY) as i64;
    let secs = t % DAY;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

fn hts(t: u64) -> String {
    iso(t)[..16].replace('T', " ") + " UTC"
}

fn handoff(from: &str, at: u64, next: &str) -> Handoff {
    Handoff {
        timestamp: hts(at),
        from: from.into(),
        to: "Engineer".into(),
        trigger: format!("trigger from {from}"),
        sections: vec![Section {
            name: "Next action needed".into(),
            text: next.into(),
        }],
        cut: false,
    }
}

fn hs(list: Vec<Handoff>) -> Handoffs {
    Handoffs {
        handoffs: list,
        cut: false,
    }
}

fn task(id: &str, status: &str) -> Task {
    Task {
        id: id.into(),
        status: status.into(),
        description: format!("Do {id}. More words."),
        ..Default::default()
    }
}

fn tasks(list: Vec<Task>) -> Tasks {
    Tasks {
        tasks: list,
        cut: false,
        focus: String::new(),
    }
}

fn stamp(kind: &str, id: &str, at: u64) -> Stamp {
    Stamp {
        kind: kind.into(),
        task_id: id.into(),
        timestamp: iso(at),
        sha: "95388eb0123".into(),
        summary: "a ‖ b".into(),
    }
}

fn texts(h: &Handoffs) -> Vec<String> {
    todo_lines(h).into_iter().map(|(t, _)| t).collect()
}

#[test]
fn iso_times_are_read_and_odd_ones_are_not_guessed() {
    assert_eq!(iso_seconds(&iso(NOW)), Some(NOW));
    assert_eq!(iso_seconds("2026-11-23T16:00:00Z"), Some(NOW));
    assert_eq!(iso_seconds("2026-11-23 16:00:00"), Some(NOW));
    assert_eq!(iso_seconds("2026-11-23T16:00:00+00:00"), Some(NOW));
    for bad in [
        "",
        "2026-11-23",
        "2026-11-23T16:00:00+02:00",
        "2026-13-23T16:00:00Z",
        "x026-11-23T16:00:00Z",
        "2026-11-23T16:00:00.Z",
    ] {
        assert_eq!(iso_seconds(bad), None, "{bad}");
    }
}

#[test]
fn the_task_title_rule_matches_the_web_sides() {
    assert_eq!(task_title("Build the Desk. Then more."), "Build the Desk.");
    assert_eq!(task_title("v1.2 ships"), "v1.2 ships");
    let long = "word ".repeat(40);
    let t = task_title(&long);
    assert!(t.chars().count() <= TITLE && t.ends_with('…'), "{t}");
}

#[test]
fn t1_an_address_with_text_is_one_row_with_the_text_after_it() {
    let h = hs(vec![handoff(
        "Architect",
        NOW - 600,
        "- **Engineer: E-106**, then E-107.\n- **Operator:** reinstall the `window` once by hand\n- the operator — say go for E-88",
    )]);
    assert_eq!(
        texts(&h),
        ["Reinstall the window once by hand", "Say go for E-88"]
    );
}

#[test]
fn t1_an_address_with_a_nested_list_gives_one_row_per_item() {
    let h = hs(vec![handoff(
        "Engineer",
        NOW - 600,
        "- **Operator:**\n  - create the update key\n  - say go for a tag\n    continued text\n- **Architect:** review",
    )]);
    assert_eq!(texts(&h), ["Create the update key", "Say go for a tag"]);
}

#[test]
fn t1_two_senders_each_give_their_newest_handoff() {
    let h = hs(vec![
        handoff("Engineer", NOW - 3000, "- Operator: old engineer line"),
        handoff("Architect", NOW - 2000, "- Operator: architect line"),
        handoff("Engineer", NOW - 1000, "- Operator: new engineer line"),
    ]);
    let got = todo_lines(&h);
    assert_eq!(
        got,
        [
            ("New engineer line".to_string(), 2),
            ("Architect line".to_string(), 1)
        ]
    );
}

#[test]
fn t1_the_same_line_from_both_senders_is_one_row() {
    let h = hs(vec![
        handoff("Architect", NOW - 2000, "- **Operator:** say go for E-88"),
        handoff("Engineer", NOW - 1000, "- Operator: say go for E-88"),
    ]);
    assert_eq!(texts(&h), ["Say go for E-88"]);
}

#[test]
fn t1_a_line_that_only_names_the_operator_gives_none() {
    let h = hs(vec![handoff(
        "Engineer",
        NOW - 600,
        "- E-79 (operator) waits on sightings\n- the operator's keys are kept\n- ask the operator later",
    )]);
    assert!(texts(&h).is_empty(), "{:?}", texts(&h));
}

#[test]
fn t1_rows_are_capped_and_cut() {
    let many: String = (0..30)
        .map(|i| format!("- Operator: line {i} {}\n", "x".repeat(200)))
        .collect();
    let h = hs(vec![handoff("Engineer", NOW - 600, &many)]);
    let f = facts(Ok(&EMPTY), &[], Some(&h));
    let d = desk(&f, NOW, &|_, _| false);
    let todo: Vec<&Row> = d
        .for_you
        .iter()
        .filter(|r| r.kind == RowKind::Todo)
        .collect();
    assert_eq!(todo.len(), MAX_TODO);
    assert!(todo.iter().all(|r| r.text.chars().count() <= ROW_TEXT));
    assert!(todo.iter().all(|r| r.primary == Action::Done
        && r.more == [Action::Details, Action::Copy]
        && r.reference == Some(Ref::Handoff(0))));
}

static EMPTY: Tasks = Tasks {
    tasks: Vec::new(),
    cut: false,
    focus: String::new(),
};

fn facts<'a>(
    t: Result<&'a Tasks, &'static str>,
    stamps: &'a [Stamp],
    handoffs: Option<&'a Handoffs>,
) -> TabFacts<'a> {
    TabFacts {
        tab: 7,
        name: "rein-app".into(),
        tasks: t,
        stamps,
        handoffs,
        setup: false,
        ended: false,
        outdated: false,
        bell: 0,
        quiet: None,
        agents_running: false,
        stuck_after_s: 600,
        seen: None,
        lanes: vec![],
    }
}

#[test]
fn t2_a_fail_then_a_pass_gives_none() {
    let t = tasks(vec![task("E-1", "OPEN")]);
    let s = [
        stamp("ACCEPT_FAIL", "E-1", NOW - 600),
        stamp("ACCEPT_PASS", "E-1", NOW - 300),
    ];
    assert!(failed_reviews(&s, &t, NOW).is_empty());
    // The other order is a failure.
    let s = [
        stamp("ACCEPT_PASS", "E-1", NOW - 600),
        stamp("ACCEPT_FAIL", "E-1", NOW - 300),
    ];
    assert_eq!(
        failed_reviews(&s, &t, NOW),
        [("E-1".to_string(), "Acceptance", NOW - 300, 1)]
    );
}

#[test]
fn t2_older_than_14_days_gives_none() {
    let t = tasks(vec![task("E-1", "OPEN")]);
    let s = [stamp("SEC_FAIL", "E-1", NOW - 15 * DAY)];
    assert!(failed_reviews(&s, &t, NOW).is_empty());
    let s = [stamp("SEC_FAIL", "E-1", NOW - 13 * DAY)];
    assert_eq!(failed_reviews(&s, &t, NOW).len(), 1);
}

#[test]
fn t2_a_done_task_with_a_later_acceptance_gives_none() {
    let s = [
        stamp("TESTS_FAIL", "E-1", NOW - 600),
        stamp("ACCEPT_PASS", "E-1", NOW - 300),
    ];
    assert!(failed_reviews(&s, &tasks(vec![task("E-1", "DONE")]), NOW).is_empty());
    assert_eq!(
        failed_reviews(&s, &tasks(vec![task("E-1", "OPEN")]), NOW).len(),
        1
    );
}

#[test]
fn t2_one_row_per_task_naming_its_newest_failure() {
    let t = tasks(vec![task("E-1", "OPEN")]);
    let s = [
        stamp("ARCH_FAIL", "E-1", NOW - 900),
        stamp("SEC_FAIL", "E-1", NOW - 600),
        stamp("CLEAN_WARN", "E-1", NOW - 300),
        stamp("WEIRD_FAIL", "E-1", NOW - 200),
    ];
    let f = facts(Ok(&t), &s, None);
    let d = desk(&f, NOW, &|_, _| false);
    assert_eq!(d.for_you.len(), 1);
    let r = &d.for_you[0];
    assert_eq!(r.text, "E-1: Security review failed");
    assert_eq!(
        (r.primary, &r.more[..]),
        (Action::Why, &[Action::Dismiss][..])
    );
    assert_eq!(r.reference, Some(Ref::Stamp(1)));
    assert_eq!(r.age_s, Some(600));
}

#[test]
fn the_board_puts_an_open_task_with_an_open_dependency_in_blocked() {
    let mut a = task("E-1", "OPEN");
    a.depends_on = vec!["E-2".into(), "E-3".into(), "E-9".into()];
    let mut b = task("E-4", "OPEN");
    b.depends_on = vec!["E-3".into(), "E-404".into()];
    let mut c = task("E-5", "BLOCKED");
    c.blocked_by = vec!["rein E-433".into()];
    let mut t = tasks(vec![
        a,
        task("E-2", "IN_PROGRESS"),
        task("E-3", "DONE"),
        b,
        c,
        task("E-9", "OPEN"),
    ]);
    t.tasks[0].tier = "2".into();
    let b = board(&t, NOW);
    let ids = |v: &[Card]| v.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
    assert_eq!(
        ids(&b.next),
        ["E-4", "E-9"],
        "a dependency not in the list counts as done"
    );
    assert_eq!(ids(&b.blocked), ["E-1", "E-5"]);
    assert_eq!(b.blocked[0].waits_on, ["E-2", "E-9"]);
    assert_eq!(b.blocked[0].tier, Some(2));
    assert_eq!(b.blocked[1].waits_on, ["rein E-433"]);
    assert_eq!(ids(&b.doing), ["E-2"]);
    assert_eq!(ids(&b.done), ["E-3"]);
    assert_eq!(
        b.counts,
        Counts {
            next: 2,
            doing: 1,
            blocked: 2,
            done: 1
        }
    );
    assert_eq!(b.next[0].title, "Do E-4.");
}

#[test]
fn the_board_shows_the_five_newest_done_and_counts_all() {
    let list = (0..9)
        .map(|i| {
            let mut t = task(&format!("E-{i}"), "DONE");
            t.completed_at = iso(NOW - 1000 + i * 10);
            t
        })
        .collect();
    let b = board(&tasks(list), NOW);
    assert_eq!(b.counts.done, 9);
    let ids: Vec<&str> = b.done.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["E-8", "E-7", "E-6", "E-5", "E-4"]);
    assert_eq!(b.done[0].age_s, Some(920));
}

#[test]
fn activity_is_newest_first_capped_and_new_against_the_seen_time() {
    let mut list = Vec::new();
    for i in 0..40 {
        let mut t = task(&format!("E-{i}"), "DONE");
        t.completed_at = iso(NOW - 10_000 + i * 100);
        t.summary = format!("Shipped {i}. Details follow.");
        list.push(t);
    }
    let mut old = task("E-99", "DONE");
    old.completed_at = iso(NOW - 15 * DAY);
    list.push(old);
    let t = tasks(list);
    let s: Vec<Stamp> = (0..10)
        .map(|i| {
            stamp(
                if i % 2 == 0 {
                    "ACCEPT_PASS"
                } else {
                    "ARCH_FAIL"
                },
                "E-1",
                NOW - 5000 + i * 100,
            )
        })
        .chain([stamp("CRITIC_STAMP", "E-1", NOW - 10)])
        .collect();
    let h = hs(vec![handoff("Engineer", NOW - 120, "- nothing")]);
    let seen = Some(NOW - 4000);
    let a = activity(Some(&t), &s, Some(&h), seen, NOW);
    assert_eq!(a.len(), MAX_EVENTS, "40 + 10 + 1 events, capped");
    assert!(a.windows(2).all(|w| w[0].at >= w[1].at), "newest first");
    assert_eq!(a[0].kind, EventKind::Handoff);
    assert_eq!(a[0].from.as_deref(), Some("Engineer"));
    assert_eq!(a[0].reference, Some(Ref::Handoff(0)));
    assert!(a.iter().all(|e| e.new == (e.at > NOW - 4000)));
    assert!(a.iter().any(|e| !e.new) && a.iter().any(|e| e.new));
    assert!(
        !a.iter().any(|e| e.task.as_deref() == Some("E-99")),
        "older than 14 days"
    );
    let done = a.iter().find(|e| e.kind == EventKind::TaskDone).unwrap();
    assert_eq!(done.title, "Shipped 39.");
    let fail = a
        .iter()
        .find(|e| e.kind == EventKind::ReviewFailed)
        .unwrap();
    assert_eq!(
        (fail.title.as_str(), fail.commit.as_deref()),
        ("Architecture review failed", Some("95388eb"))
    );
    assert!(a.iter().any(|e| e.kind == EventKind::Accepted));
    // Never seen: everything is new.
    assert!(
        activity(Some(&t), &s, Some(&h), None, NOW)
            .iter()
            .all(|e| e.new)
    );
}

#[test]
fn the_inbox_is_ordered_by_kind_then_oldest_first_and_dismissals_hide_rows() {
    let t = tasks(vec![
        task("E-2", "IN_PROGRESS"),
        task("E-1", "OPEN"),
        task("E-3", "OPEN"),
    ]);
    let s = [
        stamp("SEC_FAIL", "E-1", NOW - 100),
        stamp("SEC_FAIL", "E-3", NOW - 900),
    ];
    let h = hs(vec![handoff(
        "Engineer",
        NOW - 600,
        "- Operator: do a thing",
    )]);
    let mut f = facts(Ok(&t), &s, Some(&h));
    f.ended = true;
    f.bell = 2;
    f.outdated = true;
    f.agents_running = true;
    f.quiet = Some((900, NOW - 900));
    let d = desk(&f, NOW, &|_, _| false);
    let kinds: Vec<RowKind> = d.for_you.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [
            RowKind::Ended,
            RowKind::ReviewFailed,
            RowKind::ReviewFailed,
            RowKind::Stalled,
            RowKind::Bell,
            RowKind::Todo,
            RowKind::Outdated
        ]
    );
    assert_eq!(
        d.for_you[1].text, "E-3: Security review failed",
        "oldest first"
    );
    assert_eq!(d.for_you[0].text, "rein-app has stopped");
    assert_eq!(d.for_you[0].primary, Action::Restart);
    assert_eq!(d.for_you[3].text, "E-2 in progress, no output for 15 min");
    let keys: std::collections::HashSet<&str> = d.for_you.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys.len(), d.for_you.len(), "keys are unique");
    assert!(d.for_you.iter().all(|r| r.key.starts_with("7/")));
    assert!(
        d.for_you.iter().all(|r| !r.text.ends_with('.')),
        "no row ends in a full stop"
    );
    // Ended and set-up rows cannot be dismissed; the rest can.
    assert!(d.for_you[0].dismissal.is_none());
    let todo = d.for_you.iter().find(|r| r.kind == RowKind::Todo).unwrap();
    let (k, h_) = todo.dismissal.clone().unwrap();
    let d2 = desk(&f, NOW, &|key, hash| key == k && hash == h_);
    assert_eq!(d2.for_you.len(), d.for_you.len() - 1);
    assert!(d2.for_you.iter().all(|r| r.kind != RowKind::Todo));
    // A new bell is a new row even after the old one was dismissed.
    let bell = d
        .for_you
        .iter()
        .find(|r| r.kind == RowKind::Bell)
        .unwrap()
        .dismissal
        .clone()
        .unwrap();
    f.bell = 3;
    let d3 = desk(&f, NOW, &|key, hash| key == bell.0 && hash == bell.1);
    assert!(d3.for_you.iter().any(|r| r.kind == RowKind::Bell));
}

#[test]
fn a_set_up_tab_says_so_and_tasks_not_available_say_why() {
    let mut f = facts(Err("Tasks are not available"), &[], None);
    f.setup = true;
    f.ended = true;
    let d = desk(&f, NOW, &|_, _| false);
    assert_eq!(d.for_you.len(), 1);
    assert_eq!(d.for_you[0].text, "rein-app needs set-up");
    assert_eq!(d.for_you[0].more, [Action::Open]);
    assert_eq!(d.board, None);
    assert_eq!(d.unavailable, Some("Tasks are not available"));
}

#[test]
fn the_app_rows_offer_update_and_check() {
    let rows = app_rows(Some("0.2.0"), Some("1.4.0"), true);
    let t: Vec<(&str, &str, Action)> = rows
        .iter()
        .map(|r| (r.key.as_str(), r.text.as_str(), r.primary))
        .collect();
    assert_eq!(
        t,
        [
            (
                "app/update/app",
                "rein-app 0.2.0 is available",
                Action::Update
            ),
            ("app/update/rein", "Rein 1.4.0 is available", Action::Update),
            (
                "app/health",
                "The installation has a problem",
                Action::Check
            )
        ]
    );
    assert_eq!(rows[0].more, [Action::Later]);
    assert!(rows[2].more.is_empty());
    assert!(app_rows(None, None, false).is_empty());
}

#[test]
fn a_hostile_stamps_list_reaches_the_desk_bounded() {
    let t = tasks(vec![task("E-1", "OPEN")]);
    let mut s: Vec<Stamp> = (0..300)
        .map(|i| stamp("SEC_FAIL", &format!("E-{i}\u{2400}"), NOW - 100))
        .collect();
    s.push(Stamp {
        timestamp: "yesterday".into(),
        ..stamp("ARCH_FAIL", "E-1", 0)
    });
    let f = facts(Ok(&t), &s, None);
    let d = desk(&f, NOW, &|_, _| false);
    assert!(d.for_you.len() <= MAX_ROWS);
    assert!(d.activity.len() <= MAX_EVENTS);
    assert!(d.for_you.iter().all(|r| r.text.chars().count() <= ROW_TEXT));
    assert!(
        !d.for_you.iter().any(|r| r.text.starts_with("E-1:")),
        "an unreadable time is no failure"
    );
    let json = serde_json::to_string(&d).unwrap();
    assert!(
        !json.contains("dismissal"),
        "the dismissal hash is never sent"
    );
}
