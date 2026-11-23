use super::*;
use crate::dismissed::Dismissed;

fn row(key: &str, kind: RowKind, primary: Action, more: Vec<Action>) -> (Row, Option<String>) {
    (
        Row {
            key: key.into(),
            kind,
            tab: Some(3),
            text: "t".into(),
            age_s: None,
            primary,
            more,
            reference: Some(Ref::Handoff(2)),
            dismissal: Some(("todo/abc".into(), "a".repeat(64))),
        },
        Some("/p".into()),
    )
}

fn no_later(_: &str) -> Result<(), IpcError> {
    panic!("Later was not offered")
}

#[test]
fn desk_act_refuses_a_key_it_did_not_send() {
    let s = DeskState::default();
    let d = Dismissed::open(None);
    s.remember(
        &[row(
            "3/todo/abc",
            RowKind::Todo,
            Action::Done,
            vec![Action::Copy],
        )],
        None,
    );
    let e = act(&s, &d, &no_later, "3/todo/xyz", Action::Done, 1).unwrap_err();
    assert_eq!(e.message, GONE);
    let e = act(&s, &d, &no_later, "9/todo/forged", Action::Open, 1).unwrap_err();
    assert_eq!(e.message, GONE);
}

#[test]
fn desk_act_refuses_an_action_the_row_did_not_offer() {
    let s = DeskState::default();
    let d = Dismissed::open(None);
    s.remember(
        &[row(
            "3/todo/abc",
            RowKind::Todo,
            Action::Done,
            vec![Action::Copy],
        )],
        None,
    );
    for a in [
        Action::Restart,
        Action::Dismiss,
        Action::Later,
        Action::Update,
        Action::Answer,
    ] {
        let e = act(&s, &d, &no_later, "3/todo/abc", a, 1).unwrap_err();
        assert_eq!(e.message, NOT_OFFERED, "{a:?}");
    }
    // The row is still there for what it does offer.
    assert_eq!(
        act(&s, &d, &no_later, "3/todo/abc", Action::Copy, 1).unwrap(),
        Act::Run {
            action: Action::Copy,
            tab: Some(3),
            reference: Some(Ref::Handoff(2))
        }
    );
}

#[test]
fn done_keeps_the_row_dismissed_and_the_key_is_spent() {
    let dir = tempfile::tempdir().unwrap();
    let s = DeskState::default();
    let d = Dismissed::open(Some(dir.path().into()));
    s.remember(
        &[row("3/todo/abc", RowKind::Todo, Action::Done, vec![])],
        None,
    );
    assert_eq!(
        act(&s, &d, &no_later, "3/todo/abc", Action::Done, 9).unwrap(),
        Act::Done
    );
    assert!(d.has("/p", "todo/abc", &"a".repeat(64)));
    // Kept on disk: a new window reads it.
    assert!(Dismissed::open(Some(dir.path().into())).has("/p", "todo/abc", &"a".repeat(64)));
    assert_eq!(
        act(&s, &d, &no_later, "3/todo/abc", Action::Done, 9)
            .unwrap_err()
            .message,
        GONE
    );
}

#[test]
fn later_on_an_update_row_records_that_version() {
    let s = DeskState::default();
    let d = Dismissed::open(None);
    let rows = crate::desk::app_rows(Some("0.2.0"), Some("1.0.0"), false);
    s.remember(
        &rows.into_iter().map(|r| (r, None)).collect::<Vec<_>>(),
        None,
    );
    let asked = std::cell::RefCell::new(Vec::new());
    let later = |w: &str| {
        asked.borrow_mut().push(w.to_string());
        Ok(())
    };
    assert_eq!(
        act(&s, &d, &later, "app/update/rein", Action::Later, 1).unwrap(),
        Act::Done
    );
    assert_eq!(
        act(&s, &d, &later, "app/update/app", Action::Later, 1).unwrap(),
        Act::Done
    );
    assert_eq!(*asked.borrow(), ["rein", "app"]);
}

#[cfg(unix)]
mod window {
    use super::*;
    use crate::local_host::LocalHost;
    use crate::pty::{ChildSpec, Events, PtyId, Size};
    use std::sync::Arc;

    struct Quiet;
    impl Events for Quiet {
        fn output(&self, _: PtyId, _: Vec<u8>) {}
        fn exit(&self, _: PtyId, _: u32) {}
    }

