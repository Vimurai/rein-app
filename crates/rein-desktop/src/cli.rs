//! Health, release and profile actions (desktop.md §Components 5, §API, §Security 5, D-045).
//!
//! Release and profile actions run the `rein-app` CLI with a constant argv per action; the
//! window decides on the exit code only, so the install lock, owner guard, snapshot swap,
//! verdict and refusal texts are the CLI's own. Lines are streamed escaped and unchanged.
//! The only variable argv elements are profile names that match `[A-Za-z0-9._-]{1,64}`, start
//! with a letter or digit, and appear in `rein-app profiles list`; no key name, flag or
//! version from the web side ever reaches an argv. One run at a time per window.
//!
//! Health runs `rein doctor --scope install --json` from the home directory (the first
//! addendum to D-045) through the shared bounded run (4 MiB, 120 s) and the shared strict
//! parser (`rein_platform::doctor`); anything without a valid document is
//! `unknown` — never a pass, never a recomputed verdict, no branch on a check id.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rein_platform::doctor::{self, Status};
use serde::Serialize;

use crate::ipc::{IpcError, err};

/// What the window may ask the CLI to do (D-045). A closed set: no other argv exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    UpdateCheck,
    Update,
    Rollback,
    ProfilesList,
    /// Names already checked by [`check_profile_names`].
    ProfilesApply {
        dry_run: bool,
        names: Vec<String>,
    },
}

impl CliAction {
    pub fn argv(&self) -> Vec<String> {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        match self {
            Self::UpdateCheck => s(&["update", "--check"]),
            Self::Update => s(&["update", "--yes"]),
            Self::Rollback => s(&["rollback"]),
            Self::ProfilesList => s(&["profiles", "list"]),
            Self::ProfilesApply { dry_run, names } => {
                let mut v = s(&["profiles", "apply"]);
                if *dry_run {
                    v.push("--dry-run".into());
                }
                for n in names {
                    v.push("--profile".into());
                    v.push(n.clone());
                }
                v
            }
        }
    }
}

/// `[A-Za-z0-9._-]{1,64}`, and starting with a letter or digit: a name is placed after
/// `--profile`, and one starting with `-` could be read by the CLI's parser as a flag.
pub fn profile_name_ok(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.starts_with(|c: char| c.is_ascii_alphanumeric())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// The profile names in `rein-app profiles list` output: rows of the table only —
/// `NAME  present|missing  /config/dir` — so a stray line (a warning on stderr, which the
/// runner merges) can never be taken for a profile.
pub fn listed_profiles(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| !l.starts_with("PROFILE"))
        .filter_map(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            let row = cols.len() >= 3
                && matches!(cols[1], "present" | "missing")
                && (cols[2].starts_with('/') || cols[2].contains(":\\"));
            row.then(|| cols[0])
        })
        .filter(|n| profile_name_ok(n))
        .map(String::from)
        .collect()
}

/// Every requested name matches the pattern and is listed; empty means all listed.
pub fn check_profile_names(
    requested: &[String],
    listed: &[String],
) -> Result<Vec<String>, IpcError> {
    if requested.is_empty() {
        return Ok(listed.to_vec());
    }
    for n in requested {
        if !profile_name_ok(n) {
            return Err(err(
                "bad_profile",
                "a profile name may hold letters, digits and . _ - only",
            ));
        }
        if !listed.contains(n) {
            return Err(err("bad_profile", format!("no profile named {n}")));
        }
    }
    Ok(requested.to_vec())
}

/// A finished CLI run: its escaped lines and exit code (`None`: killed by a signal).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CliRun {
    pub lines: Vec<String>,
    pub code: Option<i32>,
}

/// The most lines one run reports; a longer output ends with a note.
pub const MAX_LINES: usize = 10_000;
/// The longest line kept, in bytes; the rest is cut.
pub const MAX_LINE: usize = 4096;

/// Starts the CLI and streams its lines; tests stub it.
pub trait Runner: Send + Sync {
    fn run(
        &self,
        program: &Path,
        args: &[String],
        line: &mut dyn FnMut(String),
    ) -> std::io::Result<Option<i32>>;
}

