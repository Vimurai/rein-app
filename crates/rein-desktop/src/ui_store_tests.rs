//! `ui_store.rs`'s tests, beside it so the module stays under the 1000-line limit (E-78).

use super::*;
use crate::config::Config;

fn cfg(set: &[&str]) -> ConfigView {
    ConfigView {
        config: Config::default(),
        path: None,
        error: None,
        set: set.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn defaults_then_ui_choices_persist() {
    let t = tempfile::tempdir().unwrap();
    let s = UiStore::open(Some(t.path().to_path_buf()));
    let v = s.view(&cfg(&[]));
    assert_eq!(v.input_style.value, "simple");
    assert_eq!(v.theme.source, "default");
    assert!(!v.first_run_done);
    s.set(&cfg(&[]), "input_style", &Value::from("modal"))
        .unwrap();
    s.set(&cfg(&[]), "theme", &Value::from("nord")).unwrap();
    s.set(&cfg(&[]), "first_run_done", &Value::from(true))
        .unwrap();
    let again = UiStore::open(Some(t.path().to_path_buf())).view(&cfg(&[]));
    assert_eq!(again.input_style.value, "modal");
    assert_eq!(
        again.theme,
        Effective {
            value: Value::from("nord"),
            source: "ui"
        }
    );
    assert!(again.first_run_done);
}

#[test]
fn config_toml_wins_and_its_fields_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let s = UiStore::open(Some(t.path().to_path_buf()));
    let c = cfg(&["theme.name", "font.size"]);
    assert_eq!(
        s.set(&c, "theme", &Value::from("nord")).unwrap_err().kind,
        "set_in_config"
    );
    assert_eq!(
        s.set(&c, "font_size", &Value::from(18)).unwrap_err().kind,
        "set_in_config"
    );
    assert_eq!(s.view(&c).theme.source, "config");
    s.set(&c, "decorations", &Value::from(false)).unwrap();
    s.set(&cfg(&[]), "font_size", &Value::from(20)).unwrap();
    assert_eq!(s.view(&cfg(&[])).font_size.value, 20);
    s.set(&cfg(&[]), "font_size", &Value::Null).unwrap();
    assert_eq!(
        s.view(&cfg(&[])).font_size.source,
        "default",
        "null clears the choice"
    );
    assert_eq!(
        s.view(&c).decorations,
        Effective {
            value: Value::from(false),
            source: "ui"
        }
    );
    // The tmux overlay settings (V6): defaults, a choice, config.toml winning.
    assert_eq!(
        s.view(&c).tmux_status,
        Effective {
            value: Value::from("hidden"),
            source: "default"
        }
    );
    s.set(&c, "tmux_status", &Value::from("app")).unwrap();
    s.set(&c, "tmux_overlay", &Value::from(false)).unwrap();
    assert_eq!(s.view(&c).tmux_status.value, "app");
    assert_eq!(s.view(&c).tmux_overlay.value, false);
    let t = cfg(&["tmux.status"]);
    assert_eq!(
        s.set(&t, "tmux_status", &Value::from("keep"))
            .unwrap_err()
            .kind,
        "set_in_config"
    );
    assert_eq!(s.view(&t).tmux_status.source, "config");
    // E-57 (V8): the tmux server — rein-app's own by default, the user's by choice, and
    // config.toml winning.
    assert_eq!(
        s.view(&c).tmux_server,
        Effective {
            value: Value::from("app"),
            source: "default"
        }
    );
    s.set(&c, "tmux_server", &Value::from("user")).unwrap();
    assert_eq!(
        s.view(&c).tmux_server,
        Effective {
            value: Value::from("user"),
            source: "ui"
        }
    );
    let t = cfg(&["tmux.server"]);
    assert_eq!(
        s.set(&t, "tmux_server", &Value::from("app"))
            .unwrap_err()
            .kind,
        "set_in_config"
    ); // E-62 (V9 component 5): close when the agents exit — on by default, off by choice,
    // config.toml winning; only a boolean.
    assert_eq!(
        s.view(&c).close_when_agents_exit,
        Effective {
            value: Value::from(true),
            source: "default"
        }
    );
    s.set(&c, "close_when_agents_exit", &Value::from(false))
        .unwrap();
    assert_eq!(s.view(&c).close_when_agents_exit.value, false);
    assert_eq!(
        s.set(&c, "close_when_agents_exit", &Value::from("no"))
            .unwrap_err()
            .kind,
        "bad_value"
    );
    let t = cfg(&["project.close_when_agents_exit"]);
    assert_eq!(
        s.set(&t, "close_when_agents_exit", &Value::from(true))
            .unwrap_err()
            .kind,
        "set_in_config"
    );
}

#[test]
fn only_listed_fields_with_valid_values() {
    let t = tempfile::tempdir().unwrap();
    let s = UiStore::open(Some(t.path().to_path_buf()));
    let c = cfg(&[]);
    for (f, v) in [
        ("input_style", Value::from("vim")),
        ("theme", Value::from("solarized-light")),
        ("font_size", Value::from(200)),
        ("decorations", Value::from("no")),
        ("tmux_overlay", Value::from("on")),
        ("tmux_status", Value::from("off")),
        ("tmux_server", Value::from("mine")),
        ("first_run_done", Value::from(1)),
        // E-63: the don't-ask list holds project paths only.
        ("update_dont_ask", Value::from("/home/me/p")),
        ("update_dont_ask", serde_json::json!(["relative/p"])),
        ("update_dont_ask", serde_json::json!(["/a\u{202e}b"])),
        ("update_dont_ask", serde_json::json!([7])),
    ] {
        assert_eq!(s.set(&c, f, &v).unwrap_err().kind, "bad_value", "{f}");
    }
    let many: Vec<String> = (0..=MAX_DONT_ASK).map(|i| format!("/p/{i}")).collect();
    assert_eq!(
        s.set(&c, "update_dont_ask", &serde_json::json!(many))
            .unwrap_err()
            .kind,
        "bad_value"
    );
    s.set(
        &c,
        "update_dont_ask",
        &serde_json::json!(["/home/me/p", "/home/me/p", "/home/me/q"]),
    )
    .unwrap();
    assert_eq!(s.view(&c).update_dont_ask, ["/home/me/p", "/home/me/q"]);
    assert_eq!(
        s.set(&c, "keys", &Value::from("x")).unwrap_err().kind,
        "bad_field"
    );
}

#[test]
fn a_failed_save_changes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let blocker = t.path().join("file");
    std::fs::write(&blocker, "").unwrap();
    let s = UiStore::open(Some(blocker.join("rein-app")));
    assert_eq!(
        s.set(&cfg(&[]), "input_style", &Value::from("modal"))
            .unwrap_err()
            .kind,
        "io"
    );
    assert_eq!(
        s.view(&cfg(&[])).input_style.value,
        "simple",
        "the live state is unchanged"
    );
}

