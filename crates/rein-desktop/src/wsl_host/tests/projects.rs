//! The probe, project validation, the pty child and the tab-scoped calls.

use super::*;

#[test]
fn states_absent_no_distro_ready() {
    assert_eq!(
        host(Stub::default()).probe_distro().unwrap_err().kind,
        "wsl_absent"
    );
    let empty =
        Stub::default()
            .on(&["--status"], 0, b"")
            .on(&["--list", "--quiet"], 0, &utf16("\r\n"));
    assert_eq!(
        host(empty).probe_distro().unwrap_err().kind,
        "wsl_no_distro"
    );
    let h = host(ready());
    let d = h.probe_distro().unwrap();
    assert_eq!(
        d,
        Distro {
            name: "Ubuntu".into(),
            home: "/home/me".into(),
            shell: "/usr/bin/zsh".into()
        }
    );
    h.probe_distro().unwrap();
    assert_eq!(h.shell.calls().len(), 3, "probed once, then cached");
}

#[test]
fn the_probe_refuses_bad_name_home_or_shell_and_defaults_the_shell() {
    for out in [
        &b"Ubu ntu\n/home/me\n/bin/bash\n"[..],
        b"Ubuntu\nrelative\n/bin/bash\n",
        b"Ubuntu\n/home/m e\n/bin/bash\n",
        b"Ubuntu\n/home/me\n/bin/ba\"sh\n",
        b"Ubuntu\n/home/\xc3\xa9\n/bin/bash\n",
        b"",
    ] {
        let s = Stub::default()
            .on(&["--status"], 0, b"")
            .on(&["--list", "--quiet"], 0, b"Ubuntu\n")
            .on(&["-e", "sh", "-c", PROBE_SCRIPT], 0, out);
        assert_eq!(
            host(s).probe_distro().unwrap_err().kind,
            "wsl_failed",
            "{:?}",
            String::from_utf8_lossy(out)
        );
    }
    let no_shell = Stub::default()
        .on(&["--status"], 0, b"")
        .on(&["--list", "--quiet"], 0, b"Ubuntu\n")
        .on(
            &["-e", "sh", "-c", PROBE_SCRIPT],
            0,
            b"Ubuntu\n/home/me\n\n",
        );
    assert_eq!(host(no_shell).probe_distro().unwrap().shell, "/bin/sh");
}

#[test]
fn a_configured_distro_must_be_installed_and_is_passed_with_d() {
    let s = Stub::default()
        .on(&["--status"], 0, b"")
        .on(&["--list", "--quiet"], 0, &utf16("Ubuntu\r\nDebian\r\n"))
        .on(
            &["-d", "Debian", "-e", "sh", "-c", PROBE_SCRIPT],
            0,
            b"Debian\n/home/me\n/bin/bash\n",
        );
    let h = WslHost::new(
        s,
        Box::new(Cli::default()),
        EXE.into(),
        Some("debian".into()),
    );
    assert_eq!(h.probe_distro().unwrap().name, "Debian");
    let missing = WslHost::new(
        ready(),
        Box::new(Cli::default()),
        EXE.into(),
        Some("Arch".into()),
    );
    assert_eq!(
        missing.probe_distro().unwrap_err().kind,
        "wsl_distro_missing"
    );
    let bad = WslHost::new(
        ready(),
        Box::new(Cli::default()),
        EXE.into(),
        Some("a b".into()),
    );
    assert_eq!(bad.probe_distro().unwrap_err().kind, "wsl_distro_missing");
}

#[test]
fn validation_exits_map_to_the_four_errors() {
    let s = ready()
        .on(&validate("/home/me/p"), 0, b"/home/me/p\n")
        .on(&validate("/home/me/noai"), 3, b"")
        .on(&validate("/home/me/norein"), 4, b"")
        .on(&validate("/home/me/file"), 2, b"")
        .on(&validate("/home/me/link"), 0, b"/home/me/a\"b\n")
        .on(&validate("/home/me/gone"), 5, b"");
    let h = host(s);
    assert_eq!(
        h.project("\\\\wsl.localhost\\Ubuntu\\home\\me\\p").unwrap(),
        PathBuf::from("/home/me/p")
    );
    let kind = |p: &str| h.project(p).unwrap_err().kind;
    assert_eq!(kind("/home/me/noai"), "no_ai");
    assert_eq!(kind("/home/me/norein"), "rein_missing");
    assert_eq!(kind("/home/me/file"), "not_a_directory");
    assert_eq!(
        kind("/home/me/link"),
        "unsafe_path",
        "the canonical path is re-checked"
    );
    assert_eq!(kind("/home/me/absent"), "not_a_directory");
    assert_eq!(
        kind("/home/me/gone"),
        "not_a_directory",
        "--cd fell back to /"
    );
    let calls = h.shell.calls().len();
    assert_eq!(kind("/"), "not_a_directory");
    assert_eq!(kind("\\\\wsl.localhost\\Ubuntu"), "not_a_directory");
    assert_eq!(
        h.shell.calls().len(),
        calls,
        "`/` is refused before any call"
    );
    assert_eq!(kind("C:\\Users\\me"), "not_wsl_path");
    assert_eq!(kind("\\\\wsl$\\Debian\\home"), "other_distro");
    assert_eq!(kind("/home/me/a\\b"), "unsafe_path");
}

