//! `rein-app profiles` end to end against a temp HOME with an empty PATH (so `claude` is
//! absent): observes argument parsing, exit codes and output at the process boundary.
//! The `claude`-present path (marketplace add, plugin install, ref verification) is
//! covered with a stub `Shell` in rein-profiles' apply_and_undo tests.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};

fn rein_app(home: &Path, args: &[&str]) -> Output {
    let empty_path = home.join("empty-path");
    fs::create_dir_all(&empty_path).unwrap();
    Command::new(env!("CARGO_BIN_EXE_rein-app"))
        .arg("--home")
        .arg(home)
        .args(args)
        .env_remove("HOME")
        .env("PATH", &empty_path)
        .output()
        .unwrap()
}

fn settings(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn without_claude_hooks_and_fallback_marketplace_are_written_exit_4_and_undo_restores() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    let hooks = json!({"Stop": [{"hooks": [{"type": "command", "command": "/x/.rein/hooks/stop-hook.sh"}]}]});
    fs::create_dir_all(h.join(".claude")).unwrap();
    fs::write(
        h.join(".claude/settings.json"),
        json!({"hooks": hooks, "model": "opus"}).to_string(),
    )
    .unwrap();
    fs::create_dir_all(h.join(".claude-personal")).unwrap();
    let personal_original = "{\"permissions\":{\"deny\":[\"Bash(rm:*)\"]}}";
    fs::write(h.join(".claude-personal/settings.json"), personal_original).unwrap();

    let list = rein_app(h, &["profiles", "list"]);
    assert!(list.status.success());
    let text = String::from_utf8_lossy(&list.stdout);
    assert!(
        text.contains("default") && text.contains("personal"),
        "{text}"
    );

    let out = rein_app(h, &["profiles", "apply", "--enable", "rein-ux"]);
    assert_eq!(
        out.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("`claude` not found"));
    let personal = settings(&h.join(".claude-personal/settings.json"));
    assert_eq!(personal["hooks"], hooks);
    assert_eq!(personal["permissions"], json!({"deny": ["Bash(rm:*)"]}));
    assert_eq!(
        personal["extraKnownMarketplaces"]["rein-plugins"]["source"]["ref"], "stable",
        "D-009 fallback entry"
    );
    assert!(
        personal.get("enabledPlugins").is_none(),
        "never hand-written (D-005)"
    );
    let default = settings(&h.join(".claude/settings.json"));
    assert_eq!(
        default["hooks"], hooks,
        "the hooks source keeps its own hooks"
    );

    let undo = rein_app(h, &["profiles", "apply", "--undo"]);
    assert!(undo.status.success());
    assert_eq!(
        settings(&h.join(".claude-personal/settings.json")),
        serde_json::from_str::<Value>(personal_original).unwrap()
    );
}

#[test]
fn unparseable_settings_exit_5_and_are_left_untouched() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    fs::create_dir_all(h.join(".claude")).unwrap();
    fs::write(h.join(".claude/settings.json"), "{ broken").unwrap();
    let out = rein_app(h, &["profiles", "apply"]);
    assert_eq!(out.status.code(), Some(5));
    assert_eq!(
        fs::read_to_string(h.join(".claude/settings.json")).unwrap(),
        "{ broken"
    );
}

#[test]
fn dry_run_lists_the_claude_commands_and_writes_nothing() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let out = rein_app(
        home.path(),
        &["profiles", "apply", "--dry-run", "--enable", "all"],
    );
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("claude plugin marketplace add Vimurai/rein-plugins#stable --scope user"),
        "{text}"
    );
    assert!(
        text.contains("claude plugin install rein-data@rein-plugins --scope user"),
        "{text}"
    );
    assert!(!home.path().join(".claude/settings.json").exists());
}

#[test]
fn unknown_plugin_is_refused_before_anything_runs() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let out = rein_app(home.path(), &["profiles", "apply", "--enable", "evil;rm"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!home.path().join(".claude/settings.json").exists());
}
