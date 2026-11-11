//! File-level behaviour of `enumerate`, `apply`, `undo` and `set_plugins` against a temp
//! HOME (blueprint §Components 6, §Rollback Plan; D-002, D-005). Includes the E-4
//! security-review PoCs as regressions. Never touches the live marketplace or `claude`.

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use rein_profiles::{
    AllowedKey, ApplyError, ApplyOptions, Patch, PluginAction, PluginError, Profile, Shell,
    ShellOutput, apply, enumerate, register_marketplace, rein_plugins, set_plugins, undo,
};
use serde_json::{Value, json};

const WRITE: ApplyOptions = ApplyOptions { dry_run: false };

fn registration() -> Patch {
    Patch::new().set(
        AllowedKey::ExtraKnownMarketplaces,
        rein_plugins::marketplace_entry(),
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn default_profile(home: &Path) -> Profile {
    enumerate(home).profiles.remove(0)
}

#[test]
fn enumerate_lists_default_personal_and_valid_accounts_once() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    fs::create_dir_all(h.join(".claude-personal")).unwrap();
    write(
        &h.join(".rein/accounts.json"),
        &json!({"accounts": {
            "work": {"config_dir": h.join("work-claude"), "token_ref": "opaque"},
            "dup": {"config_dir": h.join(".claude")},
            "rel": {"config_dir": "relative/dir"},
            "none": {},
            "default": {"config_dir": h.join("x")},
            "bad\u{1b}[31m": {"config_dir": h.join("y")}
        }, "other": 1})
        .to_string(),
    );
    let e = enumerate(h);
    let names: Vec<&str> = e.profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["default", "work", "personal"],
        "rows before the built-in (D-022)"
    );
    // rel, none, bad name, reserved default, and the duplicate `dup` (D-024).
    assert_eq!(e.warnings.len(), 5, "{:?}", e.warnings);
}

/// D-022, extending `enumerate_lists_default_personal_and_valid_accounts_once`: the report
/// from the operator's machine — a `personal` row at `~/.claude-personal` was warned about as
/// reserved, then listed anyway (through the built-in).
#[test]
fn a_personal_row_is_an_ordinary_account_and_beats_the_built_in() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    fs::create_dir_all(h.join(".claude-personal")).unwrap();
    let rows = |personal: &Path| {
        write(
            &h.join(".rein/accounts.json"),
            &json!({"accounts": {"personal": {"config_dir": personal, "added": "2026-11-10"}}})
                .to_string(),
        );
        let e = enumerate(h);
        let listed: Vec<(String, std::path::PathBuf)> = e
            .profiles
            .iter()
            .map(|p| (p.name.clone(), p.config_dir.clone()))
            .collect();
        (listed, e.warnings)
    };

    let (listed, warnings) = rows(&h.join(".claude-personal"));
    assert_eq!(
        listed,
        [
            ("default".into(), h.join(".claude")),
            ("personal".into(), h.join(".claude-personal"))
        ]
    );
    assert!(
        warnings.is_empty(),
        "no warning for a listed row: {warnings:?}"
    );

    let (listed, warnings) = rows(&h.join("elsewhere"));
    assert_eq!(
        listed,
        [
            ("default".into(), h.join(".claude")),
            ("personal".into(), h.join("elsewhere"))
        ],
        "the row's config_dir wins; the built-in is not listed under that name"
    );
    assert!(warnings.is_empty(), "{warnings:?}");

    fs::remove_file(h.join(".rein/accounts.json")).unwrap();
    let names: Vec<String> = enumerate(h).profiles.into_iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        ["default", "personal"],
        "no row: the built-in is listed"
    );

    // An unusable `personal` row is warned about and does not hide the built-in.
    write(
        &h.join(".rein/accounts.json"),
        &json!({"accounts": {"personal": {"config_dir": "relative"}}}).to_string(),
    );
    let e = enumerate(h);
    let listed: Vec<_> = e
        .profiles
        .iter()
        .map(|p| (p.name.as_str(), p.config_dir.clone()))
        .collect();
    assert_eq!(
        listed,
        [
            ("default", h.join(".claude")),
            ("personal", h.join(".claude-personal"))
        ]
    );
    assert_eq!(e.warnings.len(), 1, "{:?}", e.warnings);
}

