//! The Project sheet's Team section (desktop.md §Look and feel v2 V10, D-058, E-64; the ruling
//! "after the read of rein 0.5.0"; the security review's constraints C1–C8).
//!
//! A form over rein's own team commands: `rein team show` and `rein trunk` are shown as rein
//! printed them; `rein team add` and `rein state migrate --team | --legacy` run on a click. The
//! app reads and writes no team file and names no mode: the sheet offers by the shape of
//! `team show`'s output.
//!
//! This is the first place user-typed text becomes part of a command. Each value is checked
//! here — the only check that counts — and passed as its own argv word: directly to `rein` on
//! Linux / macOS, and on Windows as positional words after a constant, quote-free script that
//! sets `IFS=` and `noglob` (a login shell's rc may have set them) and checks them again
//! ([`ADD_SCRIPT`]). The range is formatted here from an enum and two numbers; no range text
//! comes from the web side. A migrate needs the operator's yes in a native dialog shown by
//! this process, so a web view cannot run one by itself.

use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::ipc::{Desktop, IpcError, err};
use crate::project_setup::{ReinRun, run_lines};

/// `rein team show`, `rein trunk`, `rein team add` (V10 "Execution constraints").
pub const RUN_LIMIT: Duration = Duration::from_secs(60);
/// `rein state migrate`.
pub const MIGRATE_LIMIT: Duration = Duration::from_secs(300);
/// The most lines of rein's output one answer carries.
pub const MAX_LINES: usize = 200;

/// The one sentence for a refused value (V10: fixed, whatever was wrong).
pub const REFUSED: &str = "Use an e-mail address of letters, digits and . _ + - (one @), and a range of whole numbers from 1 to 99999 with the first not above the second.";

/// rein's fixed line when the project has no `.ai/team.json`.
pub const NO_TEAM_LINE: &str =
    "no .ai/team.json — ids are allocated max+1 (single-operator project)";

/// The Windows script (security review C3): `$1` the e-mail, `$2` the range, after the
/// literal `rein-app` in the `$0` slot. Checked again before `exec`; quote-free (V8 3b).
/// Exit codes 64-66, 68, 69: `wsl.exe` 3.0.5 turns a Linux exit 67 (and 87) into 255 with
/// "User not found" (seen in E-64's interop), so 67 is skipped.
pub const ADD_SCRIPT: &str = "IFS=; set -o noglob; case $# in 2) ;; *) exit 64;; esac; case $1 in -*|*@*@*|@*|*@|*[!A-Za-z0-9._+@-]*) exit 65;; esac; case $1 in ?*@?*) ;; *) exit 66;; esac; case $2 in [EP]:[0-9]*-[0-9]*) ;; *) exit 69;; esac; case $2 in *[!EP0-9:-]*|*:*:*|*-*-*|?:*[!0-9-]*|*[!0-9]) exit 68;; esac; exec ~/.rein/bin/rein team add $1 --range $2";
/// The `$0` word before the values (security review: without it the e-mail would be `$0`).
pub const ADD_ARGV0: &str = "rein-app";

/// rein's task prefixes the sheet offers (the ruling: a choice, not typed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Prefix {
    E,
    P,
}

impl Prefix {
    fn letter(self) -> char {
        match self {
            Self::E => 'E',
            Self::P => 'P',
        }
    }
}

/// The two migrate directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Team,
    Legacy,
}

impl Mode {
    fn run(self) -> ReinRun {
        match self {
            Self::Team => ReinRun::MigrateTeam,
            Self::Legacy => ReinRun::MigrateLegacy,
        }
    }

    /// The confirm's sentence (V10, the ruling; provisional until rein has a dry run).
    pub fn confirm_text(self) -> &'static str {
        match self {
            Self::Team => {
                "rein turns this project's task list into one file per task, writes .ai/team.json with you as the planner, and stops tracking TASKS.md. The project must have no uncommitted changes. Commit the result afterwards. Close and reopen the project so its agents pick up the change. The Tasks panel does not read team projects yet."
            }
            Self::Legacy => {
                "rein folds the task files back into one list and removes .ai/team.json with every developer's id range. The project must have no uncommitted changes. Commit the result afterwards. Close and reopen the project so its agents pick up the change."
            }
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Team => "Turn on team mode?",
            Self::Legacy => "Go back to single-developer mode?",
        }
    }
}