/// The real runner: stdin closed, `CLAUDECODE` unset, stdout and stderr read line by line
/// (merged in arrival order), each line escaped and bounded.
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(
        &self,
        program: &Path,
        args: &[String],
        line: &mut dyn FnMut(String),
    ) -> std::io::Result<Option<i32>> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_remove("CLAUDECODE")
            // On Windows the program is wsl.exe: nothing crosses into WSL (W2).
            .env_remove("WSLENV");
        hide_console(&mut cmd);
        let mut child = cmd.spawn()?;
        let (tx, rx) = mpsc::channel::<String>();
        let readers: Vec<_> = [
            child
                .stdout
                .take()
                .map(|o| Box::new(o) as Box<dyn Read + Send>),
            child
                .stderr
                .take()
                .map(|e| Box::new(e) as Box<dyn Read + Send>),
        ]
        .into_iter()
        .flatten()
        .map(|r| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for l in BufReader::new(r).split(b'\n').map_while(Result::ok) {
                    let text = String::from_utf8_lossy(&l)
                        .trim_end_matches('\r')
                        .to_string();
                    if tx.send(text).is_err() {
                        break;
                    }
                }
            })
        })
        .collect();
        drop(tx);
        let mut n = 0;
        for l in rx {
            n += 1;
            if n <= MAX_LINES {
                line(bounded(&l));
            } else if n == MAX_LINES + 1 {
                line("… (more output not shown)".into());
            }
        }
        for r in readers {
            let _ = r.join();
        }
        Ok(child.wait()?.code())
    }
}

fn bounded(l: &str) -> String {
    let mut cut = l.len().min(MAX_LINE);
    while !l.is_char_boundary(cut) {
        cut -= 1;
    }
    doctor::escape(&l[..cut])
}

use rein_platform::process::hide_console;

/// One CLI run at a time per window (desktop.md §API `release_apply`).
#[derive(Default)]
pub struct OneAtATime(AtomicBool);

impl OneAtATime {
    pub fn busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn begin(&self) -> Result<Busy<'_>, IpcError> {
        if self.0.swap(true, Ordering::AcqRel) {
            return Err(err(
                "busy",
                "another rein-app run is in progress — wait for it to end",
            ));
        }
        Ok(Busy(&self.0))
    }
}

/// Held while a run is in progress; dropping it ends the run.
pub struct Busy<'a>(&'a AtomicBool);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// The `rein-app` binary beside this one, else on `PATH`, resolved once (D-045).
#[cfg(unix)] // LocalHost (Linux, macOS); WslHost runs its own calls (W4)
pub fn find_rein_app(env_path: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    find_rein_app_in(beside.as_deref(), env_path)
}

/// [`find_rein_app`] with the window's directory given: beside it first, then the absolute
/// `PATH` entries in order (a relative entry would resolve against the working directory).
#[cfg(unix)] // LocalHost (Linux, macOS); WslHost runs its own calls (W4)
pub fn find_rein_app_in(
    beside: Option<&Path>,
    env_path: Option<std::ffi::OsString>,
) -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) {
        "rein-app.exe"
    } else {
        "rein-app"
    };
    let beside = beside.map(|d| d.join(name));
    beside.filter(|p| p.is_file()).or_else(|| {
        env_path.and_then(|p| {
            std::env::split_paths(&p)
                .filter(|d| d.is_absolute())
                .map(|d| d.join(name))
                .find(|p| p.is_file())
        })
    })
}

#[cfg(unix)] // LocalHost (Linux, macOS); WslHost runs its own calls (W4)
pub fn cli_missing() -> IpcError {
    err(
        "cli_missing",
        "rein-app is not installed beside the window — install the rein-app package",
    )
}

/// `HealthView` (desktop.md §Data Model): the document's own status and checks, escaped.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthView {
    /// `ok` | `warn` | `fail` | `unknown` (no valid document — never a pass).
    pub status: &'static str,
    pub checks: Vec<HealthCheck>,
    /// Why the status is `unknown`, in plain words.
    pub reason: Option<String>,
    /// Windows: WSL is missing or has no distro — the command the operator runs (the window
    /// never installs WSL or asks for elevation; the start screen shows it).
    pub setup: Option<String>,
    /// Seconds since the Unix epoch.
    pub taken_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthCheck {
    pub id: String,
    pub status: &'static str,
    pub line: String,
    pub fix: Option<String>,
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::Warn => "warn",
        Status::Fail => "fail",
    }
}

