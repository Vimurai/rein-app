//! `rein-app install --version X.Y.Z` (blueprint §API, §Core Concept pipeline):
//! lock → fetch → verify → runtime check → lay out → `rein install` → `rein doctor`.
//! Every step before the layout sync leaves `$HOME/.rein` untouched.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rein_release::{
    Http, InstallLock, InstallOutcome, InstallPaths, OwnerChecked, ReleaseSource, RunUid,
    SignatureVerifier, fetch_release, install_tree_locked, owner_guard_with, verify_release,
};
use semver::Version;

use crate::runner::{Process, ReinCommand, RunOutcome, RuntimeReport, install_then_doctor};

/// Exit codes (blueprint §API).
pub const EXIT_ERROR: u8 = 1;
pub const EXIT_VERIFY: u8 = 2;
pub const EXIT_DOCTOR: u8 = 3;
pub const EXIT_RUNTIME: u8 = 4;

/// Everything `install` talks to, injectable for tests.
pub struct Deps<'a> {
    pub http: &'a dyn Http,
    pub source: &'a ReleaseSource,
    pub verifier: &'a dyn SignatureVerifier,
    pub process: &'a dyn Process,
    /// The owner guard's uid source (`InstallLock::run_uid`; tests inject a mismatch).
    pub run_uid: RunUid,
}

/// A per-run download directory beside the Rein home, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// Lock-held write beside the Rein home: needs the owner guard's proof (E-11).
    fn create(paths: &InstallPaths, v: &Version, checked: &OwnerChecked<'_>) -> io::Result<Self> {
        checked
            .covers(paths)
            .map_err(|e| io::Error::other(e.to_string()))?;
        let dir = paths.home().join(format!(".rein.fetch-{v}"));
        // Ours by name; the install lock is held, so no other run is using it.
        match fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() => fs::remove_dir_all(&dir)?,
            Ok(_) => {
                return Err(io::Error::other(format!(
                    "{} exists and is not a directory",
                    dir.display()
                )));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        fs::create_dir(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Print an error line. Tests also see the lines their own thread reported, so a refusal
/// test can assert what was named (D-013 / E-11).
pub fn report(line: String) {
    #[cfg(test)]
    REPORTED.with(|r| r.borrow_mut().push(line.clone()));
    eprintln!("{line}");
}

#[cfg(test)]
thread_local! {
    pub static REPORTED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The lines [`report`]ed on this thread so far (the unix-only refusal tests read them).
#[cfg(all(test, unix))]
pub fn reported() -> String {
    REPORTED.with(|r| r.borrow().join("\n"))
}

/// What `install` reports after the layout step.
pub fn outcome_lines(outcome: &InstallOutcome, rein: &Path) -> Vec<String> {
    let mut lines = vec![format!(
        "rein-app: installed rein {} at {}",
        outcome.version,
        rein.display()
    )];
    if let Some(prev) = &outcome.previous {
        lines.push(format!(
            "rein-app: previous version kept at {} (`rein-app rollback` restores it)",
            prev.display()
        ));
    }
    if outcome.unsnapshotted {
        lines.push(
            "rein-app: no snapshot taken — $HOME/.rein had no valid VERSION, so this update cannot be rolled back"
                .to_string(),
        );
    }
    lines
}

/// The pipeline; returns the exit code.
pub fn install(home: &Path, version: &Version, deps: &Deps<'_>) -> u8 {
    let paths = InstallPaths::new(home);
    let lock = match InstallLock::acquire(&paths) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("rein-app: {e}");
            return EXIT_ERROR;
        }
    };
    // Owner guard (D-013): before anything is fetched or written; the lock is released.
    let checked = match owner_guard_with(&lock, deps.run_uid, &paths, &[]) {
        Ok(c) => c,
        Err(e) => {
            report(format!("rein-app: {e}"));
            return EXIT_ERROR;
        }
    };
    let scratch = match Scratch::create(&paths, version, &checked) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rein-app: cannot create the download directory: {e}");
            return EXIT_ERROR;
        }
    };

    println!("rein-app: fetching rein {version}");
    let assets = match fetch_release(deps.http, deps.source, version, &scratch.0) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rein-app: {e}");
            return EXIT_VERIFY;
        }
    };
    let release = match verify_release(&assets, deps.verifier, version, &scratch.0.join("tree")) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("rein-app: verification failed — {e}");
            return EXIT_VERIFY;
        }
    };
    println!(
        "rein-app: rein {version} verified (signature, manifest, {} files)",
        release.manifest.files.len()
    );

    let report = RuntimeReport::probe(deps.process, &release.manifest.min_node);
    print!("{}", report.table());
    if !report.ready() {
        eprintln!(
            "rein-app: required runtime missing — install it and re-run; nothing was changed"
        );
        return EXIT_RUNTIME;
    }

    let outcome = match install_tree_locked(&paths, &release, &checked) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("rein-app: {e}");
            return EXIT_ERROR;
        }
    };
    drop(scratch);
    for line in outcome_lines(&outcome, &paths.rein()) {
        println!("{line}");
    }

    // The lock is held through `rein install`/`rein doctor`: they write into the tree
    // (install-stamp, mcp node_modules), so no other install or rollback may sync it
    // meanwhile (E-5 security review P1-2). rein itself never reads the lock file.
    let result = install_then_doctor(deps.process, home);
    drop(lock);
    exit_for(result)
}

