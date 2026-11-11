//! `rein-app update [--yes]`, `rein-app rollback`, `rein-app doctor`
//! (blueprint §Components 7, §API, §Rollback Plan).

use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

use semver::Version;

use rein_release::{
    FetchError, InstallPaths, LatestRelease, RollbackError, RunUid, installed_version,
    latest_release, rollback_tree_with,
};

use crate::install_cmd::{self, Deps, EXIT_ERROR};
use crate::runner::{Process, ReinCommand, RunOutcome, run_rein};

pub const EXIT_CURRENT: u8 = 10;
pub const EXIT_OFFLINE: u8 = 11;
pub const EXIT_NO_PREVIOUS: u8 = 12;
/// `doctor`: `rein doctor` could not be started or ended without an exit code (D-016).
/// Never 1: for `rein doctor --json`, 1 means stdout is the document with status `fail`.
pub const EXIT_DOCTOR_NOT_RUN: u8 = 14;

/// Release notes printed before the confirmation, at most this many lines.
const MAX_NOTE_LINES: usize = 60;

/// How the operator answers "install?".
pub enum Confirm<'a> {
    /// `--yes`.
    Yes,
    /// Ask on this reader.
    Ask(&'a mut dyn BufRead),
}

/// Release notes are untrusted text: drop control characters (terminal escapes) except
/// newline and tab, and cap the length.
pub fn sanitise_notes(notes: &str) -> String {
    let clean: String = notes
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect();
    let mut lines: Vec<&str> = clean.lines().collect();
    let more = lines.len().saturating_sub(MAX_NOTE_LINES);
    lines.truncate(MAX_NOTE_LINES);
    let mut out = lines.join("\n");
    if more > 0 {
        out.push_str(&format!("\n… ({more} more lines in the release notes)"));
    }
    out
}

/// The newest release, or the exit code to return (11 when the server is unreachable).
fn resolve_latest(deps: &Deps<'_>, latest_api: &str) -> Result<LatestRelease, u8> {
    latest_release(deps.http, latest_api).map_err(|e| match e {
        FetchError::Network(_) | FetchError::Status { .. } => {
            eprintln!("rein-app: cannot reach the release server — {e}");
            EXIT_OFFLINE
        }
        e => {
            eprintln!("rein-app: {e}");
            EXIT_ERROR
        }
    })
}

/// `rein-app install [--version X.Y.Z]`: without a version, install the newest release
/// (same resolver as `update`; blueprint §API, D-012 ruling 4).
pub fn install_cli(home: &Path, version: Option<Version>, latest_api: &str, deps: &Deps<'_>) -> u8 {
    let version = match version {
        Some(v) => v,
        None => match resolve_latest(deps, latest_api) {
            Ok(l) => {
                println!("rein-app: newest rein release is {}", l.version);
                l.version
            }
            Err(code) => return code,
        },
    };
    install_cmd::install(home, &version, deps)
}

pub fn update(home: &Path, latest_api: &str, confirm: Confirm<'_>, deps: &Deps<'_>) -> u8 {
    let installed = installed_version(&InstallPaths::new(home).rein());
    let latest = match resolve_latest(deps, latest_api) {
        Ok(l) => l,
        Err(code) => return code,
    };
    if installed.as_ref().is_some_and(|v| *v >= latest.version) {
        println!(
            "rein-app: rein {} is installed; newest release is {} — already current",
            installed.unwrap_or_else(|| latest.version.clone()),
            latest.version
        );
        return EXIT_CURRENT;
    }
    let from = installed.map_or_else(|| "(not installed)".to_string(), |v| v.to_string());
    println!("rein-app: rein {from} → {}", latest.version);
    let notes = sanitise_notes(&latest.notes);
    if !notes.trim().is_empty() {
        println!("\n{notes}\n");
    }
    let go = match confirm {
        Confirm::Yes => true,
        Confirm::Ask(input) => {
            print!("Install rein {}? [y/N] ", latest.version);
            let _ = std::io::stdout().flush();
            let mut answer = String::new();
            input.read_line(&mut answer).is_ok()
                && matches!(answer.trim(), "y" | "Y" | "yes" | "Yes")
        }
    };
    if !go {
        println!("rein-app: not installed");
        return EXIT_ERROR;
    }
    install_cmd::install(home, &latest.version, deps)
}

