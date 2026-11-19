//! `config.rs`'s tests, beside it so the module stays under the 1000-line limit (E-78).

use super::*;

/// The defaults file with every `# key = value` / `# [section]` line uncommented.
fn uncommented() -> String {
    DEFAULTS_FILE
        .lines()
        .map(|l| match l.strip_prefix("# ") {
            // `#   …` (indented) lines are examples, not defaults.
            Some(rest)
                if !rest.starts_with(' ') && rest.starts_with('[')
                    || !rest.starts_with(' ')
                        && rest.split('=').next().is_some_and(|k| {
                            let k = k.trim();
                            !k.is_empty()
                                && k.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                                && rest.contains('=')
                        }) =>
            {
                rest.split("   #")
                    .next()
                    .unwrap_or(rest)
                    .trim_end()
                    .to_string()
            }
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_defaults_file_is_the_built_in_defaults() {
    assert_eq!(parse(FILE_NAME, DEFAULTS_FILE).unwrap(), Config::default());
    let live = uncommented();
    assert!(
        live.contains("size = 14") && live.contains("[keys]"),
        "{live}"
    );
    assert_eq!(
        parse(FILE_NAME, &live).unwrap(),
        Config::default(),
        "{live}"
    );
}

#[test]
fn each_refusal_names_file_line_and_reason() {
    let cases = [
        ("[font]\nbogus = 1\n", 2, "unknown field"),
        (
            "[theme]\nname = \"nord\"\n[theme.custom]\naccent = \"red\"\n",
            4,
            "not #rrggbb",
        ),
        (
            "[theme.custom]\ncolor16 = \"#000000\"\n",
            2,
            "unknown colour",
        ),
        (
            "[keys.normal]\n\"g x\" = \"shell.exec\"\n",
            2,
            "is not an action",
        ),
        (
            "[keys.normal]\n\"g o\" = \"project.open /etc\"\n",
            2,
            "does not take that argument",
        ),
        ("[window]\nopacity = 0.2\n", 2, "outside 0.6–1.0"),
        (
            "[font]\nfamily = \"x; } body { background: url(http://e) }\"\n",
            2,
            "font family",
        ),
        ("[theme]\nname = \"solarized-light\"\n", 2, "unknown theme"),
        ("[wsl]\ndistro = \"a b\"\n", 2, "not a WSL distro name"),
        ("[tmux]\nstatus = \"off\"\n", 2, "hidden, app or keep"),
        ("[tmux]\nserver = \"mine\"\n", 2, "app or user"),
        ("[tmux]\noverlay = \"yes\"\n", 2, "expected a boolean"),
        ("[panels]\nrail = \"left\"\n", 2, "shown or hidden"),
        ("[tabs]\nstatus = \"left\"\n", 2, "shown or hidden"),
        ("[updates]\ninterval_min = 5\n", 2, "outside 10–1440"),
        ("[notifications]\nstuck_after_min = 2\n", 2, "outside 3–120"),
        (
            "[notifications]\nstuck_after_min = 121\n",
            2,
            "outside 3–120",
        ),
        (
            "[notifications]\nevents = [\"stopped\", \"waiting\"]\n",
            2,
            "\"waiting\" is not an event",
        ),
        (
            "[notifications]\nevents = [\"ended\", \"ended\"]\n",
            2,
            "named twice",
        ),
        ("[notifications]\nsystem = 1\n", 2, "expected a boolean"),
        (
            "[panels]\nenabled = [\"tasks\", \"shell\"]\n",
            2,
            "\"shell\" is not a panel",
        ),
        (
            "[panels]\nenabled = [\"tasks\", \"tasks\"]\n",
            2,
            "listed twice",
        ),
        (
            "[keys.normal]\n\"g p\" = \"panel.toggle shell\"\n",
            2,
            "does not take that argument",
        ),
        ("[keys]\nescape = []\n", 2, "escape is"),
        ("this is not toml\n", 1, "expected"),
    ];
    for (text, line, reason) in cases {
        let e = parse(FILE_NAME, text).unwrap_err();
        assert_eq!(e.line, Some(line), "{text:?} → {e}");
        assert!(e.reason.contains(reason), "{text:?} → {e}");
        assert!(
            e.to_string().starts_with(&format!("config.toml:{line}: ")),
            "{e}"
        );
    }
    let big = format!("# {}\n", "x".repeat(MAX_BYTES as usize));
    assert!(
        parse(FILE_NAME, &big)
            .unwrap_err()
            .reason
            .contains("256 KiB")
    );
}

#[test]
fn a_valid_file_parses() {
    let text = "[font]\nfamily = \"Iosevka Term, monospace\"\nsize = 16\n[theme]\nname = \"nord\"\n[theme.custom]\nbg = \"#101010\"\ncolor4 = \"#5e81ac\"\n[window]\ndecorations = false\nopacity = 0.9\nstart = \"last\"\n[keys]\nleader = \",\"\n[keys.normal]\n\"<leader> 1\" = \"tab.goto 1\"\n\"g s\" = \"input.style simple\"\n\"g p\" = \"panel.toggle tasks\"\n[panels]\nrail = \"hidden\"\nenabled = [\"tasks\"]\n[tabs]\nstatus = \"hidden\"\n[wsl]\ndistro = \"Ubuntu-24.04\"\n";
    let c = parse(FILE_NAME, text).unwrap();
    assert_eq!(c.font.size, 16);
    assert_eq!(
        c.theme.custom.get("color4").map(String::as_str),
        Some("#5e81ac")
    );
    assert!(!c.window.decorations);
    assert_eq!(c.keys.leader, ",");
    assert_eq!(c.wsl.distro.as_deref(), Some("Ubuntu-24.04"));
    assert_eq!(c.panels.rail, "hidden");
    assert_eq!(c.panels.enabled, ["tasks"]);
    assert_eq!(c.tabs.status, "hidden");
    assert_eq!(
        settings_keys_set(text),
        [
            "theme.name",
            "font.size",
            "window.decorations",
            "panels.rail",
            "panels.enabled",
            "tabs.status"
        ]
    );
    // E-60: the default offers every panel with the rail shown.
    assert_eq!(Config::default().panels.enabled, PANEL_IDS);
    assert_eq!(Config::default().panels.rail, "shown");
    assert_eq!(
        Config::default().tabs.status,
        "shown",
        "E-77: the marks are shown"
    );
}

#[test]
fn first_run_creates_the_file_and_a_save_is_noticed_and_a_refusal_keeps_the_previous() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("rein-app");
    let s = ConfigService::open(Some(dir.clone()));
    let file = dir.join(FILE_NAME);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), DEFAULTS_FILE);
    assert_eq!(s.view().config, Config::default());
    assert!(s.reload_if_changed().is_none(), "unchanged");

    std::fs::write(&file, "[theme]\nname = \"nord\"\n").unwrap();
    let v = s.reload_if_changed().expect("a save is noticed");
    assert_eq!(v.config.theme.name, "nord");
    assert_eq!(v.error, None);

    std::fs::write(&file, "[theme]\nname = \"nord\"\nbogus = 1\n").unwrap();
    let v = s.reload_if_changed().expect("a save is noticed");
    assert_eq!(
        v.config.theme.name, "nord",
        "the previous good config stays"
    );
    assert!(v.error.unwrap().starts_with("config.toml:3:"));

    // Never rewritten: a second open leaves the operator's file alone.
    let _ = ConfigService::open(Some(dir));
    assert!(std::fs::read_to_string(&file).unwrap().contains("bogus"));
}