/// A failed `rein install` (exit 13).
pub const EXIT_REIN_INSTALL: u8 = 13;

/// Map how `rein install` / `rein doctor` ended to rein-app's exit code (blueprint §API,
/// D-014): a failed `rein install` is 13 with rein's own code printed, a failed
/// `rein doctor` is 3. No retry, no repair.
pub fn exit_for(result: RunOutcome) -> u8 {
    let kept = "the previous version, if any, is kept as $HOME/.rein.prev-* (`rein-app rollback` restores it)";
    match result {
        RunOutcome::Ok => 0,
        RunOutcome::Failed {
            cmd: ReinCommand::Install,
            code,
        } => {
            let why = match code {
                Some(3) => {
                    "exit 3: the installed tree does not match the manifest, or the gate pin"
                }
                Some(1) => "exit 1: Node below the floor, or /bin/bash -p missing",
                _ => "",
            };
            let code = code.map_or_else(|| "a signal".to_string(), |c| format!("exit {c}"));
            let why = if why.is_empty() {
                code
            } else {
                why.to_string()
            };
            eprintln!("rein-app: rein install failed ({why}); nothing was repaired; {kept}");
            EXIT_REIN_INSTALL
        }
        RunOutcome::Failed { code, .. } => {
            let code = code.map_or_else(|| "a signal".to_string(), |c| format!("exit {c}"));
            eprintln!("rein-app: rein doctor reported problems ({code}); {kept}");
            EXIT_DOCTOR
        }
        RunOutcome::NotStarted { cmd, reason } => {
            eprintln!("rein-app: could not start rein {cmd:?}: {reason}");
            if cmd == ReinCommand::Install {
                EXIT_REIN_INSTALL
            } else {
                EXIT_DOCTOR
            }
        }
    }
}

#[cfg(test)]
#[path = "../../rein-release/tests/support/mod.rs"]
pub(crate) mod support;

#[cfg(test)]
mod tests {
    use super::support::{Release, TestKey, VERSION, serve};
    use super::*;
    use crate::runner::Invocation;
    use rein_release::{HostPolicy, ReqwestHttp};
    use std::cell::RefCell;

    /// rein itself is stubbed: every tool is present; `rein` exits with the queued codes.
    struct FakeRein {
        node: Option<&'static str>,
        exits: RefCell<Vec<i32>>,
        ran: RefCell<Vec<Invocation>>,
    }

    impl FakeRein {
        fn new(exits: &[i32]) -> Self {
            Self {
                node: Some("v22.11.0"),
                exits: RefCell::new(exits.to_vec()),
                ran: RefCell::new(Vec::new()),
            }
        }
    }

    impl Process for FakeRein {
        fn run(&self, inv: &Invocation) -> io::Result<Option<i32>> {
            let home = &inv.set.iter().find(|(k, _)| *k == "HOME").unwrap().1;
            assert!(
                Path::new(home).join(".rein.lock").exists(),
                "the install lock is held while rein runs"
            );
            self.ran.borrow_mut().push(inv.clone());
            Ok(Some(self.exits.borrow_mut().remove(0)))
        }
        fn capture(&self, program: &str, _args: &[&str]) -> Option<String> {
            match program {
                "node" => self.node.map(str::to_string),
                _ => Some(format!("{program} 1.0")),
            }
        }
    }

    fn version() -> Version {
        Version::parse(VERSION).unwrap()
    }

    /// Run the pipeline against a fixture server serving `Release::good(signer)`, trusting `trusted`.
    fn run_with(signer: &TestKey, trusted: &TestKey, rein: &FakeRein, home: &Path) -> u8 {
        run_as(signer, trusted, rein, home, InstallLock::run_uid)
    }

