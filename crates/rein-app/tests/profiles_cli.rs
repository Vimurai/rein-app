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
fn without_claude_the_fallback_marketplace_is_written_exit_4_hooks_untouched_and_undo_restores() {
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
    let personal_original = "{\"permissions\":{\"deny\":[\"Bash(rm:*)\"]},\"hooks\":{\"Stop\":[]}}";
    fs::write(h.join(".claude-personal/settings.json"), personal_original).unwrap();
    let default_hooks_bytes = || settings(&h.join(".claude/settings.json"))["hooks"].to_string();
    let before_default = default_hooks_bytes();

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
    // rein D-155 r3: hooks are rein install's — nothing is copied between profiles.
    assert_eq!(
        personal["hooks"],
        json!({"Stop": []}),
        "its own hooks, not default's"
    );
    assert_eq!(personal["permissions"], json!({"deny": ["Bash(rm:*)"]}));
    assert_eq!(
        personal["extraKnownMarketplaces"]["rein-plugins"]["source"]["ref"], "stable",
        "D-009 fallback entry"
    );
    assert!(
        personal.get("enabledPlugins").is_none(),
        "never hand-written (D-005)"
    );
    assert_eq!(
        default_hooks_bytes(),
        before_default,
        "default's hooks unchanged"
    );

    let undo = rein_app(h, &["profiles", "apply", "--undo"]);
    assert!(undo.status.success());
    assert_eq!(
        settings(&h.join(".claude-personal/settings.json")),
        serde_json::from_str::<Value>(personal_original).unwrap()
    );
    assert_eq!(
        default_hooks_bytes(),
        before_default,
        "undo leaves hooks alone"
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

fn accounts(home: &Path, rows: Value) {
    fs::create_dir_all(home.join(".rein")).unwrap();
    fs::write(
        home.join(".rein/accounts.json"),
        json!({ "accounts": rows }).to_string(),
    )
    .unwrap();
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// D-024, extending `without_claude_the_fallback_marketplace_is_written_exit_4_hooks_untouched_and_undo_restores`:
/// a config_dir carrying a terminal escape is skipped, and no raw ESC reaches either stream
/// of `profiles list` or `profiles apply --dry-run`.
#[test]
fn a_config_dir_with_control_bytes_is_skipped_and_never_printed_raw() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    accounts(
        h,
        json!({"evil": {"config_dir": "/h/\u{1b}[2J; rm -rf ~ #"}}),
    );
    for args in [
        &["profiles", "list"][..],
        &["profiles", "apply", "--dry-run"],
    ] {
        let out = rein_app(h, args);
        let all = [&out.stdout[..], &out.stderr[..]].concat();
        assert!(
            !all.contains(&0x1b),
            "{args:?}: raw ESC in {:?}",
            text(&out)
        );
        assert!(
            text(&out).contains("\\u{1b}"),
            "{args:?}: the warning shows it escaped"
        );
        assert!(
            !String::from_utf8_lossy(&out.stdout).contains("evil"),
            "{args:?}: not listed"
        );
    }
}

#[test]
fn the_dry_run_command_line_shell_quotes_the_config_dir() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    let odd = h.join("it's a dir");
    fs::create_dir_all(&odd).unwrap();
    accounts(h, json!({"odd": {"config_dir": odd}}));
    let out = rein_app(h, &["profiles", "apply", "--dry-run", "--profile", "odd"]);
    let quoted = format!(
        "CLAUDE_CONFIG_DIR='{}' claude ",
        odd.display().to_string().replace('\'', r"'\''")
    );
    assert!(text(&out).contains(&quoted), "{}", text(&out));
}

/// D-024: a row dropped as a duplicate is named, and does not hide the built-in `personal`.
#[test]
fn a_duplicate_row_warns_and_never_suppresses_the_built_in() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    fs::create_dir_all(h.join(".claude")).unwrap();
    fs::create_dir_all(h.join(".claude-personal")).unwrap();
    accounts(h, json!({"personal": {"config_dir": h.join(".claude")}}));
    let out = rein_app(h, &["profiles", "list"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("\"personal\"") && stderr.contains("default"),
        "names the profile holding the dir: {stderr}"
    );
    let listed = String::from_utf8_lossy(&out.stdout);
    let personal = listed.lines().find(|l| l.starts_with("personal ")).unwrap();
    assert!(personal.ends_with(".claude-personal"), "{listed}");
}

/// D-024: `apply` notes a config dir that does not look like a Claude profile.
#[test]
fn apply_notes_a_config_dir_that_is_not_a_claude_dir() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    fs::create_dir_all(h.join(".ssh")).unwrap();
    accounts(h, json!({"keys": {"config_dir": h.join(".ssh")}}));
    let out = rein_app(h, &["profiles", "apply", "--dry-run"]);
    let notes: Vec<_> = text(&out)
        .lines()
        .filter(|l| l.contains("note:"))
        .map(str::to_owned)
        .collect();
    assert_eq!(notes.len(), 1, "one note, for keys only: {notes:?}");
    assert!(
        notes[0].contains("keys") && notes[0].contains(".ssh"),
        "{notes:?}"
    );
}