#[test]
fn row_names_get_a_terminal_safety_check_not_a_grammar() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    write(
        &h.join(".rein/accounts.json"),
        &json!({"accounts": {
            "client-a2": {"config_dir": h.join("a")},
            "Work.Old_1": {"config_dir": h.join("b")},
            "bad\u{1b}[2J": {"config_dir": h.join("c")},
            "has space": {"config_dir": h.join("d")},
            "default": {"config_dir": h.join(".claude")}
        }})
        .to_string(),
    );
    let e = enumerate(h);
    let mut names: Vec<&str> = e.profiles.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Work.Old_1", "client-a2", "default"]);
    assert_eq!(e.warnings.len(), 2, "{:?}", e.warnings);
    assert!(
        e.warnings
            .iter()
            .all(|w| !w.bytes().any(|b| b < 0x20 || b == 0x7f)),
        "no raw control byte: {:?}",
        e.warnings
    );
    assert!(
        e.warnings.iter().any(|w| w.contains("\\u{1b}")),
        "{:?}",
        e.warnings
    );
}

#[test]
fn apply_backs_up_once_and_undo_restores_rein_keys_only() {
    let home = tempfile::tempdir().unwrap();
    let original = "{\n  \"permissions\": {\"allow\": [\"Bash(*)\"]},\n  \"model\": \"opus\"\n}\n";
    write(&home.path().join(".claude/settings.json"), original);
    let p = default_profile(home.path());
    let backup = p.config_dir.join("settings.json.rein-bak");

    assert!(apply(&p, &registration(), WRITE).unwrap().written);
    assert_eq!(fs::read_to_string(&backup).unwrap(), original);
    // A second, different write keeps the pre-Rein backup (security review P1, PoC4).
    let more = registration().set(AllowedKey::Attribution, json!({"commit": ""}));
    assert!(apply(&p, &more, WRITE).unwrap().written);
    assert_eq!(fs::read_to_string(&backup).unwrap(), original);

    // The operator revokes Bash(*) after apply; undo must not re-grant it (PoC1).
    let mut now = read(&p.settings_path);
    now["permissions"] = json!({"allow": []});
    write(&p.settings_path, &now.to_string());
    undo(&p).unwrap();
    let after = read(&p.settings_path);
    assert_eq!(after["permissions"], json!({"allow": []}));
    assert!(after.get("extraKnownMarketplaces").is_none());
    assert!(after.get("attribution").is_none());
    assert_eq!(after["model"], "opus");
    assert!(!backup.exists());
    assert!(matches!(undo(&p), Err(ApplyError::NoBackup)));
}

#[test]
fn hooks_are_never_written_and_undo_keeps_what_rein_install_wrote_since() {
    // rein D-155 r3: `hooks` belong to `rein install`. A backup taken before it changed them
    // must not bring the old value back, and apply never touches them.
    let home = tempfile::tempdir().unwrap();
    let original = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"old"}]}]}}"#;
    write(&home.path().join(".claude/settings.json"), original);
    let p = default_profile(home.path());
    assert!(apply(&p, &registration(), WRITE).unwrap().written);
    assert_eq!(read(&p.settings_path)["hooks"], read_str(original)["hooks"]);

    let mut now = read(&p.settings_path);
    let newer = json!({"Stop": [{"hooks": [{"type": "command", "command": "new"}]}]});
    now["hooks"] = newer.clone(); // rein install rewrote them after apply
    write(&p.settings_path, &now.to_string());
    undo(&p).unwrap();
    assert_eq!(read(&p.settings_path)["hooks"], newer);
}

fn read_str(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn planted_backup_cannot_inject_forbidden_keys() {
    // Security review P0, PoC2.
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".claude/settings.json"),
        "{\"model\":\"opus\"}",
    );
    write(
        &home.path().join(".claude/settings.json.rein-bak"),
        r#"{"permissions":{"defaultMode":"bypassPermissions"},"defaultMode":"bypassPermissions","autoMode":true}"#,
    );
    let p = default_profile(home.path());
    undo(&p).unwrap();
    let after = read(&p.settings_path);
    assert_eq!(after, json!({"model": "opus"}));
}

#[test]
fn undo_of_a_created_file_keeps_content_added_later() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let p = default_profile(home.path());
    apply(&p, &registration(), WRITE).unwrap();
    let mut now = read(&p.settings_path);
    now["model"] = json!("sonnet"); // written by the user after apply
    write(&p.settings_path, &now.to_string());
    undo(&p).unwrap();
    assert_eq!(read(&p.settings_path), json!({"model": "sonnet"}));

    // With nothing else added, undo removes the file it created.
    let fresh = tempfile::tempdir().unwrap();
    fs::create_dir_all(fresh.path().join(".claude")).unwrap();
    let q = default_profile(fresh.path());
    apply(&q, &registration(), WRITE).unwrap();
    undo(&q).unwrap();
    assert!(!q.settings_path.exists());
}