pub fn unknown(reason: impl Into<String>) -> HealthView {
    HealthView {
        status: "unknown",
        checks: Vec::new(),
        reason: Some(reason.into()),
        setup: None,
        taken_at: now(),
    }
}

/// Map one finished `rein doctor --json` run (exit code, stdout) to a `HealthView`. Exit 0
/// or 1 with a valid schema-1 document → its status; exit 1 must say `fail`; anything else
/// → `unknown`.
pub fn health_from(code: Option<i32>, stdout: &[u8]) -> HealthView {
    if !matches!(code, Some(0 | 1)) {
        return unknown("rein doctor did not finish normally");
    }
    match doctor::parse(stdout) {
        Ok(doc) if code == Some(1) && doc.status != Status::Fail => {
            unknown("rein doctor exited 1 but its status is not fail")
        }
        Ok(doc) => HealthView {
            status: status_word(doc.status),
            checks: doc
                .checks
                .iter()
                .map(|c| HealthCheck {
                    id: doctor::escape(&c.id),
                    status: status_word(c.status),
                    line: c
                        .line
                        .split('\n')
                        .map(doctor::escape)
                        .collect::<Vec<_>>()
                        .join(" / "),
                    fix: c.fix.as_deref().map(doctor::escape),
                })
                .collect(),
            reason: None,
            setup: None,
            taken_at: now(),
        },
        Err(e) => unknown(format!(
            "rein doctor printed no valid document: {}",
            doctor::escape(&e)
        )),
    }
}

/// `<home>/.rein/bin/rein doctor --scope install --json` with the shared bounded run (4 MiB,
/// 120 s).
#[cfg(unix)] // LocalHost (Linux, macOS); WslHost runs its own calls (W4)
pub fn local_doctor(home: &Path) -> HealthView {
    local_doctor_within(home, DOCTOR_LIMIT)
}

/// [`local_doctor`] with the time limit given (tests use a short one).
#[cfg(unix)] // LocalHost (Linux, macOS); WslHost runs its own calls (W4)
pub fn local_doctor_within(home: &Path, limit: Duration) -> HealthView {
    let rein = home.join(".rein/bin/rein");
    if !rein.is_file() {
        return unknown("rein is not installed — install it with `rein-app install`");
    }
    let mut cmd = Command::new(&rein);
    // The install scope (the document the CLI's own install verdict reads, D-029): the
    // statusline says "install ok / FAIL", and a full run judges whatever directory it runs
    // in as a project. From the home directory, so the window's start directory never counts.
    cmd.args(DOCTOR_ARGS)
        .current_dir(home)
        .stderr(Stdio::null())
        .env_remove("CLAUDECODE");
    hide_console(&mut cmd);
    match rein_platform::process::output(&mut cmd, limit, DOCTOR_CAP) {
        Ok(Some((code, out))) => health_from(code, &out),
        Ok(None) => unknown(format!(
            "rein doctor did not finish within {} seconds",
            limit.as_secs()
        )),
        Err(e) => unknown(format!("cannot run rein doctor: {e}")),
    }
}

/// `doctor --scope install --json` (contract §2.6; the CLI's install check runs it too).
#[cfg(unix)] // LocalHost (Linux, macOS); WslHost runs its own calls (W4)
pub const DOCTOR_ARGS: [&str; 4] = ["doctor", "--scope", "install", "--json"];
pub const DOCTOR_LIMIT: Duration = Duration::from_secs(120);
pub const DOCTOR_CAP: u64 = 4 * 1024 * 1024;

/// What `update --check` said: the two leading lines' values (the only text read, D-045).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReleaseCheck {
    pub installed: Option<String>,
    pub newest: Option<String>,
    /// Everything the CLI printed, escaped (the notes are shown as text).
    pub lines: Vec<String>,
    /// 0 newer · 10 current · 11 offline · anything else: an error the lines explain.
    pub code: Option<i32>,
}

