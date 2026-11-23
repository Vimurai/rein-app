//! The Desk's commands (desk.md §API, D-074; E-106): `desk_all()` — every open project's Desk
//! in one reply; `panel_data(tab, desk)` — one project's (`panel_ipc`); `desk_act(key,
//! action)` — a row's action; `desk_seen(tab | all)` — the changes marked read. The web side
//! names only a row key the core last sent and one action that row offered; the core checks
//! both and either does the action itself (Done, Dismiss, Later) or answers which of the
//! window's existing actions to run, with the tab and the reader's reference from its own
//! record — never from the request.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::agents::AgentState;
use crate::attention::{AttentionState, Health};
use crate::desk::{self, Action, Desk, Ref, Row, RowKind, TabFacts};
use crate::dismissed::Dismissal;
use crate::ipc::{Desktop, IpcError, err};
use crate::panel_ipc;
use crate::panel_sources::Tasks;
use crate::ui_store::UiStore;
use crate::updates::Watch;

/// The fixed sentences of a refused act.
pub const GONE: &str = "that row is no longer there";
pub const NOT_OFFERED: &str = "that row does not offer this action";

/// A row as the core sent it: what an act may do with it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sent {
    tab: Option<u32>,
    project: Option<String>,
    kind: RowKind,
    primary: Action,
    more: Vec<Action>,
    dismissal: Option<(String, String)>,
    reference: Option<Ref>,
    /// `app` or `rein` for an update row.
    which: Option<&'static str>,
}

/// The rows last sent, by key.
#[derive(Default)]
pub struct DeskState(Mutex<HashMap<String, Sent>>);

impl DeskState {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Sent>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Remember `rows`; `tab`: replace only that tab's rows (none: replace all).
    fn remember(&self, rows: &[(Row, Option<String>)], tab: Option<u32>) {
        let mut m = self.lock();
        match tab {
            Some(t) => m.retain(|_, s| s.tab != Some(t)),
            None => m.clear(),
        }
        for (r, project) in rows {
            let which = match (r.kind, r.key.as_str()) {
                (RowKind::Update, "app/update/app") => Some("app"),
                (RowKind::Update, "app/update/rein") => Some("rein"),
                _ => None,
            };
            m.insert(
                r.key.clone(),
                Sent {
                    tab: r.tab,
                    project: project.clone(),
                    kind: r.kind,
                    primary: r.primary,
                    more: r.more.clone(),
                    dismissal: r.dismissal.clone(),
                    reference: r.reference.clone(),
                    which,
                },
            );
        }
    }
}

/// What a Desk is computed from.
pub struct Ctx<'a> {
    pub d: &'a Desktop,
    pub a: &'a AttentionState,
    pub ui: &'a UiStore,
    pub watch: &'a Watch,
    pub stuck_after_s: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TabDesk {
    pub tab: u32,
    pub desk: Desk,
}

/// `desk_all()`'s answer: every row in inbox order (the app's and every tab's), and each
/// open tab's Desk.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeskAll {
    pub for_you: Vec<Row>,
    pub tabs: Vec<TabDesk>,
}

/// `desk_act`'s answer: done here, or the window's own action to run.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    Done,
    Run {
        action: Action,
        tab: Option<u32>,
        #[serde(rename = "ref")]
        reference: Option<Ref>,
    },
}

fn name_of(project: &std::path::Path) -> String {
    let n = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    crate::panel_sources::shown(&n, desk::NAME - 1, false)
}