/// `update` from the CLI: asks on stdin when it is a terminal, else requires `--yes`.
pub fn update_cli(home: &Path, yes: bool, latest_api: &str, deps: &Deps<'_>) -> u8 {
    if yes {
        return update(home, latest_api, Confirm::Yes, deps);
    }
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        eprintln!("rein-app: stdin is not a terminal — pass --yes to install without asking");
        return EXIT_ERROR;
    }
    let mut lock = stdin.lock();
    update(home, latest_api, Confirm::Ask(&mut lock), deps)
}

pub fn rollback(home: &Path, process: &dyn Process, run_uid: RunUid) -> u8 {
    let paths = InstallPaths::new(home);
    let (outcome, lock) = match rollback_tree_with(&paths, run_uid) {
        Ok(r) => r,
        Err(RollbackError::NoPrevious) => {
            eprintln!(
                "rein-app: no snapshot of an earlier version to roll back to ($HOME/.rein.prev-*)"
            );
            return EXIT_NO_PREVIOUS;
        }
        Err(e) => {
            install_cmd::report(format!("rein-app: {e}"));
            return EXIT_ERROR;
        }
    };
    println!(
        "rein-app: rolled back to rein {} (code only; your data in $HOME/.rein is unchanged)",
        outcome.restored
    );
    if let Some(snap) = &outcome.snapshot {
        println!(
            "rein-app: the version you rolled back from is kept at {} — `rein-app rollback` again returns to it",
            snap.display()
        );
    }
    // D-014: the rolled-back tree is re-installed (hooks, settings) and then checked, under
    // the lock, exactly like `install`.
    let code = crate::verdict::install_then_verdict(process, home);
    drop(lock);
    code
}