#[test]
fn invalid_json_is_refused_and_left_untouched() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".claude/settings.json"), "{ not json");
    let p = default_profile(home.path());
    assert!(matches!(
        apply(&p, &registration(), WRITE),
        Err(ApplyError::Parse(_))
    ));
    assert_eq!(fs::read_to_string(&p.settings_path).unwrap(), "{ not json");
    assert!(!p.config_dir.join("settings.json.rein-bak").exists());
}

#[test]
fn dry_run_and_missing_config_dir_write_nothing() {
    let home = tempfile::tempdir().unwrap();
    let p = default_profile(home.path()); // ~/.claude absent
    let dry = ApplyOptions { dry_run: true };
    assert!(apply(&p, &registration(), dry).unwrap().written);
    assert!(!p.config_dir.exists());
    assert!(matches!(
        apply(&p, &registration(), WRITE),
        Err(ApplyError::Io(_))
    ));
    assert!(!p.config_dir.exists());
}

#[cfg(unix)]
#[test]
fn symlink_escaping_the_profile_is_refused_and_one_inside_is_followed() {
    let home = tempfile::tempdir().unwrap();
    let outside = home.path().join("dotfiles/claude-settings.json");
    write(&outside, "{}");
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let link = home.path().join(".claude/settings.json");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let p = default_profile(home.path());
    assert!(matches!(
        apply(&p, &registration(), WRITE),
        Err(ApplyError::SymlinkOutsideProfile(_))
    ));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "{}");

    // A link to another file in the same profile dir keeps working, and stays a link.
    fs::remove_file(&link).unwrap();
    let inside = home.path().join(".claude/settings.real.json");
    write(&inside, "{}");
    std::os::unix::fs::symlink(&inside, &link).unwrap();
    apply(&p, &registration(), WRITE).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(read(&inside).get("extraKnownMarketplaces").is_some());
}

#[cfg(unix)]
#[test]
fn private_settings_keep_their_mode() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(".claude/settings.json");
    write(&path, "{}");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let p = default_profile(home.path());
    apply(&p, &registration(), WRITE).unwrap();
    for f in [&path, &p.config_dir.join("settings.json.rein-bak")] {
        assert_eq!(
            fs::metadata(f).unwrap().permissions().mode() & 0o777,
            0o600,
            "{}",
            f.display()
        );
    }
}

/// One recorded call: argv and environment set.
type Call = (Vec<String>, Vec<(String, OsString)>);

/// Records every call; simulates Claude Code writing `enabledPlugins` on success.
struct StubShell {
    calls: RefCell<Vec<Call>>,
    exit: Option<i32>,
    simulate_write: bool,
    missing: bool,
    wrong_ref: Option<&'static str>,
    /// A hostile or buggy `claude` that also writes a forbidden key.
    inject_forbidden: bool,
}

impl Shell for StubShell {
    fn run(
        &self,
        program: &str,
        args: &[String],
        set_env: &[(String, OsString)],
        _remove: &[&str],
    ) -> std::io::Result<ShellOutput> {
        assert_eq!(program, "claude");
        if self.missing {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no claude",
            ));
        }
        self.calls
            .borrow_mut()
            .push((args.to_vec(), set_env.to_vec()));
        if self.simulate_write && self.exit == Some(0) {
            let dir = Path::new(&set_env[0].1).to_path_buf();
            let path = dir.join("settings.json");
            let mut doc = fs::read(&path)
                .map(|b| serde_json::from_slice(&b).unwrap())
                .unwrap_or(json!({}));
            if args[1] == "marketplace" {
                // What Claude Code 2.1.319 writes for `owner/repo#ref` (probed 2026-11-08).
                let (repo, r) = args[3].split_once('#').unwrap();
                let r = self.wrong_ref.unwrap_or(r);
                doc["extraKnownMarketplaces"]["rein-plugins"] =
                    json!({"source": {"source": "github", "repo": repo, "ref": r}});
            } else {
                doc["enabledPlugins"][&args[2]] = json!(args[1] == "install");
            }
            if self.inject_forbidden {
                doc["permissions"] = json!({"defaultMode": "bypassPermissions"});
            }
            fs::write(&path, doc.to_string()).unwrap();
        }
        Ok(ShellOutput {
            code: self.exit,
            stderr: "boom".into(),
        })
    }
}

fn stub(exit: i32, simulate_write: bool) -> StubShell {
    StubShell {
        calls: RefCell::default(),
        exit: Some(exit),
        simulate_write,
        missing: false,
        wrong_ref: None,
        inject_forbidden: false,
    }
}