/// One open tab's Desk and its project path (none: the tab is not open).
fn tab_desk(c: &Ctx, tab: u32, now: u64) -> Option<(Desk, String)> {
    let (sent, _) = c.a.reported.last_input();
    let tin = sent.iter().find(|t| t.tab == Some(tab));
    let refused = c.a.reported.refused(tab) || tin.is_some_and(|t| t.refused);
    let flagged_setup = c.a.reported.setup(tab) || tin.is_some_and(|t| t.setup);
    if let Some((pty, _, project)) = c.d.tab_target(tab) {
        let path = project.to_string_lossy().into_owned();
        let tasks = panel_ipc::tasks(c.d, tab);
        let stamps = panel_ipc::stamps(c.d, tab);
        let handoffs = panel_ipc::handoffs(c.d, tab).ok();
        let seen = c.d.agents.seen(tab);
        let alive = c.d.host.alive(pty);
        let f = TabFacts {
            tab,
            name: name_of(&project),
            tasks: tasks.as_deref().map_err(|e| e.sentence()),
            stamps: &stamps,
            handoffs: handoffs.as_deref(),
            setup: refused || flagged_setup,
            ended: !alive,
            outdated: tin.is_some_and(|t| t.update),
            bell: c.a.reported.bells(tab),
            quiet: seen.activity.quiet_s.zip(seen.activity.last_output),
            agents_running: seen.agents.iter().any(|x| x.state == AgentState::Running),
            stuck_after_s: c.stuck_after_s,
            seen: c.ui.desk_seen(&path),
            lanes: if alive {
                crate::status::card_agents(&seen.agents)
            } else {
                vec![]
            },
        };
        let dismissed = |k: &str, h: &str| c.a.dismissed.has(&path, k, h);
        return Some((desk::desk(&f, now, &dismissed), path));
    }
    // A set-up tab has no process and no files read: only its set-up row.
    let project =
        c.d.projects
            .setup
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&tab)
            .map(|s| s.project.clone())?;
    let f = TabFacts {
        tab,
        name: name_of(&project),
        tasks: Err::<&Tasks, _>(crate::project_file::Unavailable::Unreadable.sentence()),
        stamps: &[],
        handoffs: None,
        setup: true,
        ended: false,
        outdated: false,
        bell: 0,
        quiet: None,
        agents_running: false,
        stuck_after_s: c.stuck_after_s,
        seen: None,
        lanes: vec![],
    };
    Some((
        desk::desk(&f, now, &|_, _| false),
        project.to_string_lossy().into_owned(),
    ))
}

/// Every open tab: those with a process, then the set-up tabs.
fn open_tabs(d: &Desktop) -> Vec<u32> {
    let mut v: Vec<u32> = d.running_tabs().into_iter().map(|t| t.0).collect();
    v.extend(d.setup_tabs());
    v
}

/// Debug builds only, with `REIN_DESKTOP_TRACE_PANELS` set: one `key=value` stderr line per
/// Desk answer — counts and time, never project text (obs_baseline).
fn trace(op: &str, tabs: usize, rows: usize, started: std::time::Instant) {
    #[cfg(debug_assertions)]
    if crate::dirs::process_env("REIN_DESKTOP_TRACE_PANELS").is_some() {
        eprintln!(
            "desk-trace op={op} tabs={tabs} rows={rows} elapsed_ms={}",
            started.elapsed().as_millis()
        );
    }
    #[cfg(not(debug_assertions))]
    let _ = (op, tabs, rows, started);
}

/// `desk_all()` at `now`.
pub fn all(c: &Ctx, state: &DeskState, now: u64) -> DeskAll {
    let started = std::time::Instant::now();
    let (_, health) = c.a.reported.last_input();
    let (app, rein) = crate::updates::offered(&c.watch.state());
    let mut sent: Vec<(Row, Option<String>)> =
        desk::app_rows(app.as_deref(), rein.as_deref(), health == Health::Fail)
            .into_iter()
            .map(|r| (r, None))
            .collect();
    let mut tabs = Vec::new();
    for tab in open_tabs(c.d) {
        let Some((d, path)) = tab_desk(c, tab, now) else {
            continue;
        };
        sent.extend(d.for_you.iter().map(|r| (r.clone(), Some(path.clone()))));
        tabs.push(TabDesk { tab, desk: d });
    }
    let mut for_you: Vec<Row> = sent.iter().map(|(r, _)| r.clone()).collect();
    desk::sort_rows(&mut for_you);
    state.remember(&sent, None);
    trace("all", tabs.len(), for_you.len(), started);
    DeskAll { for_you, tabs }
}