/// An e-mail as the sheet accepts it (C1): `[A-Za-z0-9._+-]{1,64}@[A-Za-z0-9.-]{1,190}`,
/// exactly one `@`, not starting with `-`.
pub fn email_ok(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    let local_ok = (1..=64).contains(&local.len())
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'));
    let domain_ok = (1..=190).contains(&domain.len())
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
    local_ok && domain_ok && !s.starts_with('-')
}

/// The checked values of one `rein team add`, ready as argv words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Add {
    pub email: String,
    /// `P:from-to`, formatted here.
    pub spec: String,
}

/// Check the form's values (C1); `Err` is [`REFUSED`].
pub fn add_values(email: &str, prefix: Prefix, from: u32, to: u32) -> Result<Add, &'static str> {
    let range = 1..=99_999;
    if !email_ok(email) || !range.contains(&from) || !range.contains(&to) || from > to {
        return Err(REFUSED);
    }
    Ok(Add {
        email: email.to_string(),
        spec: format!("{}:{from}-{to}", prefix.letter()),
    })
}

/// What `team show` printed, by its shape (the ruling): the app offers by it and names no mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// The first line starts `planner: ` — offer Go back to single-developer mode.
    Team,
    /// Exactly rein's fixed line — offer Turn on team mode.
    Single,
    /// Anything else — the text only, no button.
    Other,
}

/// The shape of `team show`'s raw output (exit 0 only).
pub fn shape(code: Option<i32>, lines: &[String]) -> Shape {
    let lines: Vec<&str> = lines
        .iter()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty())
        .collect();
    match (code, lines.as_slice()) {
        (Some(0), [only]) if *only == NO_TEAM_LINE => Shape::Single,
        (Some(0), [first, ..]) if first.starts_with("planner: ") => Shape::Team,
        _ => Shape::Other,
    }
}

/// One finished run: its exit code and its lines, escaped and capped.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Run {
    pub code: Option<i32>,
    pub lines: Vec<String>,
    /// More lines than [`MAX_LINES`] were printed.
    pub cut: bool,
    /// Killed at the limit.
    pub timed_out: bool,
}

fn run(cmd: std::process::Command, limit: Duration) -> Result<(Run, Vec<String>), IpcError> {
    let never = AtomicBool::new(false);
    let mut raw = Vec::new();
    let (code, stopped) = run_lines(cmd, limit, &never, &mut |l| raw.push(l))
        .map_err(|e| err("cli", format!("cannot run rein: {e}")))?;
    let shown: Vec<String> = raw
        .iter()
        .take(MAX_LINES)
        .map(|l| crate::cli::bounded(l))
        .collect();
    Ok((
        Run {
            code,
            lines: shown,
            cut: raw.len() > MAX_LINES,
            timed_out: stopped.is_some(),
        },
        raw,
    ))
}

/// `team_get`'s answer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TeamView {
    pub show: Run,
    pub trunk: Run,
    pub shape: Shape,
}

/// The project of a started tab of this window, from its recorded state (C4).
fn project(d: &Desktop, tab: u32) -> Result<std::path::PathBuf, IpcError> {
    if d.projects.setup_tab(tab).is_some() {
        return Err(err(
            "not_set_up",
            "this folder is not set up for Rein — the tab offers set-up",
        ));
    }
    d.tab_project(tab)
        .ok_or_else(|| err("unknown_tab", "the tab is not open"))
}

pub fn get(d: &Desktop, tab: u32) -> Result<TeamView, IpcError> {
    let p = project(d, tab)?;
    let (show, raw) = run(d.platform.rein_command(&p, ReinRun::TeamShow)?, RUN_LIMIT)?;
    let (trunk, _) = run(d.platform.rein_command(&p, ReinRun::Trunk)?, RUN_LIMIT)?;
    let shape = shape(show.code, &raw);
    Ok(TeamView { show, trunk, shape })
}

pub fn add(d: &Desktop, tab: u32, values: &Add) -> Result<Run, IpcError> {
    let p = project(d, tab)?;
    let (r, _) = run(d.platform.rein_team_add(&p, values)?, RUN_LIMIT)?;
    Ok(r)
}

/// `team_mode`'s answer: rein's run, or the operator said no.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ModeEnd {
    Ran(Run),
    Declined { declined: bool },
}

