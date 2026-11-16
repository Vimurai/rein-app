//! Doctor, the rein-app CLI actions, directory completion and the folder dialog.

use super::*;

fn doc_ok() -> Vec<u8> {
    br#"{"schema":1,"version":"0.5.0","status":"ok","checks":[]}"#.to_vec()
}

#[test]
fn doctor_runs_the_install_scope_inside_wsl_with_the_long_limit() {
    let s = ready().on(
        &["-d", "Ubuntu", "-e", "/usr/bin/zsh", "-lc", DOCTOR_SCRIPT],
        0,
        &doc_ok(),
    );
    let h = host(s);
    assert_eq!(h.doctor().status, "ok");
    assert!(
        h.shell
            .limits
            .lock()
            .unwrap()
            .contains(&crate::cli::DOCTOR_LIMIT)
    );
    assert_eq!(
        host(Stub::default()).doctor().status,
        "unknown",
        "WSL absent"
    );
    assert_eq!(
        h.distro_name().as_deref(),
        Some("Ubuntu"),
        "recorded with recent projects"
    );
    let absent = host(Stub::default()).doctor();
    assert_eq!(absent.setup.as_deref(), Some("wsl --install"));
    let empty =
        Stub::default()
            .on(&["--status"], 0, b"")
            .on(&["--list", "--quiet"], 0, &utf16("\r\n"));
    assert_eq!(
        host(empty).doctor().setup.as_deref(),
        Some("wsl --install -d Ubuntu")
    );
    assert!(
        host(Stub::default())
            .doctor()
            .reason
            .unwrap()
            .contains("wsl --install")
    );
}

#[test]
fn cli_actions_run_the_constant_script_in_the_login_shell() {
    let cli = Cli::default();
    let calls = Arc::clone(&cli.calls);
    let h = WslHost::new(ready(), Box::new(cli), EXE.into(), None);
    let mut lines = Vec::new();
    h.run_cli(&crate::cli::CliAction::UpdateCheck, &mut |l| lines.push(l))
        .unwrap();
    let names = vec!["work".to_string(), "default".to_string()];
    h.run_cli(
        &crate::cli::CliAction::ProfilesApply {
            dry_run: true,
            names,
        },
        &mut |_| {},
    )
    .unwrap();
    let calls = calls.lock().unwrap();
    let want = |script: &str| -> Vec<String> {
        ["-d", "Ubuntu", "-e", "/usr/bin/zsh", "-lc", script]
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    assert_eq!(calls[0], want("exec rein-app update --check"));
    assert_eq!(
        calls[1],
        want("exec rein-app profiles apply --dry-run --profile work --profile default")
    );
    assert_eq!(lines, ["a line"]);
}

#[test]
fn every_cli_action_has_its_exact_script_inside_wsl() {
    use crate::cli::CliAction;
    let cli = Cli::default();
    let calls = Arc::clone(&cli.calls);
    let h = WslHost::new(ready(), Box::new(cli), EXE.into(), None);
    let names = vec!["work".to_string()];
    for a in [
        CliAction::Update,
        CliAction::Rollback,
        CliAction::ProfilesList,
        CliAction::ProfilesApply {
            dry_run: false,
            names,
        },
    ] {
        h.run_cli(&a, &mut |_| {}).unwrap();
    }
    let scripts: Vec<String> = calls.lock().unwrap().iter().map(|c| c[5].clone()).collect();
    assert_eq!(
        scripts,
        [
            "exec rein-app update --yes",
            "exec rein-app rollback",
            "exec rein-app profiles list",
            "exec rein-app profiles apply --profile work",
        ]
    );
}

#[test]
fn exit_127_says_the_linux_package_is_missing_and_bad_names_never_reach_a_script() {
    let cli = Cli {
        code: Some(127),
        ..Cli::default()
    };
    let h = WslHost::new(ready(), Box::new(cli), EXE.into(), None);
    let e = h
        .run_cli(&crate::cli::CliAction::Update, &mut |_| {})
        .unwrap_err();
    assert_eq!(e.kind, "cli_missing");
    assert!(e.message.contains("inside WSL"));
    for bad in ["a b", "x;id", "$(id)", "a\"b", "", "-x", "--enable"] {
        let a = crate::cli::CliAction::ProfilesApply {
            dry_run: false,
            names: vec![bad.into()],
        };
        assert_eq!(cli_script(&a).unwrap_err().kind, "bad_profile", "{bad:?}");
    }
}

#[test]
fn dir_completion_lists_inside_wsl_with_the_guard_and_never_errors() {
    let list = b"proj/\nprod/\nprofile.txt\n.hidden/\n./\n../\n";
    let s = ready()
        .on(
            &[
                "-d",
                "Ubuntu",
                "--cd",
                "/home/me/",
                "-e",
                "sh",
                "-c",
                LIST_SCRIPT,
            ],
            0,
            list,
        )
        .on(
            &[
                "-d",
                "Ubuntu",
                "--cd",
                "/",
                "-e",
                "sh",
                "-c",
                LIST_ROOT_SCRIPT,
            ],
            0,
            b"home/\netc/\n",
        );
    let h = host(s);
    assert_eq!(
        h.complete_dir("/home/me/pro").unwrap(),
        ["/home/me/prod/", "/home/me/proj/"]
    );
    assert_eq!(
        h.complete_dir("/h").unwrap(),
        ["/home/"],
        "the root uses the unguarded listing"
    );
    assert!(
        h.complete_dir("/home/me/missing/x").unwrap().is_empty(),
        "a failed call: no lines"
    );
    assert!(h.complete_dir("relative").unwrap().is_empty());
    assert!(h.shell.limits.lock().unwrap().contains(&LIST_LIMIT));
    assert!(
        host(Stub::default())
            .complete_dir("/home/me/p")
            .unwrap()
            .is_empty(),
        "WSL absent"
    );
}

#[test]
fn the_dialog_starts_in_the_linux_home_and_maps_back() {
    let h = host(ready());
    let start = h.dialog_start().unwrap();
    assert_eq!(start, PathBuf::from("\\\\wsl.localhost\\Ubuntu\\home\\me"));
    assert_eq!(
        wsl_path::to_linux(&start.to_string_lossy(), "Ubuntu").unwrap(),
        "/home/me"
    );
    assert_eq!(
        host(Stub::default()).dialog_start(),
        None,
        "WSL absent: the system default"
    );
}