#[test]
fn the_theme_list_is_the_web_sides() {
    let ts = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/theme.ts"),
    )
    .unwrap();
    let start = ts.find("export const THEME_NAMES").unwrap();
    let list = &ts[start..start + ts[start..].find("];").unwrap()];
    let names: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
    assert_eq!(names, THEMES);
}

#[test]
fn a_config_dir_that_cannot_be_created_is_reported() {
    let t = tempfile::tempdir().unwrap();
    let blocker = t.path().join("file");
    std::fs::write(&blocker, "").unwrap();
    let s = ConfigService::open(Some(blocker.join("rein-app")));
    assert_eq!(s.view().config, Config::default());
    assert!(
        s.view()
            .error
            .unwrap()
            .starts_with("cannot create config.toml")
    );
}

#[test]
fn the_settings_keys_a_file_sets_are_known() {
    assert!(
        settings_keys_set(DEFAULTS_FILE).is_empty(),
        "all commented out"
    );
    assert_eq!(
        settings_keys_set(
            "[theme]\nname = \"nord\"\n[font]\nfamily = \"x\"\n[window]\ndecorations = false\n"
        ),
        ["theme.name", "window.decorations"]
    );
    // E-57 (V8): tmux.server is one of the keys Settings can also set.
    assert_eq!(
        settings_keys_set("[tmux]\nserver = \"user\"\nstatus = \"app\"\n"),
        ["tmux.status", "tmux.server"]
    );
}

#[test]
fn key_names() {
    for ok in [
        "a", ":", "C-\\", "C-n", "C-S-p", "M-1", "Space", "F12", "S-Tab", "<",
    ] {
        assert!(key_ok(ok), "{ok}");
    }
    for bad in ["", "ab", "C-", "F13", "Ctrl+n", "é"] {
        assert!(!key_ok(bad), "{bad}");
    }
}