/// Run a migrate after `confirm` (the native dialog) said yes.
pub fn mode(
    d: &Desktop,
    tab: u32,
    mode: Mode,
    confirm: &dyn Fn(&str, &str) -> bool,
) -> Result<ModeEnd, IpcError> {
    let p = project(d, tab)?;
    if !confirm(mode.title(), mode.confirm_text()) {
        return Ok(ModeEnd::Declined { declined: true });
    }
    let (r, _) = run(d.platform.rein_command(&p, mode.run())?, MIGRATE_LIMIT)?;
    Ok(ModeEnd::Ran(r))
}

/// The native yes/no dialog (C5): shown by this process, not by the web view.
fn native_confirm(app: &AppHandle, title: &str, text: &str) -> bool {
    let dialog = |title: String, text: String| {
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(text)
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    };
    #[cfg(target_os = "macos")]
    {
        // AppKit dialogs run on the main thread.
        let (tx, rx) = std::sync::mpsc::channel();
        let (t, x) = (title.to_string(), text.to_string());
        if app
            .run_on_main_thread(move || {
                let _ = tx.send(dialog(t, x));
            })
            .is_err()
        {
            return false;
        }
        rx.recv().unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        dialog(title.to_string(), text.to_string())
    }
}

#[tauri::command(async)]
pub fn team_get(
    state: State<'_, Desktop>,
    cli_state: State<'_, crate::cli_ipc::CliState>,
    tab: u32,
) -> Result<TeamView, IpcError> {
    let _busy = cli_state.busy.begin()?;
    get(&state, tab)
}

#[tauri::command(async)]
pub fn team_add(
    state: State<'_, Desktop>,
    cli_state: State<'_, crate::cli_ipc::CliState>,
    tab: u32,
    email: String,
    prefix: Prefix,
    from: u32,
    to: u32,
) -> Result<Run, IpcError> {
    let values = add_values(&email, prefix, from, to).map_err(|why| err("refused", why))?;
    let _busy = cli_state.busy.begin()?;
    add(&state, tab, &values)
}