#[test]
fn an_unreadable_file_gives_the_defaults() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join(FILE_NAME), "not json").unwrap();
    assert_eq!(
        UiStore::open(Some(t.path().to_path_buf()))
            .view(&cfg(&[]))
            .input_style
            .value,
        "simple"
    );
    std::fs::write(
        t.path().join(FILE_NAME),
        r#"{"schema":1,"input_style":"vim"}"#,
    )
    .unwrap();
    assert_eq!(
        UiStore::open(Some(t.path().to_path_buf()))
            .view(&cfg(&[]))
            .input_style
            .value,
        "simple"
    );
}

/// D-056 addendum 2 ruling 4 (E-73): the file keeps the panels switched off, so a panel
/// added later is on. Extends `the_panel_choices_are_closed_and_config_wins`.
#[test]
fn the_file_keeps_the_panels_switched_off() {
    let t = tempfile::tempdir().unwrap();
    let c = cfg(&[]);
    let s = UiStore::open(Some(t.path().to_path_buf()));
    s.set(
        &c,
        "panels_enabled",
        &serde_json::json!(["tasks", "attention"]),
    )
    .unwrap();
    let text = std::fs::read_to_string(t.path().join(FILE_NAME)).unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        file["panels_disabled"],
        serde_json::json!(["dashboard", "communication"])
    );
    assert!(file.get("panels_enabled").is_none());
    // Round trip.
    let again = UiStore::open(Some(t.path().to_path_buf())).view(&c);
    assert_eq!(
        again.panels_enabled.value,
        serde_json::json!(["tasks", "attention"])
    );
    assert_eq!(again.panels_enabled.source, "ui");
    // An id this version does not know is dropped, not a refusal of the file.
    std::fs::write(
        t.path().join(FILE_NAME),
        r#"{"schema":1,"input_style":"modal","panels_disabled":["tasks","weather"]}"#,
    )
    .unwrap();
    let v = UiStore::open(Some(t.path().to_path_buf())).view(&c);
    assert_eq!(v.input_style.value, "modal");
    assert_eq!(
        v.panels_enabled.value,
        serde_json::json!(["dashboard", "communication", "attention"])
    );
    // A repeated id or a value that is not a list refuses the file (the defaults apply).
    for bad in [
        r#"{"schema":1,"input_style":"modal","panels_disabled":["tasks","tasks"]}"#,
        r#"{"schema":1,"input_style":"modal","panels_disabled":"tasks"}"#,
    ] {
        std::fs::write(t.path().join(FILE_NAME), bad).unwrap();
        let v = UiStore::open(Some(t.path().to_path_buf())).view(&c);
        assert_eq!(v.input_style.value, "simple", "{bad}");
        assert_eq!(
            v.panels_enabled.value,
            serde_json::json!(PANEL_IDS),
            "{bad}"
        );
    }
    // config.toml still wins.
    let mut from_cfg = cfg(&["panels.enabled"]);
    from_cfg.config.panels.enabled = vec!["dashboard".into()];
    std::fs::write(
        t.path().join(FILE_NAME),
        r#"{"schema":1,"panels_disabled":["dashboard"]}"#,
    )
    .unwrap();
    let s = UiStore::open(Some(t.path().to_path_buf()));
    let v = s.view(&from_cfg);
    assert_eq!(
        (v.panels_enabled.value, v.panels_enabled.source),
        (serde_json::json!(["dashboard"]), "config")
    );
    assert_eq!(
        s.set(&from_cfg, "panels_enabled", &serde_json::json!(["tasks"]))
            .unwrap_err()
            .kind,
        "set_in_config"
    );
}

