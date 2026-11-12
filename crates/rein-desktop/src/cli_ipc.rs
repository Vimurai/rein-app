//! IPC for health, release and profiles (desktop.md §API, E-39, D-045): `health_get`,
//! `release_check`, `release_apply`, `profiles_plan`, `profiles_apply`; event
//! `health_changed`. Every run goes through the host (`LocalHost` or `WslHost`) and the
//! one-run-at-a-time guard; the web side can name an action and profile names, never a
//! program, flag, key or version.

use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, State};

use crate::cli::{self, CliAction, CliRun, HealthView, OneAtATime, ReleaseCheck};
use crate::host::Host;
use crate::ipc::{Desktop, IpcError, err};

pub const EVENT_HEALTH_CHANGED: &str = "health_changed";

#[derive(Default)]
pub struct CliState {
    busy: OneAtATime,
    health: Mutex<Option<HealthView>>,
}

impl CliState {
    /// The last health view, or a fresh one; a refresh tells the web side.
    pub fn health(&self, host: &dyn Host, refresh: bool) -> (HealthView, bool) {
        let mut h = self.health.lock().unwrap_or_else(|e| e.into_inner());
        // While an update or rollback swaps the tree, doctor is not run against it; the
        // refresh after the run reports the result.
        let hold = self.busy.busy();
        if let (true, Some(v)) = (!refresh || hold, h.as_ref()) {
            return (v.clone(), false);
        }
        if hold {
            return (cli::unknown("an update or rollback is in progress"), false);
        }
        let v = host.doctor();
        *h = Some(v.clone());
        (v, true)
    }

    /// Run `action` to completion, collecting its lines.
    pub fn collect(&self, host: &dyn Host, action: &CliAction) -> Result<CliRun, IpcError> {
        let _busy = self.busy.begin()?;
        let mut lines = Vec::new();
        let code = host.run_cli(action, &mut |l| lines.push(l))?;
        Ok(CliRun { lines, code })
    }

    /// The listed profiles, then `names` checked against them (empty = all).
    pub fn profile_names(
        &self,
        host: &dyn Host,
        names: &[String],
    ) -> Result<Vec<String>, IpcError> {
        // Shape first: an unsafe name is refused before any process starts.
        if let Some(bad) = names.iter().find(|n| !cli::profile_name_ok(n)) {
            return Err(err(
                "bad_profile",
                format!(
                    "{:?} is not a profile name",
                    bad.chars().take(64).collect::<String>()
                ),
            ));
        }
        let listed = self.collect(host, &CliAction::ProfilesList)?;
        if listed.code != Some(0) {
            return Err(err("cli", listed.lines.join("\n")));
        }
        cli::check_profile_names(names, &cli::listed_profiles(&listed.lines))
    }
}

#[tauri::command(async)]
pub fn health_get(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    cli_state: State<'_, CliState>,
    refresh: bool,
) -> HealthView {
    let (mut view, fresh) = cli_state.health(desktop.platform.as_ref(), refresh);
    with_overlay(&mut view, desktop.overlay.failure());
    if fresh {
        let _ = app.emit(EVENT_HEALTH_CHANGED, view.clone());
    }
    view
}

/// The tmux overlay's last failure as one health line (V6): a warning, never a failure —
/// tmux simply looks as the user has it.
pub fn with_overlay(view: &mut HealthView, failure: Option<String>) {
    if let Some(why) = failure {
        view.checks.push(crate::cli::HealthCheck {
            id: "tmux_overlay".into(),
            status: "warn",
            line: crate::tmux_overlay::not_applied(&why),
            fix: None,
        });
    }
}

#[tauri::command(async)]
pub fn release_check(
    desktop: State<'_, Desktop>,
    cli_state: State<'_, CliState>,
) -> Result<ReleaseCheck, IpcError> {
    let run = cli_state.collect(desktop.platform.as_ref(), &CliAction::UpdateCheck)?;
    Ok(cli::release_check_from(run))
}