#[test]
fn the_pty_child_is_wsl_exe_with_the_constant_script_and_no_wslenv() {
    let h = host(ready());
    let spec = h
        .rein_start(Path::new("/home/me/p"), crate::tmux_server::Server::User)
        .unwrap();
    assert_eq!(spec.program, PathBuf::from(EXE));
    let argv: Vec<String> = spec
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        argv,
        [
            "-d",
            "Ubuntu",
            "--cd",
            "/home/me/p",
            "-e",
            "/usr/bin/zsh",
            "-lic",
            "tty; exec ~/.rein/bin/rein start"
        ]
    );
    assert!(spec.tty_line, "the tty line is taken out of the output");
    assert_eq!(spec.cwd, None);
    let cmd = spec.command();
    assert_eq!(cmd.get_env("WSLENV"), None);
    assert!(spec.env_set.is_empty(), "TERM comes from WSL, not from us");
}

/// E-63 (V9 components 1–2): a directory without `.ai/` is told apart in the same
/// validation call (no second process) — exit 3 now carries the canonical path; set-up is
/// the tab's own start path with the set-up constant; `rein init [--check]` likewise.
#[test]
fn a_folder_without_ai_is_known_in_the_one_validation_and_set_up_on_the_start_path() {
    use crate::project_setup::{ReinRun, setup_script};
    use crate::tmux_server::Server;
    let h = host(
        ready()
            .on(&validate("/home/me/new"), 3, b"/home/me/new\n")
            .on(&validate("/home/me/p"), 0, b"/home/me/p\n"),
    );
    h.probe_distro().unwrap(); // once per window, not per open
    let before = h.shell.calls().len();
    assert_eq!(
        h.project_dir("/home/me/new").unwrap(),
        (PathBuf::from("/home/me/new"), crate::host::Found::Plain)
    );
    assert_eq!(
        h.project_dir("/home/me/p").unwrap(),
        (PathBuf::from("/home/me/p"), crate::host::Found::Project)
    );
    assert_eq!(h.shell.calls().len(), before + 2, "one call per open");
    assert!(VALIDATE_SCRIPT.find("pwd -P").unwrap() < VALIDATE_SCRIPT.find(".ai").unwrap());
    let argv = |a: &[OsString]| -> Vec<String> {
        a.iter().map(|x| x.to_string_lossy().into_owned()).collect()
    };
    for server in [Server::App, Server::User] {
        let start = argv(
            &h.rein_start(Path::new("/home/me/new"), server)
                .unwrap()
                .args,
        );
        let setup = h.rein_setup(Path::new("/home/me/new"), server).unwrap();
        let got = argv(&setup.args);
        assert_eq!(got[..7], start[..7], "the start's own path");
        assert_eq!(got[7], setup_script(server, true));
        assert!(setup.tty_line);
    }
    for which in [ReinRun::InitCheck, ReinRun::Init] {
        let cmd = h.rein_command(Path::new("/home/me/p"), which).unwrap();
        assert_eq!(cmd.get_program(), EXE);
        let got: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            got,
            [
                "-d",
                "Ubuntu",
                "--cd",
                "/home/me/p",
                "-e",
                "/usr/bin/zsh",
                "-lic",
                which.wsl_script()
            ]
        );
        assert!(cmd.get_envs().any(|(k, v)| k == "WSLENV" && v.is_none()));
    }
    assert_eq!(
        h.rein_command(Path::new("/home/me/a\"b"), ReinRun::Init)
            .unwrap_err()
            .kind,
        "unsafe_path"
    );
}

