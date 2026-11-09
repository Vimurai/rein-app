//! `rein-app update [--yes]`, `rein-app rollback`, `rein-app doctor`
//! (blueprint §Components 7, §API, §Rollback Plan).

use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

use semver::Version;

use rein_release::{
    FetchError, InstallPaths, LatestRelease, RollbackError, RunUid, installed_version,
    latest_release, rollback_tree_with,
};

use crate::install_cmd::{self, Deps, EXIT_DOCTOR, EXIT_ERROR};
use crate::runner::{Process, ReinCommand, RunOutcome, run_rein};

pub const EXIT_CURRENT: u8 = 10;
pub const EXIT_OFFLINE: u8 = 11;
pub const EXIT_NO_PREVIOUS: u8 = 12;

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
    let result = run_rein(process, home, &[ReinCommand::Doctor]);
    drop(lock);
    match result {
        RunOutcome::Ok => 0,
        _ => {
            eprintln!("rein-app: rein doctor reported problems after the rollback");
            EXIT_DOCTOR
        }
    }
}

/// `rein-app doctor`: run `rein doctor` and pass its exit code through.
pub fn doctor(home: &Path, process: &dyn Process) -> u8 {
    match run_rein(process, home, &[ReinCommand::Doctor]) {
        RunOutcome::Ok => 0,
        RunOutcome::Failed { code, .. } => code.and_then(|c| u8::try_from(c).ok()).unwrap_or(1),
        RunOutcome::NotStarted { reason, .. } => {
            eprintln!(
                "rein-app: could not start rein doctor ({reason}) — run `rein-app install` first"
            );
            EXIT_ERROR
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
        fn capture(&self, program: &str, _args: &[&str]) -> Option<String> {
            Some(if program == "node" {
                "v24.0.0".into()
            } else {
                format!("{program} 1")
            })
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
        assert_eq!(*rein.0.borrow(), [vec!["doctor"]]);
        assert_eq!(doctor(home.path(), &rein), 0);
    }
}