#[test]
fn marketplace_is_registered_by_claude_and_verified_never_patched() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".claude/settings.json"), "{}");
    let p = default_profile(home.path());
    let shell = stub(0, true);
    register_marketplace(&shell, &p).unwrap();
    assert_eq!(
        shell.calls.borrow()[0].0,
        [
            "plugin",
            "marketplace",
            "add",
            "Vimurai/rein-plugins#stable",
            "--scope",
            "user"
        ]
    );
    assert_eq!(
        read(&p.settings_path)["extraKnownMarketplaces"],
        rein_plugins::marketplace_entry()
    );

    // Claude Code writes a different ref: reported (CLI exit 7), left as Claude wrote it.
    let wrong = StubShell {
        wrong_ref: Some("main"),
        ..stub(0, true)
    };
    assert!(matches!(
        register_marketplace(&wrong, &p),
        Err(PluginError::MarketplaceMismatch(_))
    ));
    assert_eq!(
        read(&p.settings_path)["extraKnownMarketplaces"]["rein-plugins"]["source"]["ref"],
        "main"
    );
}

#[test]
fn plugins_install_via_claude_with_fixed_argv_and_config_dir() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".claude/settings.json"), "{}");
    let p = default_profile(home.path());
    let shell = stub(0, true);
    set_plugins(&shell, &p, PluginAction::Install, &["rein-ux", "rein-seo"]).unwrap();
    let calls = shell.calls.borrow();
    assert_eq!(
        calls[0].0,
        [
            "plugin",
            "install",
            "rein-ux@rein-plugins",
            "--scope",
            "user"
        ]
    );
    assert_eq!(calls[1].0[2], "rein-seo@rein-plugins");
    assert_eq!(
        calls[0].1,
        [(
            "CLAUDE_CONFIG_DIR".to_owned(),
            p.config_dir.clone().into_os_string()
        )]
    );
    assert_eq!(
        read(&p.settings_path)["enabledPlugins"]["rein-ux@rein-plugins"],
        true
    );
}

#[test]
fn plugin_failures_are_reported_not_papered_over() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".claude/settings.json"), "{}");
    let p = default_profile(home.path());
    // claude exits 0 but writes nothing: the app does not write the key itself.
    assert!(matches!(
        set_plugins(&stub(0, false), &p, PluginAction::Install, &["rein-ux"]),
        Err(PluginError::NotVerified(_))
    ));
    assert_eq!(read(&p.settings_path), json!({}));
    assert!(matches!(
        set_plugins(&stub(1, true), &p, PluginAction::Install, &["rein-ux"]),
        Err(PluginError::Failed { .. })
    ));
    let missing = StubShell {
        missing: true,
        ..stub(0, true)
    };
    assert!(matches!(
        set_plugins(&missing, &p, PluginAction::Install, &["rein-ux"]),
        Err(PluginError::ClaudeMissing)
    ));
    let shell = stub(0, true);
    assert!(matches!(
        set_plugins(&shell, &p, PluginAction::Install, &["evil; rm -rf ~"]),
        Err(PluginError::UnknownPlugin(_))
    ));
    assert!(
        shell.calls.borrow().is_empty(),
        "no call for an unknown plugin"
    );
}

#[test]
fn claude_writing_anything_beyond_rein_plugins_entries_is_caught() {
    // E-4 security re-review P1: `claude` comes from PATH and is not trusted.
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".claude/settings.json"), "{}");
    let p = default_profile(home.path());
    let hostile = StubShell {
        inject_forbidden: true,
        ..stub(0, true)
    };
    match register_marketplace(&hostile, &p) {
        Err(PluginError::UnexpectedChange(keys)) => assert_eq!(keys, ["permissions"]),
        other => panic!("expected UnexpectedChange, got {other:?}"),
    }
    write(&p.settings_path, "{}"); // fresh file for the plugin-step case
    let hostile = StubShell {
        inject_forbidden: true,
        ..stub(0, true)
    };
    assert!(matches!(
        set_plugins(&hostile, &p, PluginAction::Install, &["rein-ux"]),
        Err(PluginError::UnexpectedChange(_))
    ));
}

#[test]
fn registering_a_profile_already_registered_at_session_start_is_idempotent() {
    // D-009: Claude Code registered the hand-written entry at its next start; the D-007
    // path then runs on a profile that already holds it.
    let home = tempfile::tempdir().unwrap();
    let existing =
        json!({"model": "opus", "extraKnownMarketplaces": rein_plugins::marketplace_entry()});
    write(
        &home.path().join(".claude/settings.json"),
        &existing.to_string(),
    );
    let p = default_profile(home.path());
    register_marketplace(&stub(0, true), &p).unwrap();
    assert_eq!(read(&p.settings_path), existing);
}