    fn run_as(
        signer: &TestKey,
        trusted: &TestKey,
        rein: &FakeRein,
        home: &Path,
        run_uid: RunUid,
    ) -> u8 {
        let source = ReleaseSource {
            base_url: serve(Release::good(signer).routes()),
        };
        let http = ReqwestHttp::new(HostPolicy::loopback_only()).unwrap();
        let verifier = trusted.verifier();
        let deps = Deps {
            http: &http,
            source: &source,
            verifier: &verifier,
            process: rein,
            run_uid,
        };
        install(home, &version(), &deps)
    }

    fn leftovers(home: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(home)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn installs_then_runs_rein_install_and_doctor_from_the_new_tree() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let rein = FakeRein::new(&[0, 0]);
        assert_eq!(run_with(&key, &key, &rein, home.path()), 0);
        assert_eq!(
            leftovers(home.path()),
            [".rein"],
            "no lock, fetch or staging dir left"
        );
        assert_eq!(
            fs::read_to_string(home.path().join(".rein/VERSION")).unwrap(),
            "0.5.0\n"
        );
        let ran = rein.ran.borrow();
        assert_eq!(ran.len(), 2);
        assert_eq!(ran[0].program, home.path().join(".rein/bin/rein"));
        assert_eq!(
            (ran[0].args.clone(), ran[1].args.clone()),
            (vec!["install"], vec!["doctor"])
        );
    }

    #[test]
    fn a_bad_signature_exits_2_and_writes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let (signer, other) = (TestKey::generate(), TestKey::generate());
        let rein = FakeRein::new(&[]);
        assert_eq!(run_with(&signer, &other, &rein, home.path()), EXIT_VERIFY);
        assert!(leftovers(home.path()).is_empty());
        assert!(rein.ran.borrow().is_empty());
    }

    #[test]
    fn missing_node_exits_4_before_the_live_tree_is_touched() {
        let home = tempfile::tempdir().unwrap();
        let live = home.path().join(".rein");
        fs::create_dir_all(live.join("bin")).unwrap();
        fs::write(live.join("VERSION"), "0.4.0\n").unwrap();
        let key = TestKey::generate();
        let mut rein = FakeRein::new(&[]);
        rein.node = None;
        assert_eq!(run_with(&key, &key, &rein, home.path()), EXIT_RUNTIME);
        assert_eq!(fs::read_to_string(live.join("VERSION")).unwrap(), "0.4.0\n");
        assert_eq!(leftovers(home.path()), [".rein"]);
    }

    #[test]
    fn a_failing_doctor_exits_3_with_the_new_tree_in_place() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let rein = FakeRein::new(&[0, 1]);
        assert_eq!(run_with(&key, &key, &rein, home.path()), EXIT_DOCTOR);
        assert!(home.path().join(".rein/bin/rein").exists());
    }

    #[cfg(unix)]
    #[test]
    fn install_as_another_user_is_refused_naming_the_path() {
        let home = tempfile::tempdir().unwrap();
        let key = TestKey::generate();
        let rein = FakeRein::new(&[]);
        let foreign: RunUid = |l| l.run_uid().map(|u| u.wrapping_add(1));
        assert_eq!(run_as(&key, &key, &rein, home.path(), foreign), EXIT_ERROR);
        let said = reported();
        assert!(
            said.contains(&format!("{} belongs to uid", home.path().display())),
            "the refused path is named: {said}"
        );
        assert!(
            leftovers(home.path()).is_empty(),
            "nothing written, lock released"
        );
        assert!(rein.ran.borrow().is_empty());
    }

    #[test]
    fn the_no_snapshot_notice_is_reported() {
        let outcome = InstallOutcome {
            version: Version::new(0, 5, 0),
            previous: None,
            pruned: vec![],
            unsnapshotted: true,
        };
        let lines = outcome_lines(&outcome, Path::new("/h/.rein"));
        assert!(
            lines.iter().any(|l| l.contains("cannot be rolled back")),
            "{lines:?}"
        );
    }

    #[test]
    fn a_held_lock_exits_1_without_fetching() {
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join(".rein.lock"), "1").unwrap();
        let key = TestKey::generate();
        let rein = FakeRein::new(&[]);
        assert_eq!(run_with(&key, &key, &rein, home.path()), EXIT_ERROR);
        assert_eq!(leftovers(home.path()), [".rein.lock"]);
    }
}