pub fn release_check_from(run: CliRun) -> ReleaseCheck {
    let value = |prefix: &str| {
        run.lines
            .iter()
            .take(2)
            .find_map(|l| l.strip_prefix(prefix))
            .map(|v| v.trim().to_string())
            .filter(|v| v.len() <= 64 && v.bytes().all(|b| b.is_ascii_graphic()))
    };
    ReleaseCheck {
        installed: value("installed: "),
        newest: value("newest: "),
        lines: run.lines.clone(),
        code: run.code,
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(status: &str, checks: &[(&str, &str, Option<&str>)]) -> Vec<u8> {
        let rows: Vec<serde_json::Value> = checks
            .iter()
            .map(|(id, st, fix)| {
                serde_json::json!({"id": id, "scope": "install", "subject": null,
                "status": st, "line": format!("{id} line"), "fix": fix})
            })
            .collect();
        serde_json::json!({"schema": 1, "version": "0.5.0", "status": status, "checks": rows})
            .to_string()
            .into_bytes()
    }

    #[test]
    fn health_is_the_documents_status_and_garbage_is_unknown() {
        let ok = health_from(Some(0), &doc("ok", &[("tree", "ok", None)]));
        assert_eq!(ok.status, "ok");
        assert_eq!(ok.checks[0].line, "tree line");
        let warn = health_from(
            Some(0),
            &doc("warn", &[("node", "warn", Some("install node 22"))]),
        );
        assert_eq!(warn.status, "warn");
        assert_eq!(warn.checks[0].fix.as_deref(), Some("install node 22"));
        assert_eq!(
            health_from(Some(1), &doc("fail", &[("tree", "fail", None)])).status,
            "fail"
        );
        // A host failure in an `ok` document stays ok: no verdict is recomputed (D-029).
        assert_eq!(
            health_from(Some(0), &doc("ok", &[("host_x", "fail", None)])).status,
            "ok"
        );
        for (code, out) in [
            (Some(0), b"garbage".to_vec()),
            (Some(2), doc("ok", &[])),
            (None, doc("ok", &[])),
            (Some(1), doc("ok", &[])),
            (Some(0), b"{}".to_vec()),
        ] {
            let h = health_from(code, &out);
            assert_eq!(h.status, "unknown", "{code:?}");
            assert!(h.reason.is_some());
        }
        let esc = health_from(
            Some(0),
            &doc("ok", &[("tree", "ok", Some("run \u{1b}[31mx"))]),
        );
        assert_eq!(esc.checks[0].fix.as_deref(), Some("run \\u{1b}[31mx"));
    }

    #[test]
    fn every_action_has_its_exact_argv() {
        let names = vec!["work".to_string(), "personal".to_string()];
        let cases = [
            (CliAction::UpdateCheck, vec!["update", "--check"]),
            (CliAction::Update, vec!["update", "--yes"]),
            (CliAction::Rollback, vec!["rollback"]),
            (CliAction::ProfilesList, vec!["profiles", "list"]),
            (
                CliAction::ProfilesApply {
                    dry_run: true,
                    names: names.clone(),
                },
                vec![
                    "profiles",
                    "apply",
                    "--dry-run",
                    "--profile",
                    "work",
                    "--profile",
                    "personal",
                ],
            ),
            (
                CliAction::ProfilesApply {
                    dry_run: false,
                    names,
                },
                vec![
                    "profiles",
                    "apply",
                    "--profile",
                    "work",
                    "--profile",
                    "personal",
                ],
            ),
        ];
        for (action, want) in cases {
            assert_eq!(action.argv(), want, "{action:?}");
        }
    }

    #[test]
    fn profile_names_are_checked_before_any_process_starts() {
        let lines: Vec<String> = [
            "PROFILE          SETTINGS CONFIG DIR",
            "default          present  /home/me/.claude",
            "work             present  /home/me/.claude-work",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let mut with_noise = lines.clone();
        with_noise.push("notice something".into());
        with_noise.push("skipping the rest for now".into());
        let listed = listed_profiles(&with_noise);
        assert_eq!(listed, ["default", "work"], "only table rows are profiles");
        assert_eq!(
            check_profile_names(&[], &listed).unwrap(),
            listed,
            "empty = all"
        );
        assert!(check_profile_names(&["work".into()], &listed).is_ok());
        for bad in ["hooks", "--enable", "a b", "x;rm", "", &"a".repeat(65)] {
            assert_eq!(
                check_profile_names(&[bad.to_string()], &listed)
                    .unwrap_err()
                    .kind,
                "bad_profile",
                "{bad:?}"
            );
        }
    }

    #[test]
    fn one_run_at_a_time() {
        let g = OneAtATime::default();
        let first = g.begin().unwrap();
        assert_eq!(g.begin().err().unwrap().kind, "busy");
        drop(first);
        assert!(g.begin().is_ok(), "free again once the run ends");
    }

    #[test]
    fn the_check_reads_only_the_two_leading_values() {
        let lines: Vec<String> = [
            "installed: 0.5.0",
            "newest: 0.5.1",
            "newest: 9.9.9",
            "notes",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let c = release_check_from(CliRun {
            lines,
            code: Some(0),
        });
        assert_eq!(c.installed.as_deref(), Some("0.5.0"));
        assert_eq!(c.newest.as_deref(), Some("0.5.1"));
        let none = release_check_from(CliRun {
            lines: vec!["rein-app: offline".into()],
            code: Some(11),
        });
        assert_eq!((none.installed, none.newest), (None, None));
    }

    #[test]
    #[cfg(unix)]
    fn rein_app_is_found_beside_the_window_first_then_on_absolute_path_entries() {
        use std::os::unix::fs::PermissionsExt;
        let make = |dir: &Path| {
            std::fs::create_dir_all(dir).unwrap();
            let p = dir.join("rein-app");
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };
        let t = tempfile::tempdir().unwrap();
        let beside = t.path().join("bin");
        let on_path = t.path().join("path");
        let path_var = std::env::join_paths(["relative/dir".into(), on_path.clone()]).unwrap();
        // Nothing anywhere.
        assert_eq!(
            find_rein_app_in(Some(&beside), Some(path_var.clone())),
            None
        );
        // Only on PATH (the relative entry is skipped).
        let p = make(&on_path);
        assert_eq!(
            find_rein_app_in(Some(&beside), Some(path_var.clone())),
            Some(p)
        );
        // Beside the window wins over PATH.
        let b = make(&beside);
        assert_eq!(find_rein_app_in(Some(&beside), Some(path_var)), Some(b));
        // A relative PATH entry is never searched, even when it holds a rein-app.
        let cwd_like = std::ffi::OsString::from("relative/dir");
        assert_eq!(find_rein_app_in(None, Some(cwd_like)), None);
    }

    #[test]
    #[cfg(unix)]
    fn local_doctor_runs_the_install_scope_from_home_and_maps_failures_to_unknown() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let home = t.path();
        assert_eq!(local_doctor(home).status, "unknown", "rein not installed");
        let bin = home.join(".rein/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("rein");
        let write = |body: &str| {
            std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        // The argv and cwd are what the document records; status ok only with both right.
        let doc = r#"{"schema":1,"version":"0.5.0","status":"ok","checks":[]}"#;
        write(&format!(
            "[ \"$*\" = \"doctor --scope install --json\" ] && [ \"$(pwd -P)\" = \"{}\" ] && printf '%s' '{doc}' || exit 2",
            home.canonicalize().unwrap().display()
        ));
        assert_eq!(local_doctor(home).status, "ok");
        write("echo not json");
        assert_eq!(local_doctor(home).status, "unknown");
        write("sleep 5");
        let slow = local_doctor_within(home, Duration::from_millis(300));
        assert_eq!(slow.status, "unknown");
        assert!(slow.reason.unwrap().contains("did not finish"));
    }

    #[test]
    #[cfg(unix)]
    fn the_system_runner_streams_both_streams_escaped() {
        let mut got = Vec::new();
        let code = SystemRunner
            .run(
                Path::new("/bin/sh"),
                &[
                    "-c".into(),
                    "echo out; echo err 1>&2; printf 'esc \\033[2J\\n'; exit 13".into(),
                ],
                &mut |l| got.push(l),
            )
            .unwrap();
        assert_eq!(code, Some(13), "the exit code unchanged");
        got.sort();
        assert_eq!(got, ["err", "esc \\u{1b}[2J", "out"]);
    }
}