/// V12 (E-69): the live lookup rides in the setup process — the tag is its one word after
/// `--`, the answer its last line of output — and the attach child is the tab's own start
/// path with the attach script and the tag as its positional word. A word that is not a tag
/// reaches no process.
#[test]
fn lookup_and_attach_are_constant_wsl_calls_with_the_tag_as_one_word() {
    let tag = "a".repeat(64);
    let other = "b".repeat(64);
    let h = host(
        ready()
            .on(
                &["-d", "Ubuntu", "-e", "sh", "-s", "--", &tag],
                0,
                format!("motd\nsessions\ns|$3|{tag}|/home/me/p\ns|$4||/home/me/q\nend\n")
                    .as_bytes(),
            )
            .on(
                &["-d", "Ubuntu", "-e", "sh", "-s", "--", &other],
                0,
                b"sessions\nend\n",
            ),
    );
    use crate::tmux_server::SessionRow;
    assert_eq!(
        h.tmux_setup(&tag),
        Ok(vec![
            SessionRow {
                id: "$3".into(),
                tag: tag.clone(),
                path: "/home/me/p".into()
            },
            SessionRow {
                id: "$4".into(),
                tag: String::new(),
                path: "/home/me/q".into()
            },
        ])
    );
    assert_eq!(h.tmux_setup(&other), Ok(vec![]));
    let before = h.shell.calls().len();
    for bad in [
        "sweep",
        "",
        &"A".repeat(64),
        &format!("{};x", "a".repeat(62)),
    ] {
        assert!(h.tmux_setup(bad).is_err(), "{bad:?}");
        assert!(h.rein_attach(Path::new("/home/me/p"), bad, None).is_err());
        assert!(
            h.rein_attach(Path::new("/home/me/p"), &tag, Some(bad))
                .is_err(),
            "an id that is not $N"
        );
    }
    assert_eq!(
        h.shell.calls().len(),
        before,
        "no process for a refused word"
    );

    // An adoption (V12 9): the session id is the second word after the tag.
    let adopt = h
        .rein_attach(Path::new("/home/me/p"), &tag, Some("$4"))
        .unwrap();
    let words: Vec<String> = adopt
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        words[8..],
        ["sh".to_string(), tag.clone(), "$4".to_string()]
    );
    let spec = h.rein_attach(Path::new("/home/me/p"), &tag, None).unwrap();
    let argv: Vec<String> = spec
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let start: Vec<String> = h
        .rein_start(Path::new("/home/me/p"), crate::tmux_server::Server::App)
        .unwrap()
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(argv[..7], start[..7], "the start's own path");
    assert_eq!(argv[7], crate::tmux_server::attach_script_wsl());
    assert_eq!(argv[8..], ["sh".to_string(), tag]);
    assert!(spec.tty_line);
    assert_eq!(spec.command().get_env("WSLENV"), None);
}

/// E-64 (security review C3, C8): `rein team add` reaches WSL as the constant script and
/// then `rein-app`, the e-mail and the range — each one argv word, none inside a script.
#[test]
fn team_add_values_are_their_own_words_after_the_constant() {
    let h = host(ready());
    let v = crate::team::add_values("a.b+c@x-y.io", crate::team::Prefix::P, 425, 524).unwrap();
    let cmd = h.rein_team_add(Path::new("/home/me/p"), &v).unwrap();
    assert_eq!(cmd.get_program(), EXE);
    let got: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        got,
        [
            "-d",
            "Ubuntu",
            "--cd",
            "/home/me/p",
            "-e",
            "/usr/bin/zsh",
            "-lic",
            crate::team::ADD_SCRIPT,
            "rein-app",
            "a.b+c@x-y.io",
            "P:425-524"
        ]
    );
    assert!(cmd.get_envs().any(|(k, v)| k == "WSLENV" && v.is_none()));
    for which in [
        crate::project_setup::ReinRun::TeamShow,
        crate::project_setup::ReinRun::Trunk,
        crate::project_setup::ReinRun::MigrateTeam,
        crate::project_setup::ReinRun::MigrateLegacy,
    ] {
        let c = h.rein_command(Path::new("/home/me/p"), which).unwrap();
        let last = c.get_args().last().unwrap().to_string_lossy().into_owned();
        assert_eq!(last, which.wsl_script());
        assert!(!last.contains(['"', '\'']));
    }
}

/// E-63 rework 3: inside a project (exit 6) is told in the same one call, with the parent.
#[test]
fn a_folder_inside_a_project_is_told_with_its_parent() {
    let h = host(ready().on(
        &validate("/home/me/p/src"),
        6,
        b"/home/me/p/src\n/home/me/p\n",
    ));
    assert_eq!(
        h.project_dir("/home/me/p/src").unwrap(),
        (
            PathBuf::from("/home/me/p/src"),
            crate::host::Found::Inside(PathBuf::from("/home/me/p"))
        )
    );
    assert!(!VALIDATE_SCRIPT.contains(['"', '\'']), "argv rule");
}

