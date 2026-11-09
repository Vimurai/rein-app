//! `rein-app update [--yes]`, `rein-app rollback`, `rein-app doctor`
//! (blueprint §Components 7, §API, §Rollback Plan).

use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

use rein_release::{
    FetchError, InstallPaths, RollbackError, installed_version, latest_release, rollback_tree,
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

pub fn update(home: &Path, latest_api: &str, confirm: Confirm<'_>, deps: &Deps<'_>) -> u8 {
    let installed = installed_version(&InstallPaths::new(home).rein());
    let latest = match latest_release(deps.http, latest_api) {
        Ok(l) => l,
        Err(e @ (FetchError::Network(_) | FetchError::Status { .. })) => {
            eprintln!("rein-app: cannot reach the release server — {e}");
            return EXIT_OFFLINE;
        }
        Err(e) => {
            eprintln!("rein-app: {e}");
            return EXIT_ERROR;
        }
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

pub fn rollback(home: &Path, process: &dyn Process) -> u8 {
    let paths = InstallPaths::new(home);
    let (outcome, lock) = match rollback_tree(&paths) {
        Ok(r) => r,
        Err(RollbackError::NoPrevious) => {
            eprintln!("rein-app: no previous tree to roll back to");
            return EXIT_NO_PREVIOUS;
        }
        Err(e) => {
            eprintln!("rein-app: {e}");
            return EXIT_ERROR;
        }
    };
    let restored = outcome
        .restored
        .map_or_else(|| "(unknown version)".to_string(), |v| v.to_string());
    println!("rein-app: rolled back to rein {restored}");
    if let Some(out) = &outcome.swapped_out {
        println!(
            "rein-app: the tree you rolled back from is kept at {} — operator files written since \
             the restored tree was replaced (accounts, approvals, secrets) are there, not in $HOME/.rein",
            out.display()
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
    use rein_release::{Http, ReleaseSource};
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
        assert_eq!(rollback(home.path(), &rein), EXIT_NO_PREVIOUS);
        fs::create_dir_all(home.path().join(".rein.prev-0.4.0")).unwrap();
        fs::write(home.path().join(".rein.prev-0.4.0/VERSION"), "0.4.0\n").unwrap();
        assert_eq!(rollback(home.path(), &rein), 0);
        assert_eq!(*rein.0.borrow(), [vec!["doctor"]]);
        assert_eq!(doctor(home.path(), &rein), 0);
    }
}