    fn tab_on(d: &Desktop, project: &std::path::Path) -> u32 {
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 30".into()],
            cwd: Some(project.into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        d.open_spec(&spec, project, Size { cols: 80, rows: 24 }, Arc::new(Quiet))
            .unwrap()
            .id
    }

    const NOW: u64 = 1_795_449_600; // 2026-11-23 16:00 UTC

    /// A project with `tasks` tasks, `stamps` stamps and `handoffs` handoffs.
    fn project(dir: &std::path::Path, tasks: usize, stamps: usize, handoffs: usize) {
        std::fs::create_dir_all(dir.join(".ai")).unwrap();
        let t: Vec<String> = (0..tasks)
            .map(|i| {
                let status = ["OPEN", "DONE", "IN_PROGRESS", "BLOCKED"][i % 4];
                format!(
                    r#"{{"id":"E-{i}","status":"{status}","tier":2,"description":"Build part {i} of the thing. {}","depends_on":["E-{}"],"created_at":"2026-11-20T10:00:00.000Z","updated_at":"2026-11-22T10:00:00.000Z","completed_at":"2026-11-23T1{}:00:00.000Z","summary":"Shipped {i}."}}"#,
                    "words ".repeat(60),
                    i + 1,
                    i % 6
                )
            })
            .collect();
        let s: Vec<String> = (0..stamps)
            .map(|i| {
                let kind = ["ACCEPT_PASS", "ARCH_FAIL", "SEC_PASS", "CRITIC_STAMP"][i % 4];
                format!(
                    r#"{{"type":"{kind}","task_id":"E-{}","timestamp":"2026-11-2{}T10:00:00.000Z","sha":"95388eb0123","summary":"{} ‖ 1 | PASS | x"}}"#,
                    i % tasks.max(1),
                    i % 3,
                    "s".repeat(500)
                )
            })
            .collect();
        std::fs::write(
            dir.join(".ai/state.json"),
            format!(
                r#"{{"tasks":[{}],"stamps":[{}]}}"#,
                t.join(","),
                s.join(",")
            ),
        )
        .unwrap();
        let h: String = (0..handoffs)
            .map(|i| {
                format!(
                    "## Handoff — 2026-11-23 {:02}:{:02} UTC\n**From**: {} → Architect\n**Trigger**: work {i}\n\n### Next action needed\n- **Operator:** do thing {i}\n- Engineer: next\n\n---\n",
                    i / 60 % 24,
                    i % 60,
                    if i % 2 == 0 { "Engineer" } else { "Architect" }
                )
            })
            .collect();
        std::fs::write(dir.join(".ai/COMM.md"), h).unwrap();
    }

    struct World {
        d: Desktop,
        a: AttentionState,
        ui: UiStore,
        watch: Watch,
        s: DeskState,
    }

    fn world() -> World {
        World {
            d: Desktop::new(Box::new(LocalHost::new(None))),
            a: AttentionState::new(Dismissed::open(None)),
            ui: UiStore::open(None),
            watch: Watch::new(Default::default(), None),
            s: DeskState::default(),
        }
    }

    fn ctx(w: &World) -> Ctx<'_> {
        Ctx {
            d: &w.d,
            a: &w.a,
            ui: &w.ui,
            watch: &w.watch,
            stuck_after_s: 600,
        }
    }

    #[test]
    fn desk_all_gives_each_open_tab_and_done_hides_a_row_for_good() {
        let p = tempfile::tempdir().unwrap();
        project(p.path(), 8, 8, 3);
        let w = world();
        let tab = tab_on(&w.d, p.path());
        let first = all(&ctx(&w), &w.s, NOW);
        assert_eq!(first.tabs.len(), 1);
        assert_eq!(first.tabs[0].tab, tab);
        let todo: Vec<&Row> = first
            .for_you
            .iter()
            .filter(|r| r.kind == RowKind::Todo)
            .collect();
        // Two senders: the newest handoff of each.
        assert_eq!(todo.len(), 2, "{todo:?}");
        let board = first.tabs[0].desk.board.as_ref().unwrap();
        assert_eq!(board.counts.next + board.counts.blocked, 4);
        let key = todo[0].key.clone();
        assert_eq!(
            act(&w.s, &w.a.dismissed, &no_later, &key, Action::Done, NOW).unwrap(),
            Act::Done
        );
        let again = all(&ctx(&w), &w.s, NOW);
        assert!(again.for_you.iter().all(|r| r.key != key));
        // One tab asked alone replaces only its own rows.
        let alone = one(&ctx(&w), &w.s, tab, NOW).unwrap();
        assert_eq!(alone.for_you.len(), again.tabs[0].desk.for_you.len());
        // The seen time is the project's and makes its changes old.
        w.ui.set_desk_seen(&seen_projects(&w.d, Some(tab)), NOW)
            .unwrap();
        let seen = all(&ctx(&w), &w.s, NOW);
        assert!(seen.tabs[0].desk.activity.iter().all(|e| !e.new));
        assert!(seen_projects(&w.d, Some(tab + 99)).is_empty());
    }

    /// The optimised-build time of `desk_all()` for 16 projects of 500 tasks, 300 stamps and
    /// 30 handoffs (desk.md §Execution Constraints: 150 ms on Linux). Run with
    /// `cargo test --release -p rein-desktop desk_all_time -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn desk_all_time_for_16_projects() {
        let dirs: Vec<tempfile::TempDir> = (0..16).map(|_| tempfile::tempdir().unwrap()).collect();
        for d in &dirs {
            project(d.path(), 500, 300, 30);
        }
        let w = world();
        for d in &dirs {
            tab_on(&w.d, d.path());
        }
        let t = std::time::Instant::now();
        let cold = all(&ctx(&w), &w.s, NOW);
        let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = std::time::Instant::now();
        let warm = all(&ctx(&w), &w.s, NOW);
        let warm_ms = t.elapsed().as_secs_f64() * 1000.0;
        let bytes = serde_json::to_vec(&cold).unwrap().len();
        println!(
            "desk_all 16 projects x 500 tasks, 300 stamps, 30 handoffs: cold {cold_ms:.1} ms (files read and parsed), warm {warm_ms:.1} ms (files unchanged), {} rows, {bytes} bytes",
            warm.for_you.len()
        );
        assert_eq!(cold.tabs.len(), 16);
        assert!(cold_ms < 150.0, "cold {cold_ms} ms");
    }
}