/// A ui.json written before E-73 holds the panels switched on: read once, then gone.
#[test]
fn a_stored_panels_enabled_is_read_once() {
    let t = tempfile::tempdir().unwrap();
    let c = cfg(&[]);
    let file = t.path().join(FILE_NAME);
    let open = |text: &str| {
        std::fs::write(&file, text).unwrap();
        UiStore::open(Some(t.path().to_path_buf()))
    };
    // All three that existed then: all four on.
    let s = open(r#"{"schema":1,"panels_enabled":["dashboard","communication","tasks"]}"#);
    assert_eq!(
        s.view(&c).panels_enabled.value,
        serde_json::json!(PANEL_IDS)
    );
    // Only tasks: dashboard and communication off, attention on.
    let s = open(r#"{"schema":1,"panels_enabled":["tasks"]}"#);
    assert_eq!(
        s.view(&c).panels_enabled.value,
        serde_json::json!(["tasks", "attention"])
    );
    // The old key is gone after the next save, whatever is saved.
    s.set(&c, "panel_width", &Value::from(400)).unwrap();
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(saved.get("panels_enabled").is_none());
    assert_eq!(
        saved["panels_disabled"],
        serde_json::json!(["dashboard", "communication"])
    );
    // Both keys: the new one is the answer.
    let s = open(r#"{"schema":1,"panels_enabled":["tasks"],"panels_disabled":[]}"#);
    assert_eq!(
        s.view(&c).panels_enabled.value,
        serde_json::json!(PANEL_IDS)
    );
}

/// E-77 (status.md §3): the tab status switch — kept, closed, and config.toml wins.
#[test]
fn the_tab_status_switch_is_kept_closed_and_config_wins() {
    let t = tempfile::tempdir().unwrap();
    let s = UiStore::open(Some(t.path().to_path_buf()));
    let c = cfg(&[]);
    assert_eq!(s.view(&c).tabs_status.value, Value::from("shown"));
    s.set(&c, "tabs_status", &Value::from("hidden")).unwrap();
    let again = UiStore::open(Some(t.path().to_path_buf())).view(&c);
    assert_eq!(
        (again.tabs_status.value, again.tabs_status.source),
        (Value::from("hidden"), "ui")
    );
    for value in [Value::from("off"), Value::from(false), Value::from(1)] {
        assert_eq!(
            s.set(&c, "tabs_status", &value).unwrap_err().kind,
            "bad_value",
            "{value}"
        );
    }
    let from_cfg = cfg(&["tabs.status"]);
    assert_eq!(
        s.set(&from_cfg, "tabs_status", &Value::from("shown"))
            .unwrap_err()
            .kind,
        "set_in_config"
    );
    assert_eq!(s.view(&from_cfg).tabs_status.source, "config");
    s.set(&c, "tabs_status", &Value::Null).unwrap();
    assert_eq!(s.view(&c).tabs_status.source, "default");
}

/// E-78 (status.md §Data Model): the four notification settings — kept, closed, config.toml
/// wins, and what the notifier and the `stuck` rule read from them.
#[test]
fn the_notification_settings_are_kept_closed_and_config_wins() {
    use crate::notify::Event;
    let t = tempfile::tempdir().unwrap();
    let s = UiStore::open(Some(t.path().to_path_buf()));
    let c = cfg(&[]);
    let (n, stuck) = notify_settings(&s.view(&c));
    assert_eq!((n, stuck), (crate::notify::Settings::default(), 600));
    s.set(&c, "notifications_enabled", &Value::from(false))
        .unwrap();
    s.set(&c, "notifications_system", &Value::from(false))
        .unwrap();
    s.set(
        &c,
        "notifications_events",
        &serde_json::json!(["stuck", "ended"]),
    )
    .unwrap();
    s.set(&c, "notifications_stuck_after_min", &Value::from(3))
        .unwrap();
    let again = UiStore::open(Some(t.path().to_path_buf())).view(&c);
    assert_eq!(again.notifications_system.value, Value::from(false));
    let (n, stuck) = notify_settings(&again);
    assert!(!n.enabled);
    assert_eq!(n.events, [Event::Stuck, Event::Ended]);
    assert_eq!(stuck, 180);
    for (field, value) in [
        ("notifications_enabled", Value::from("no")),
        ("notifications_events", serde_json::json!(["waiting"])),
        (
            "notifications_events",
            serde_json::json!(["ended", "ended"]),
        ),
        ("notifications_events", Value::from("ended")),
        ("notifications_stuck_after_min", Value::from(2)),
        ("notifications_stuck_after_min", Value::from(121)),
    ] {
        assert_eq!(
            s.set(&c, field, &value).unwrap_err().kind,
            "bad_value",
            "{field} {value}"
        );
    }
    let from_cfg = cfg(&["notifications.events"]);
    assert_eq!(
        s.set(&from_cfg, "notifications_events", &serde_json::json!([]))
            .unwrap_err()
            .kind,
        "set_in_config"
    );
    let v = s.view(&from_cfg);
    assert_eq!(v.notifications_events.source, "config");
    assert_eq!(notify_settings(&v).0.events, Event::ALL);
    // An empty list is allowed: every event off.
    s.set(&c, "notifications_events", &serde_json::json!([]))
        .unwrap();
    assert!(notify_settings(&s.view(&c)).0.events.is_empty());
}

/// E-60 (panels.md §Data Model): the side menu's choices and the panel's own state.
#[test]
fn the_panel_choices_are_closed_and_config_wins() {
    let t = tempfile::tempdir().unwrap();
    let s = UiStore::open(Some(t.path().to_path_buf()));
    let c = cfg(&[]);
    let v = s.view(&c);
    assert_eq!(v.panels_rail.value, Value::from("shown"));
    assert_eq!(v.panels_enabled.value, serde_json::json!(PANEL_IDS));
    assert_eq!((v.panel_width, v.panel_open), (PANEL_WIDTH_DEFAULT, None));
    s.set(&c, "panels_rail", &Value::from("hidden")).unwrap();
    s.set(&c, "panels_enabled", &serde_json::json!(["tasks"]))
        .unwrap();
    s.set(&c, "panel_width", &Value::from(480)).unwrap();
    s.set(&c, "panel_open", &Value::from("communication"))
        .unwrap();
    let again = UiStore::open(Some(t.path().to_path_buf())).view(&c);
    assert_eq!(again.panels_rail.value, Value::from("hidden"));
    assert_eq!(again.panels_enabled.value, serde_json::json!(["tasks"]));
    assert_eq!(again.panel_width, 480);
    assert_eq!(again.panel_open.as_deref(), Some("communication"));
    s.set(&c, "panel_open", &Value::Null).unwrap();
    assert_eq!(s.view(&c).panel_open, None);
    for (field, value) in [
        ("panels_rail", Value::from("left")),
        ("panels_enabled", serde_json::json!(["shell"])),
        ("panels_enabled", serde_json::json!(["tasks", "tasks"])),
        ("panels_enabled", Value::from("tasks")),
        ("panel_width", Value::from(319)),
        ("panel_width", Value::from(721)),
        ("panel_width", Value::from(4_294_967_656_u64)),
        ("panel_open", Value::from("shell")),
    ] {
        assert_eq!(
            s.set(&c, field, &value).unwrap_err().kind,
            "bad_value",
            "{field} {value}"
        );
    }
    let t2 = cfg(&["panels.rail", "panels.enabled"]);
    assert_eq!(
        s.set(&t2, "panels_rail", &Value::from("shown"))
            .unwrap_err()
            .kind,
        "set_in_config"
    );
    assert_eq!(s.view(&t2).panels_enabled.source, "config");
    // A hand-edited ui.json out of range never reaches the page.
    std::fs::write(
        t.path().join(FILE_NAME),
        r#"{"schema":1,"panel_width":9000,"panel_open":"shell"}"#,
    )
    .unwrap();
    let odd = UiStore::open(Some(t.path().to_path_buf())).view(&c);
    assert_eq!(
        (odd.panel_width, odd.panel_open),
        (PANEL_WIDTH_DEFAULT, None)
    );
}