/// `panel_data(tab, desk)` at `now`: one tab's Desk; its rows replace that tab's sent rows.
pub fn one(c: &Ctx, state: &DeskState, tab: u32, now: u64) -> Option<Desk> {
    let started = std::time::Instant::now();
    let (d, path) = tab_desk(c, tab, now)?;
    let sent: Vec<(Row, Option<String>)> = d
        .for_you
        .iter()
        .map(|r| (r.clone(), Some(path.clone())))
        .collect();
    state.remember(&sent, Some(tab));
    trace("one", 1, d.for_you.len(), started);
    Some(d)
}

/// `desk_act(key, action)`: only a key the core last sent, only an action that row offered.
/// Done and Dismiss write the dismissed store; Later records the version; the rest are
/// answered for the window to run.
pub fn act(
    state: &DeskState,
    dismissed: &crate::dismissed::Dismissed,
    later: &dyn Fn(&str) -> Result<(), IpcError>,
    key: &str,
    action: Action,
    now: u64,
) -> Result<Act, IpcError> {
    let row = state
        .lock()
        .get(key)
        .cloned()
        .ok_or_else(|| err("not_in_list", GONE))?;
    if action != row.primary && !row.more.contains(&action) {
        return Err(err("not_offered", NOT_OFFERED));
    }
    match action {
        Action::Done | Action::Dismiss => {
            let (Some(project), Some((k, hash))) = (row.project, row.dismissal) else {
                return Err(err("not_offered", NOT_OFFERED));
            };
            dismissed
                .add(
                    Dismissal {
                        project,
                        key: k,
                        text_hash: hash,
                        at: 0,
                    },
                    now,
                )
                .map_err(|_| err("io", "the row could not be saved as done"))?;
            state.lock().remove(key);
            Ok(Act::Done)
        }
        Action::Later => {
            later(row.which.ok_or_else(|| err("not_offered", NOT_OFFERED))?)?;
            state.lock().remove(key);
            Ok(Act::Done)
        }
        _ => Ok(Act::Run {
            action,
            tab: row.tab,
            reference: row.reference,
        }),
    }
}

/// `desk_seen(tab | all)`: the projects whose changes are now read.
pub fn seen_projects(d: &Desktop, tab: Option<u32>) -> Vec<String> {
    let tabs = match tab {
        Some(t) => vec![t],
        None => d.running_tabs().into_iter().map(|t| t.0).collect(),
    };
    tabs.into_iter()
        .filter_map(|t| d.tab_project(t))
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

fn now_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[tauri::command(async)]
pub fn desk_all(
    state: State<'_, Desktop>,
    attention: State<'_, AttentionState>,
    desk: State<'_, DeskState>,
    config: State<'_, std::sync::Arc<crate::config::ConfigService>>,
    ui: State<'_, UiStore>,
    watch: State<'_, Watch>,
) -> DeskAll {
    let c = Ctx {
        d: &state,
        a: &attention,
        ui: &ui,
        watch: &watch,
        stuck_after_s: crate::status_watch::notify_settings(&config, &ui).1,
    };
    all(&c, &desk, now_s())
}

#[tauri::command(async)]
pub fn desk_act(
    app: AppHandle,
    attention: State<'_, AttentionState>,
    desk: State<'_, DeskState>,
    ui: State<'_, UiStore>,
    watch: State<'_, Watch>,
    key: String,
    action: Action,
) -> Result<Act, IpcError> {
    let later = |which: &str| crate::updates::later(&app, &watch, &ui, which).map(|_| ());
    act(&desk, &attention.dismissed, &later, &key, action, now_s())
}

#[tauri::command(async)]
pub fn desk_seen(
    state: State<'_, Desktop>,
    ui: State<'_, UiStore>,
    tab: Option<u32>,
) -> Result<(), IpcError> {
    let projects = seen_projects(&state, tab);
    if tab.is_some() && projects.is_empty() {
        return Err(err("not_in_list", "that tab is not open"));
    }
    ui.set_desk_seen(&projects, now_s())
}

#[cfg(test)]
#[path = "desk_ipc_tests.rs"]
mod tests;