/// `update` | `rollback`: the CLI's lines on `output` as they come, then its exit code;
/// health is checked again afterwards (`health_changed`).
#[tauri::command(async)]
pub fn release_apply(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    cli_state: State<'_, CliState>,
    kind: String,
    output: Channel<String>,
) -> Result<Option<i32>, IpcError> {
    let action = match kind.as_str() {
        "update" => CliAction::Update,
        "rollback" => CliAction::Rollback,
        _ => return Err(err("bad_kind", "update or rollback")),
    };
    let code = {
        let _busy = cli_state.busy.begin()?;
        desktop.platform.run_cli(&action, &mut |l| {
            let _ = output.send(l);
        })?
    };
    let (view, _) = cli_state.health(desktop.platform.as_ref(), true);
    let _ = app.emit(EVENT_HEALTH_CHANGED, view);
    Ok(code)
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfilesRun {
    /// The profiles the run covered.
    pub profiles: Vec<String>,
    pub lines: Vec<String>,
    pub code: Option<i32>,
}

fn profiles(
    desktop: &Desktop,
    cli_state: &CliState,
    names: &[String],
    dry_run: bool,
) -> Result<ProfilesRun, IpcError> {
    let host = desktop.platform.as_ref();
    let names = cli_state.profile_names(host, names)?;
    if names.is_empty() {
        return Err(err("bad_profile", "no Claude profiles were found"));
    }
    let run = cli_state.collect(
        host,
        &CliAction::ProfilesApply {
            dry_run,
            names: names.clone(),
        },
    )?;
    Ok(ProfilesRun {
        profiles: names,
        lines: run.lines,
        code: run.code,
    })
}

/// What `profiles apply` would change, per profile (`--dry-run`).
#[tauri::command(async)]
pub fn profiles_plan(
    desktop: State<'_, Desktop>,
    cli_state: State<'_, CliState>,
    names: Vec<String>,
) -> Result<ProfilesRun, IpcError> {
    profiles(&desktop, &cli_state, &names, true)
}

#[tauri::command(async)]
pub fn profiles_apply(
    desktop: State<'_, Desktop>,
    cli_state: State<'_, CliState>,
    names: Vec<String>,
) -> Result<ProfilesRun, IpcError> {
    profiles(&desktop, &cli_state, &names, false)
}

// LocalHost is the Linux / macOS host; on Windows these run against WslHost in E-45.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::cli::Runner;
    use crate::local_host::LocalHost;
    use std::path::Path;
    use std::sync::{Arc, Mutex as StdMutex};

    #[derive(Clone, Default)]
    struct Recorder(Arc<StdMutex<Vec<Vec<String>>>>);

    impl Runner for Recorder {
        fn run(
            &self,
            _p: &Path,
            args: &[String],
            line: &mut dyn FnMut(String),
        ) -> std::io::Result<Option<i32>> {
            self.0.lock().unwrap().push(args.to_vec());
            if args == ["profiles", "list"] {
                line("PROFILE  SETTINGS CONFIG DIR".into());
                line("default  present  /home/me/.claude".into());
                line("work     present  /home/me/.claude-work".into());
            }
            Ok(Some(
                if args.first().map(String::as_str) == Some("rollback") {
                    12
                } else {
                    0
                },
            ))
        }
    }

    fn host(rec: &Recorder, cli: bool) -> LocalHost {
        LocalHost::with_runner(
            Some("/home/me".into()),
            cli.then(|| "/usr/bin/rein-app".into()),
            Box::new(rec.clone()),
        )
    }

    #[test]
    fn exit_codes_come_back_unchanged() {
        let rec = Recorder::default();
        let h = host(&rec, true);
        let st = CliState::default();
        assert_eq!(st.collect(&h, &CliAction::Rollback).unwrap().code, Some(12));
    }

    #[test]
    fn a_bad_profile_name_starts_no_process_and_an_unlisted_one_is_refused() {
        let rec = Recorder::default();
        let h = host(&rec, true);
        let st = CliState::default();
        for bad in ["--enable", "x y", "hooks;"] {
            assert_eq!(
                st.profile_names(&h, &[bad.into()]).unwrap_err().kind,
                "bad_profile"
            );
        }
        assert!(
            rec.0.lock().unwrap().is_empty(),
            "no process for a malformed name"
        );
        assert_eq!(
            st.profile_names(&h, &["personal".into()]).unwrap_err().kind,
            "bad_profile"
        );
        assert_eq!(st.profile_names(&h, &[]).unwrap(), ["default", "work"]);
        assert!(
            rec.0
                .lock()
                .unwrap()
                .iter()
                .all(|a| a == &["profiles", "list"])
        );
    }

    #[test]
    fn health_is_not_checked_while_a_run_swaps_the_tree() {
        let rec = Recorder::default();
        let h = host(&rec, true);
        let st = CliState::default();
        let (first, _) = st.health(&h, true);
        let _run = st.busy.begin().unwrap();
        let (during, fresh) = st.health(&h, true);
        assert!(!fresh, "no doctor run during an update");
        assert_eq!(during, first, "the last view stands");
    }

    #[test]
    fn a_second_run_while_one_is_in_progress_is_refused() {
        let st = CliState::default();
        let _first = st.busy.begin().unwrap();
        let rec = Recorder::default();
        assert_eq!(
            st.collect(&host(&rec, true), &CliAction::UpdateCheck)
                .unwrap_err()
                .kind,
            "busy"
        );
    }

    #[test]
    fn no_cli_is_reported_and_health_still_works() {
        let rec = Recorder::default();
        let h = host(&rec, false);
        let st = CliState::default();
        assert_eq!(
            st.collect(&h, &CliAction::UpdateCheck).unwrap_err().kind,
            "cli_missing"
        );
        let (v, fresh) = st.health(&h, true);
        assert!(fresh);
        assert_eq!(
            v.status, "unknown",
            "no rein under /home/me: unknown, not a failure to run"
        );
    }
}