#[tauri::command(async)]
pub fn team_mode(
    app: AppHandle,
    state: State<'_, Desktop>,
    cli_state: State<'_, crate::cli_ipc::CliState>,
    tab: u32,
    mode: Mode,
) -> Result<ModeEnd, IpcError> {
    let _busy = cli_state.busy.begin()?;
    self::mode(&state, tab, mode, &|t, x| native_confirm(&app, t, x))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V10's validation table and the security review's additions (C8).
    #[test]
    fn the_validation_table() {
        let ok = |e: &str| add_values(e, Prefix::E, 1, 2).is_ok();
        for e in [
            "dev@example.com",
            "a.b+c_d-e@sub.example-x.io",
            ".lead@x",
            "+tag@x",
            &format!("{}@{}", "a".repeat(64), "b".repeat(190)),
        ] {
            assert!(ok(e), "{e}");
        }
        let long = format!("{}@x.io", "a".repeat(300));
        for e in [
            "",
            "plain",
            "-dev@x.io",
            "--range",
            "--range=E:1-2",
            "@x.io",
            "dev@",
            "a@b@c",
            "dev @x.io",
            "dev@x .io",
            "dev\t@x.io",
            "dev\n@x.io",
            "\"dev\"@x.io",
            "'dev'@x.io",
            "d$v@x.io",
            "d`v`@x.io",
            "d;v@x.io",
            "d|v@x.io",
            "d&v@x.io",
            "d%v@x.io",
            "d:v@x.io",
            "dé@x.io",
            "dev@x.ío",
            "d*v@x.io",
            "d?v@x.io",
            "d[v]@x.io",
            "d\\v@x.io",
            &long,
            &format!("{}@x", "a".repeat(65)),
            &format!("a@{}", "b".repeat(191)),
        ] {
            assert!(!ok(e), "{e:?}");
        }
        assert_eq!(add_values("d@x", Prefix::P, 0, 0), Err(REFUSED));
        assert_eq!(add_values("d@x", Prefix::P, 5, 1), Err(REFUSED));
        assert_eq!(add_values("d@x", Prefix::P, 1, 100_000), Err(REFUSED));
        assert_eq!(
            add_values("d@x", Prefix::P, 425, 524).unwrap().spec,
            "P:425-524"
        );
        assert_eq!(add_values("d@x", Prefix::E, 7, 7).unwrap().spec, "E:7-7");
        // The prefix is a closed choice: anything else does not deserialise.
        assert!(serde_json::from_str::<Prefix>("\"X\"").is_err());
        assert!(serde_json::from_str::<Prefix>("\"E:1-2\"").is_err());
        assert!(serde_json::from_str::<Mode>("\"team --trunk x\"").is_err());
    }

    #[test]
    fn the_shape_of_team_show() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(shape(Some(0), &l(&[NO_TEAM_LINE])), Shape::Single);
        assert_eq!(
            shape(
                Some(0),
                &l(&[
                    "planner: a@x   claim_ttl_days: 7   shared_log: false",
                    "  a@x (planner)  E:1-100 (3/100 used)"
                ])
            ),
            Shape::Team
        );
        // Anything else: the text only — rc noise first, a non-zero exit, a near miss.
        assert_eq!(
            shape(Some(0), &l(&["Welcome!", NO_TEAM_LINE])),
            Shape::Other
        );
        assert_eq!(shape(Some(1), &l(&[NO_TEAM_LINE])), Shape::Other);
        assert_eq!(shape(None, &l(&["planner: a@x"])), Shape::Other);
        assert_eq!(shape(Some(0), &l(&["no .ai/team.json"])), Shape::Other);
        assert_eq!(shape(Some(0), &[]), Shape::Other);
    }

    /// C3: quote-free, no backslash, no command substitution; the guard comes first; every
    /// value reaches rein as one word.
    #[test]
    fn the_windows_script_is_constant_and_guarded() {
        assert!(!ADD_SCRIPT.contains(['"', '\'', '\\', '`', '\n']));
        assert!(!ADD_SCRIPT.contains("$("));
        assert!(ADD_SCRIPT.starts_with("IFS=; set -o noglob;"));
        assert!(ADD_SCRIPT.ends_with("exec ~/.rein/bin/rein team add $1 --range $2"));
    }

    /// The script at its process boundary under a hostile rc: `sh`, `bash` and `zsh` (when
    /// present) with IFS set to the values' own characters and globbing options on; each
    /// value arrives as one word, and bad values exit 64–68 before rein runs.
    #[cfg(unix)]
    #[test]
    fn the_windows_script_under_a_hostile_rc() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".rein/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(
            bin.join("rein"),
            "#!/bin/sh\nfor a in \"$@\"; do printf '[%s]' \"$a\"; done\n",
        )
        .unwrap();
        std::fs::set_permissions(bin.join("rein"), std::fs::Permissions::from_mode(0o755)).unwrap();
        // What an rc the login shell reads before the script may leave set (security review
        // P1): IFS made of the values' own characters, splitting and globbing options on.
        let rc = "IFS=.@:+-; setopt GLOB_SUBST SH_WORD_SPLIT EXTENDED_GLOB KSH_GLOB 2>/dev/null; shopt -s extglob nullglob 2>/dev/null; ";
        let shells: Vec<&str> = ["/bin/sh", "/bin/bash", "/bin/zsh", "/usr/bin/zsh"]
            .into_iter()
            .filter(|s| std::path::Path::new(s).exists())
            .collect();
        for sh in shells {
            let run = |args: &[&str]| {
                let o = std::process::Command::new(sh)
                    .args(["-c", &format!("{rc}{ADD_SCRIPT}")])
                    .args(args)
                    .env_clear()
                    .env("HOME", home.path())
                    .env("PATH", "/usr/bin:/bin")
                    .output()
                    .unwrap();
                (
                    o.status.code(),
                    String::from_utf8_lossy(&o.stdout).into_owned(),
                )
            };
            assert_eq!(
                run(&[ADD_ARGV0, "a.b+c@d-e.f", "E:1-2"]),
                (
                    Some(0),
                    "[team][add][a.b+c@d-e.f][--range][E:1-2]".to_string()
                ),
                "{sh}"
            );
            for (bad, code) in [
                (vec![ADD_ARGV0, "a@b"], 64),
                (vec![ADD_ARGV0, "-a@b", "E:1-2"], 65),
                (vec![ADD_ARGV0, "a@b@c", "E:1-2"], 65),
                (vec![ADD_ARGV0, "a b@c", "E:1-2"], 65),
                (vec![ADD_ARGV0, "a*@c", "E:1-2"], 65),
                (vec![ADD_ARGV0, "nobody", "E:1-2"], 66),
                (vec![ADD_ARGV0, "a@b", "X:1-2"], 69),
                (vec![ADD_ARGV0, "a@b", "E:1-2;id"], 68),
                (vec![ADD_ARGV0, "a@b", "E:1-2-3"], 68),
                (vec![ADD_ARGV0, "a@b", "E:1-2E"], 68),
                (vec![ADD_ARGV0, "a@b", "E:1P-2"], 68),
            ] {
                assert_eq!(run(&bad).0, Some(code), "{sh} {bad:?}");
            }
        }
    }

    /// C2: on Linux / macOS five argv words straight to rein, no shell, in the project.
    #[cfg(unix)]
    #[test]
    fn local_add_is_five_words_and_no_shell() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".rein/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("rein"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(bin.join("rein"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let h = crate::local_host::LocalHost::new(Some(home.path().into()));
        let p = tempfile::tempdir().unwrap();
        let v = add_values("dev@x.io", Prefix::E, 1, 100).unwrap();
        let cmd = crate::host::Host::rein_team_add(&h, p.path(), &v).unwrap();
        assert_eq!(cmd.get_program(), bin.join("rein"));
        let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(args, ["team", "add", "dev@x.io", "--range", "E:1-100"]);
        assert_eq!(cmd.get_current_dir(), Some(p.path()));
    }

    /// C5: no yes in the native dialog, no process; a tab still waiting for set-up has no team.
    #[cfg(unix)]
    #[test]
    fn a_migrate_needs_the_native_yes_and_a_set_up_project() {
        use crate::pty::{Events, PtyId};
        struct Quiet;
        impl Events for Quiet {
            fn output(&self, _: PtyId, _: Vec<u8>) {}
            fn exit(&self, _: PtyId, _: u32) {}
        }
        // No rein installed: any run would fail with rein_missing — so "declined" proves
        // nothing ran.
        let d = Desktop::new(Box::new(crate::local_host::LocalHost::new(Some(
            "/nonexistent-home".into(),
        ))));
        let proj = tempfile::tempdir().unwrap();
        std::fs::create_dir(proj.path().join(".ai")).unwrap();
        let spec = crate::pty::ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 5".into()],
            cwd: Some(proj.path().into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        let tab = d
            .open_spec(
                &spec,
                proj.path(),
                crate::pty::Size { cols: 80, rows: 24 },
                std::sync::Arc::new(Quiet),
            )
            .unwrap();
        let asked = std::cell::Cell::new(None);
        let no = |t: &str, x: &str| {
            asked.set(Some((t.to_string(), x.to_string())));
            false
        };
        assert_eq!(
            mode(&d, tab.id, Mode::Legacy, &no).unwrap(),
            ModeEnd::Declined { declined: true }
        );
        let (title, text) = asked.take().unwrap();
        assert_eq!(title, Mode::Legacy.title());
        assert!(text.contains("removes .ai/team.json with every developer's id range"));
        assert_eq!(
            mode(&d, tab.id, Mode::Team, &|_, _| true).unwrap_err().kind,
            "rein_missing"
        );
        assert_eq!(get(&d, tab.id + 9).unwrap_err().kind, "unknown_tab");
        // A folder without .ai/: the set-up tab answers "not set up", the card offers set-up.
        let plain = tempfile::tempdir().unwrap();
        let id = match d
            .open(
                plain.path().to_str().unwrap(),
                80,
                24,
                std::sync::Arc::new(Quiet),
                crate::tmux_server::Server::User,
            )
            .unwrap()
        {
            crate::project_setup::Opened::SetUp { setup } => setup.id,
            other => panic!("{other:?}"),
        };
        assert_eq!(get(&d, id).unwrap_err().kind, "not_set_up");
        let v = add_values("a@b", Prefix::E, 1, 2).unwrap();
        assert_eq!(add(&d, id, &v).unwrap_err().kind, "not_set_up");
        d.detach(id);
        d.detach(tab.id);
    }

    #[test]
    fn the_answers_shapes() {
        let r = Run {
            code: Some(1),
            lines: vec!["✗ [RANGE_OVERLAP] x".into()],
            cut: false,
            timed_out: false,
        };
        assert_eq!(
            serde_json::to_value(ModeEnd::Ran(r.clone())).unwrap(),
            serde_json::json!({"code": 1, "lines": ["✗ [RANGE_OVERLAP] x"], "cut": false, "timed_out": false})
        );
        assert_eq!(
            serde_json::to_value(ModeEnd::Declined { declined: true }).unwrap(),
            serde_json::json!({"declined": true})
        );
    }
}