/// `rein-app doctor [--json]`: run `rein doctor` (with `--json`, its stdout untouched) and
/// pass its exit code through.
pub fn doctor(home: &Path, process: &dyn Process, json: bool) -> u8 {
    let cmd = if json {
        ReinCommand::DoctorJson
    } else {
        ReinCommand::Doctor
    };
    match run_rein(process, home, &[cmd]) {
        RunOutcome::Ok => 0,
        RunOutcome::Failed { code, .. } => match code.map(|c| (c, u8::try_from(c))) {
            Some((_, Ok(c))) => c,
            Some((c, Err(_))) => {
                eprintln!("rein-app: rein doctor exited with code {c} (outside 0..=255)");
                EXIT_DOCTOR_NOT_RUN
            }
            None => {
                eprintln!("rein-app: rein doctor ended without an exit code (killed by a signal?)");
                EXIT_DOCTOR_NOT_RUN
            }
        },
        RunOutcome::NotStarted { reason, .. } => {
            eprintln!(
                "rein-app: could not start rein doctor ({reason}) — run `rein-app install` first"
            );
            EXIT_DOCTOR_NOT_RUN
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_cmd::support::{Release, TestKey, VERSION};
    use crate::runner::Invocation;
    use rein_release::{Http, InstallLock, ReleaseSource};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs;
    use std::io;

    /// In-memory HTTP: URL → body; anything else is a network error.
    struct MemHttp(HashMap<String, Vec<u8>>);

    impl Http for MemHttp {
        fn download(&self, url: &str, _cap: u64, out: &mut dyn Write) -> Result<u64, FetchError> {
            let body = self
                .0
                .get(url)
                .ok_or_else(|| FetchError::Network(format!("no route {url}")))?;
            out.write_all(body)?;
            Ok(body.len() as u64)
        }
    }

    const API: &str = "mem://api/latest";
    const BASE: &str = "mem://rel";

    fn mem_release(key: &TestKey, tag: &str) -> MemHttp {
        let mut routes: HashMap<String, Vec<u8>> = Release::good(key)
            .routes()
            .into_iter()
            .map(|(path, route)| match route {
                crate::install_cmd::support::Route::Body(b) => (format!("{BASE}{path}"), b),
                crate::install_cmd::support::Route::Redirect(_) => unreachable!(),
            })
            .collect();
        routes.insert(
            API.into(),
            format!(r#"{{"tag_name":"{tag}","body":"- new\u001b[31m red\n"}}"#).into_bytes(),
        );
        MemHttp(routes)
    }

    #[derive(Default)]
    struct OkRein(RefCell<Vec<Vec<&'static str>>>);

    impl Process for OkRein {
        fn run(&self, inv: &Invocation) -> io::Result<Option<i32>> {
            self.0.borrow_mut().push(inv.args.clone());
            Ok(Some(0))
        }
        fn output(&self, inv: &Invocation) -> io::Result<(Option<i32>, Vec<u8>)> {
            self.0.borrow_mut().push(inv.args.clone());
            Ok((Some(0), crate::verdict::tests::doc("ok", &[]).into_bytes()))
        }
        fn capture(&self, program: &str, _args: &[&str]) -> Option<String> {
            Some(if program == "node" {
                "v24.0.0".into()
            } else {
                format!("{program} 1")
            })
        }
    }

    /// rein whose `install` and plain `doctor` exit with the given codes, and whose install
    /// document has top-level status `doc` (`"garbage"`: not a document) beside a failing
    /// host check; records each argv and whether the install lock was held while it ran.
    struct Scripted {
        install: i32,
        doctor: i32,
        doc: &'static str,
        ran: RefCell<Vec<(Vec<&'static str>, bool)>>,
    }

    impl Scripted {
        fn new(install: i32, doctor: i32) -> Self {
            Self::with_doc(install, doctor, "ok")
        }
        fn with_doc(install: i32, doctor: i32, doc: &'static str) -> Self {
            Self {
                install,
                doctor,
                doc,
                ran: RefCell::default(),
            }
        }
        fn argv(&self) -> Vec<Vec<&'static str>> {
            self.ran.borrow().iter().map(|(a, _)| a.clone()).collect()
        }
    }

    impl Process for Scripted {
        fn run(&self, inv: &Invocation) -> io::Result<Option<i32>> {
            let home = &inv.set.iter().find(|(k, _)| *k == "HOME").unwrap().1;
            let locked = Path::new(home).join(".rein.lock").exists();
            self.ran.borrow_mut().push((inv.args.clone(), locked));
            Ok(Some(if inv.args[0] == "install" {
                self.install
            } else {
                self.doctor
            }))
        }
        fn output(&self, inv: &Invocation) -> io::Result<(Option<i32>, Vec<u8>)> {
            let home = &inv.set.iter().find(|(k, _)| *k == "HOME").unwrap().1;
            let locked = Path::new(home).join(".rein.lock").exists();
            self.ran.borrow_mut().push((inv.args.clone(), locked));
            Ok(match self.doc {
                "garbage" => (Some(0), b"not a document".to_vec()),
                status => (
                    Some(i32::from(status == "fail")),
                    crate::verdict::tests::doc(status, &[("host", "fail"), ("install", "ok")])
                        .into_bytes(),
                ),
            })
        }
        fn capture(&self, program: &str, _args: &[&str]) -> Option<String> {
            Some(if program == "node" {
                "v24.0.0".into()
            } else {
                format!("{program} 1")
            })
        }
    }

    /// A home with one snapshot of 0.4.0 (bin/rein = "old").
    fn home_with_snapshot() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let snap = home.path().join(".rein.prev-0.4.0");
        fs::create_dir_all(snap.join("tree/bin")).unwrap();
        fs::write(snap.join("tree/bin/rein"), "old").unwrap();
        fs::write(snap.join("VERSION"), "0.4.0\n").unwrap();
        fs::write(
            snap.join("layout.json"),
            r#"{"mirrored":["bin"],"operator_owned":[]}"#,
        )
        .unwrap();
        home
    }

    #[test]
    fn rollback_runs_rein_install_then_doctor_under_the_lock() {
        let home = home_with_snapshot();
        let rein = Scripted::new(0, 0);
        assert_eq!(rollback(home.path(), &rein, InstallLock::run_uid), 0);
        assert_eq!(
            *rein.ran.borrow(),
            [
                (vec!["install"], true),
                (vec!["doctor", "--scope", "install", "--json"], true)
            ]
        );
        assert!(!home.path().join(".rein.lock").exists(), "released after");
    }

    /// D-029, on every command that syncs: a failed `rein install` is 13; then the verdict is
    /// the install document's top-level status alone (fail 3, warn and ok 0, no document 14).
    /// Every document here carries a failing `host` check: it never decides the verdict.
    #[test]
    fn the_install_verdict_is_the_documents_status_on_every_command() {
        use crate::install_cmd::{EXIT_DOCTOR, EXIT_REIN_INSTALL};
        let rows: [(i32, &str, u8); 6] = [
            (3, "ok", EXIT_REIN_INSTALL),
            (1, "ok", EXIT_REIN_INSTALL),
            (0, "fail", EXIT_DOCTOR),
            (0, "warn", 0),
            (0, "ok", 0),
            (0, "garbage", EXIT_DOCTOR_NOT_RUN),
        ];
        let key = TestKey::generate();
        for (install, doc, want) in rows {
            let home = home_with_snapshot();
            let rein = Scripted::with_doc(install, 0, doc);
            assert_eq!(
                rollback(home.path(), &rein, InstallLock::run_uid),
                want,
                "rollback {install}/{doc}"
            );
            let expected_runs = if install == 0 { 2 } else { 1 };
            assert_eq!(
                rein.argv().len(),
                expected_runs,
                "no retry; doctor only after a good install"
            );
            for is_update in [false, true] {
                let http = mem_release(&key, &format!("v{VERSION}"));
                let home = tempfile::tempdir().unwrap();
                let rein = Scripted::with_doc(install, 0, doc);
                assert_eq!(
                    run_with_process(home.path(), &http, &key, &rein, is_update),
                    want,
                    "{} {install}/{doc}",
                    if is_update { "update" } else { "install" }
                );
            }
        }
    }

    #[test]
    fn doctor_json_passes_the_flag_and_reins_exit_code_through() {
        let home = tempfile::tempdir().unwrap();
        let rein = Scripted::new(0, 2);
        assert_eq!(doctor(home.path(), &rein, true), 2);
        assert_eq!(rein.argv(), [vec!["doctor", "--json"]]);
        let rein = Scripted::new(0, 0);
        assert_eq!(doctor(home.path(), &rein, false), 0);
        assert_eq!(rein.argv(), [vec!["doctor"]]);
    }

    /// `rein doctor` that cannot be started (`None`) or ends with the given result.
    struct DoctorEnds(Option<Option<i32>>);

    impl Process for DoctorEnds {
        fn run(&self, _inv: &Invocation) -> io::Result<Option<i32>> {
            self.0
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no rein"))
        }
        fn output(&self, inv: &Invocation) -> io::Result<(Option<i32>, Vec<u8>)> {
            Ok((self.run(inv)?, Vec::new()))
        }
        fn capture(&self, _program: &str, _args: &[&str]) -> Option<String> {
            None
        }
    }

    /// Extends `doctor_json_passes_the_flag_and_reins_exit_code_through` (D-016 ruling 7):
    /// the app's own failures are 14, never rein's 1.
    #[test]
    fn doctor_exits_14_when_rein_does_not_run_and_passes_reins_codes_through() {
        let home = tempfile::tempdir().unwrap();
        for json in [false, true] {
            assert_eq!(doctor(home.path(), &DoctorEnds(None), json), 14);
            assert_eq!(doctor(home.path(), &DoctorEnds(Some(None)), json), 14);
            assert_eq!(doctor(home.path(), &DoctorEnds(Some(Some(300))), json), 14);
            for code in [0, 1, 2, 3, 13, 255] {
                let got = doctor(home.path(), &DoctorEnds(Some(Some(code))), json);
                assert_eq!(i32::from(got), code);
            }
        }
    }

    /// `install` (no version: newest tag) or `update --yes` with the given rein.
    fn run_with_process(
        home: &Path,
        http: &MemHttp,
        key: &TestKey,
        rein: &dyn Process,
        update_cmd: bool,
    ) -> u8 {
        let verifier = key.verifier();
        let source = ReleaseSource {
            base_url: BASE.into(),
        };
        let deps = Deps {
            http,
            source: &source,
            verifier: &verifier,
            process: rein,
            run_uid: InstallLock::run_uid,
        };
        if update_cmd {
            update(home, API, Confirm::Yes, &deps)
        } else {
            install_cli(home, None, API, &deps)
        }
    }

    fn run_update(home: &Path, http: &MemHttp, key: &TestKey, answer: Option<&str>) -> u8 {
        run_update_as(home, http, key, answer, InstallLock::run_uid)
    }

    fn run_update_as(
        home: &Path,
        http: &MemHttp,
        key: &TestKey,
        answer: Option<&str>,
        run_uid: RunUid,
    ) -> u8 {
        let rein = OkRein::default();
        let verifier = key.verifier();
        let source = ReleaseSource {
            base_url: BASE.into(),
        };
        let deps = Deps {
            http,
            source: &source,
            verifier: &verifier,
            process: &rein,
            run_uid,
        };
        match answer {
            None => update(home, API, Confirm::Yes, &deps),
            Some(a) => update(home, API, Confirm::Ask(&mut a.as_bytes()), &deps),
        }
    }

    #[test]
    fn installs_a_newer_release_on_yes_and_reports_current_after() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let http = mem_release(&key, &format!("v{VERSION}"));
        assert_eq!(run_update(home.path(), &http, &key, Some("y\n")), 0);
        assert_eq!(
            fs::read_to_string(home.path().join(".rein/VERSION")).unwrap(),
            format!("{VERSION}\n")
        );
        assert_eq!(run_update(home.path(), &http, &key, None), EXIT_CURRENT);
    }

    #[test]
    fn declining_or_an_offline_server_installs_nothing() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let http = mem_release(&key, &format!("v{VERSION}"));
        assert_eq!(run_update(home.path(), &http, &key, Some("\n")), EXIT_ERROR);
        assert_eq!(
            run_update(home.path(), &MemHttp(HashMap::new()), &key, None),
            EXIT_OFFLINE
        );
        assert!(!home.path().join(".rein").exists());
    }

    fn run_install(home: &Path, http: &MemHttp, key: &TestKey, version: Option<&str>) -> u8 {
        let rein = OkRein::default();
        let verifier = key.verifier();
        let source = ReleaseSource {
            base_url: BASE.into(),
        };
        let deps = Deps {
            http,
            source: &source,
            verifier: &verifier,
            process: &rein,
            run_uid: InstallLock::run_uid,
        };
        install_cli(
            home,
            version.map(|v| Version::parse(v).unwrap()),
            API,
            &deps,
        )
    }

    #[test]
    fn install_without_a_version_installs_the_newest_tag() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let http = mem_release(&key, &format!("v{VERSION}"));
        assert_eq!(run_install(home.path(), &http, &key, None), 0);
        assert_eq!(
            fs::read_to_string(home.path().join(".rein/VERSION")).unwrap(),
            format!("{VERSION}\n")
        );
    }

    #[test]
    fn install_with_a_version_skips_the_lookup_and_offline_exits_11() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let mut http = mem_release(&key, "v9.9.9");
        http.0.remove(API); // the API is never asked when --version is given
        assert_eq!(run_install(home.path(), &http, &key, Some(VERSION)), 0);
        let offline = tempfile::tempdir().unwrap();
        assert_eq!(
            run_install(offline.path(), &MemHttp(HashMap::new()), &key, None),
            EXIT_OFFLINE
        );
        assert!(!offline.path().join(".rein").exists());
    }

    /// A uid that is never the owner of the test's home (owner guard, D-013).
    #[cfg(unix)]
    const FOREIGN: RunUid = |l| l.run_uid().map(|u| u.wrapping_add(1));

    #[cfg(unix)]
    #[test]
    fn update_as_another_user_is_refused_before_anything_is_fetched() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let http = mem_release(&key, &format!("v{VERSION}"));
        assert_eq!(
            run_update_as(home.path(), &http, &key, None, FOREIGN),
            EXIT_ERROR
        );
        let said = install_cmd::reported();
        assert!(
            said.contains(&format!("{} belongs to uid", home.path().display())),
            "the refused path is named: {said}"
        );
        let left: Vec<_> = fs::read_dir(home.path()).unwrap().collect();
        assert!(left.is_empty(), "nothing written, no lock left: {left:?}");
    }

    #[cfg(unix)]
    #[test]
    fn rollback_as_another_user_is_refused_and_leaves_no_lock() {
        let home = tempfile::tempdir().unwrap();
        let snap = home.path().join(".rein.prev-0.4.0");
        fs::create_dir_all(snap.join("tree/bin")).unwrap();
        fs::write(snap.join("tree/bin/rein"), "old").unwrap();
        fs::write(snap.join("VERSION"), "0.4.0\n").unwrap();
        fs::write(
            snap.join("layout.json"),
            r#"{"mirrored":["bin"],"operator_owned":[]}"#,
        )
        .unwrap();
        let rein = OkRein::default();
        assert_eq!(rollback(home.path(), &rein, FOREIGN), EXIT_ERROR);
        let said = install_cmd::reported();
        assert!(
            said.contains(&format!("{} belongs to uid", home.path().display())),
            "the refused path is named: {said}"
        );
        assert!(!home.path().join(".rein").exists());
        assert!(!home.path().join(".rein.lock").exists());
        assert!(rein.0.borrow().is_empty(), "rein doctor not run");
    }

    #[test]
    fn notes_lose_terminal_escapes_and_are_capped() {
        assert_eq!(
            sanitise_notes("- a\u{1b}[31m red\r\n\tb"),
            "- a[31m red\n\tb"
        );
        let long = "x\n".repeat(MAX_NOTE_LINES + 5);
        assert!(sanitise_notes(&long).ends_with("(5 more lines in the release notes)"));
    }

    #[test]
    fn rollback_without_a_previous_tree_exits_12_and_doctor_passes_rein_through() {
        let home = tempfile::tempdir().unwrap();
        let rein = OkRein::default();
        assert_eq!(
            rollback(home.path(), &rein, InstallLock::run_uid),
            EXIT_NO_PREVIOUS
        );
        let snap = home.path().join(".rein.prev-0.4.0");
        fs::create_dir_all(snap.join("tree/bin")).unwrap();
        fs::write(snap.join("tree/bin/rein"), "old").unwrap();
        fs::write(snap.join("VERSION"), "0.4.0\n").unwrap();
        fs::write(
            snap.join("layout.json"),
            r#"{"mirrored":["bin"],"operator_owned":[]}"#,
        )
        .unwrap();
        assert_eq!(rollback(home.path(), &rein, InstallLock::run_uid), 0);
        assert_eq!(
            fs::read_to_string(home.path().join(".rein/bin/rein")).unwrap(),
            "old"
        );
        assert_eq!(
            *rein.0.borrow(),
            [
                vec!["install"],
                vec!["doctor", "--scope", "install", "--json"]
            ]
        );
        assert_eq!(doctor(home.path(), &rein, false), 0);
    }
}