/// E-62 (V9 component 4): the status script goes on stdin with only the validated words in
/// argv; the kill is the tab's own start path made into `--kill --yes`, quote-free, on
/// each server; its failures are the app's fixed sentences.
#[test]
fn agents_status_and_kill_take_the_tabs_own_path() {
    use crate::tmux_server::Server;
    let words = ["/dev/pts/3".to_string(), "app".to_string()];
    let status = ["-d", "Ubuntu", "-e", "sh", "-ls", "--", "/dev/pts/3", "app"];
    let kill = |script: &str| {
        [
            "-d",
            "Ubuntu",
            "--cd",
            "/home/me/p",
            "-e",
            "/usr/bin/zsh",
            "-lic",
            script,
        ]
        .map(String::from)
        .to_vec()
    };
    let app = crate::tmux_server::kill_script();
    let h = host(
        ready()
            .on(&status, 0, b"engineer|claude|0\n")
            .on(
                &kill(&app).iter().map(String::as_str).collect::<Vec<_>>(),
                0,
                b"",
            )
            .on(
                &kill(KILL_SCRIPT)
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                2,
                b"refusing",
            ),
    );
    assert_eq!(h.tmux_agents(&words).unwrap(), b"engineer|claude|0\n");
    assert_eq!(h.rein_kill(Path::new("/home/me/p"), Server::App), Ok(()));
    assert_eq!(
        h.rein_kill(Path::new("/home/me/p"), Server::User),
        Err(crate::agents::KILL_FAILED)
    );
    let calls = h.shell.calls();
    assert!(calls.contains(&kill(&app)) && calls.contains(&kill(KILL_SCRIPT)));
    assert!(!app.contains(['\'', '"']) && app.ends_with("rein start --kill --yes"));
    // A path the start would refuse is refused before any process.
    let n = h.shell.calls().len();
    assert_eq!(
        h.rein_kill(Path::new("/home/me/a\"b"), Server::User),
        Err(crate::agents::KILL_NOT_STARTED)
    );
    assert_eq!(h.shell.calls().len(), n);
    // WSL missing: fixed sentences, nothing else.
    let none = host(Stub::default());
    assert_eq!(none.tmux_agents(&words), Err(crate::agents::NOT_ASKED));
    assert_eq!(
        none.rein_kill(Path::new("/home/me/p"), Server::App),
        Err(crate::agents::KILL_NOT_STARTED)
    );
}

/// E-57 (V8 3b): on the app's server the argv start script is the quote-free constant with
/// the tty line, and nothing else changes in the child.
#[test]
fn the_app_server_start_is_the_quote_free_constant() {
    let h = host(ready());
    let spec = h
        .rein_start(Path::new("/home/me/p"), crate::tmux_server::Server::App)
        .unwrap();
    let argv: Vec<String> = spec
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        &argv[..7],
        [
            "-d",
            "Ubuntu",
            "--cd",
            "/home/me/p",
            "-e",
            "/usr/bin/zsh",
            "-lic"
        ]
    );
    assert_eq!(argv[7], crate::tmux_server::start_script_wsl());
    assert!(
        !argv[7].contains(['\'', '"']),
        "argv rule: no quote character"
    );
    assert!(spec.tty_line);
    assert_eq!(spec.command().get_env("WSLENV"), None);
}

#[test]
fn the_overlay_runs_sh_ls_inside_wsl_with_only_the_validated_words() {
    let words: Vec<String> = [
        "/dev/pts/3",
        "hidden",
        "#1e1e2e",
        "#cdd6f4",
        "#a6adc8",
        "#6c7086",
        "#cba6f7",
    ]
    .map(String::from)
    .to_vec();
    let mut want = vec!["-d", "Ubuntu", "-e", "sh", "-ls", "--"];
    want.extend(words.iter().map(String::as_str));
    let h = host(ready().on(&want, 0, b""));
    assert_eq!(h.tmux_overlay(&words, &|| true), Ok(()));
    assert_eq!(h.shell.calls().last().unwrap(), &want);
    assert_eq!(
        h.shell.limits.lock().unwrap().last(),
        Some(&tmux_overlay::LIMIT)
    );
    let h = host(ready().on(&want, 4, b""));
    assert_eq!(
        h.tmux_overlay(&words, &|| true),
        Err("tmux is older than 3.2".into())
    );
}

/// E-60 (owed by E-59): the panel read refuses a project path with a `.` or `..` segment, or
/// one ending in a dot or a space (Windows strips those, so `proj.` would read `proj`), before
/// it asks WSL anything.
#[test]
fn the_panel_read_refuses_odd_segments_before_any_call() {
    use crate::project_file::{ProjectFile, Unavailable};
    let h = host(ready());
    for p in [
        "/home/me/p.",
        "/home/me/p ",
        "/home/me/./p",
        "/home/me/../p",
        "relative/p",
    ] {
        assert_eq!(
            h.read_project_file(Path::new(p), ProjectFile::StateJson, true)
                .unwrap_err(),
            Unavailable::Unreadable,
            "{p:?}"
        );
    }
    assert!(
        h.shell.calls().is_empty(),
        "no wsl.exe call for a refused path"
    );
}
