//! `panel_data` (desktop panels.md §API, D-056; E-59): the one IPC command behind the panels.
//! The web side names a tab of this window and a source from the closed list; the project is
//! the tab's own, taken from app state — no path ever comes from the web side. A file is
//! stat-ed first and read and parsed again only when its stamp changed; a tab's entries go
//! when the tab closes. The answer is `{ok, read_at}` or `{unavailable: <fixed sentence>}`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::ipc::Desktop;
use crate::panel_attention::{self, Attention};
use crate::panel_sources::{self, Handoffs, Summary, Tasks};
use crate::project_file::{FileStamp, ProjectFile, Unavailable};
use crate::stamps::{self, Stamp};

/// The sources the web side may ask for (the closed list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Tasks,
    Handoffs,
    Summary,
    /// The "Needs you" facts of one tab (E-61): asked for every 5 s for every open tab.
    Attention,
    /// One stamp by index, for the reader (desk.md §API, E-106).
    Stamps,
    /// The tab's Desk (desk.md §Components 1, E-106): answered by `panel_data` itself, which
    /// holds the Desk's state.
    Desk,
}

/// The largest answer sent (security review of E-59, P2-4); a larger one drops its oldest
/// rows until it fits and says it was cut.
pub const MAX_REPLY: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Data {
    Tasks(Tasks),
    Handoffs(Handoffs),
    Summary(Box<Summary>),
    Attention(Attention),
    Stamp(Box<Stamp>),
    Desk(Box<crate::desk::Desk>),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Reply {
    Ok { ok: Box<Data>, read_at: u64 },
    Unavailable { unavailable: &'static str },
}

#[derive(Clone)]
enum Parsed {
    /// `state.json`: its tasks and, read from the same bytes, its stamps (E-106).
    State(Result<Arc<Tasks>, Unavailable>, Arc<Vec<Stamp>>),
    Handoffs(Arc<Handoffs>),
}

/// Parsed files per (tab, file), with the stamp they were read at.
#[derive(Default)]
pub struct PanelCache(Mutex<HashMap<(u32, ProjectFile), (FileStamp, Parsed)>>);

impl PanelCache {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<(u32, ProjectFile), (FileStamp, Parsed)>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The tab closed: nothing of it is kept.
    pub fn forget(&self, tab: u32) {
        self.lock().retain(|(t, _), _| *t != tab);
    }
}

/// Debug builds only: with `REIN_DESKTOP_TRACE_PANELS` set, one stderr line per file looked
/// at — `stat` (unchanged, from the cache) or `read` (read and parsed) — so a window run can
/// see that an open panel follows a change and that a hidden window reads nothing (E-60).
/// Release builds ignore it, as they do `REIN_DESKTOP_SLOW_LIST_MS`. No path or file text.
fn trace(tab: u32, which: ProjectFile, what: &str) {
    #[cfg(debug_assertions)]
    if crate::dirs::process_env("REIN_DESKTOP_TRACE_PANELS").is_some() {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        eprintln!("panel-trace {ms} tab={tab} file={which:?} {what}");
    }
    #[cfg(not(debug_assertions))]
    let _ = (tab, which, what);
}

/// One file of a tab, parsed — from the cache when its stamp is unchanged.
fn parsed(d: &Desktop, tab: u32, which: ProjectFile) -> Result<Parsed, Unavailable> {
    let project = d.tab_project(tab).ok_or(Unavailable::Unreadable)?;
    let stat = d.platform.read_project_file(&project, which, false)?;
    if let Some((stamp, p)) = d.panels.lock().get(&(tab, which))
        && *stamp == stat.stamp
    {
        trace(tab, which, "stat");
        return Ok(p.clone());
    }
    let file = d.platform.read_project_file(&project, which, true)?;
    trace(tab, which, "read");
    let p = match which {
        ProjectFile::StateJson => Parsed::State(
            panel_sources::parse_tasks(&file.bytes).map(Arc::new),
            Arc::new(stamps::parse_stamps(&file.bytes)),
        ),
        ProjectFile::CommMd => Parsed::Handoffs(Arc::new(panel_sources::parse_handoffs(
            &file.bytes,
            file.from_start,
        ))),
    };
    // Only a tab that is still open is remembered.
    if d.tab_project(tab).is_some() {
        d.panels
            .lock()
            .insert((tab, which), (file.stamp, p.clone()));
    }
    Ok(p)
}

pub fn tasks(d: &Desktop, tab: u32) -> Result<Arc<Tasks>, Unavailable> {
    match parsed(d, tab, ProjectFile::StateJson)? {
        Parsed::State(t, _) => t,
        Parsed::Handoffs(_) => Err(Unavailable::Unreadable),
    }
}

/// The tab's stamps (none when `state.json` cannot be read).
pub fn stamps(d: &Desktop, tab: u32) -> Arc<Vec<Stamp>> {
    match parsed(d, tab, ProjectFile::StateJson) {
        Ok(Parsed::State(_, s)) => s,
        _ => Arc::new(Vec::new()),
    }
}

/// A mark that changes when either file of the tab changed (the stamps the cache last saw):
/// the status event carries it so the Desk asks again for that tab only (E-106).
pub fn files_mark(d: &Desktop, tab: u32) -> u32 {
    use std::hash::{Hash, Hasher};
    let cache = d.panels.lock();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for which in [ProjectFile::StateJson, ProjectFile::CommMd] {
        match cache.get(&(tab, which)) {
            Some((s, _)) => (s.len, s.mtime_ns, s.id, s.ctime_ns).hash(&mut h),
            None => 0u8.hash(&mut h),
        }
    }
    // 32 bits: a JSON number the web side holds exactly.
    (h.finish() >> 32) as u32
}

pub fn handoffs(d: &Desktop, tab: u32) -> Result<Arc<Handoffs>, Unavailable> {
    match parsed(d, tab, ProjectFile::CommMd)? {
        Parsed::Handoffs(h) => Ok(h),
        Parsed::State(..) => Err(Unavailable::Unreadable),
    }
}

/// The answer for `source` of `tab` at `now` (seconds since the epoch). `index`: the stamp
/// the `stamps` source gives. `desk` answers nothing here (see [`panel_data`]).
pub fn panel_reply(d: &Desktop, tab: u32, source: Source, index: Option<u32>, now: u64) -> Reply {
    let data = match source {
        Source::Stamps => d
            .tab_project(tab)
            .ok_or(Unavailable::Unreadable)
            .and_then(|_| {
                let s = stamps(d, tab);
                index
                    .and_then(|i| s.get(i as usize).cloned())
                    .map(|x| Data::Stamp(Box::new(x)))
                    .ok_or(Unavailable::Unreadable)
            }),
        Source::Desk => Err(Unavailable::Unreadable),
        Source::Tasks => tasks(d, tab).map(|t| Data::Tasks((*t).clone())),
        Source::Handoffs => handoffs(d, tab).map(|h| Data::Handoffs((*h).clone())),
        // The summary needs the tasks; without COMM.md it has no last handoff.
        Source::Summary => tasks(d, tab).map(|t| {
            let h = handoffs(d, tab).unwrap_or_else(|_| {
                Arc::new(Handoffs {
                    handoffs: Vec::new(),
                    cut: false,
                })
            });
            Data::Summary(Box::new(panel_sources::summary(&t, &h)))
        }),
        // Each file on its own: a missing one gives no facts, the other still counts. Only a
        // tab that is gone is "not available".
        Source::Attention => d.tab_project(tab).ok_or(Unavailable::Unreadable).map(|_| {
            let t = tasks(d, tab).ok();
            let h = handoffs(d, tab).ok();
            Data::Attention(panel_attention::attention(t.as_deref(), h.as_deref()))
        }),
    };
    match data {
        Ok(data) => Reply::Ok {
            ok: Box::new(fit(data)),
            read_at: now,
        },
        Err(why) => Reply::Unavailable {
            unavailable: why.sentence(),
        },
    }
}

/// At most [`MAX_REPLY`] bytes once serialised: the oldest rows dropped first, marked as cut.
fn fit(mut data: Data) -> Data {
    let size = |d: &Data| serde_json::to_vec(d).map(|v| v.len()).unwrap_or(usize::MAX);
    while size(&data) > MAX_REPLY {
        match &mut data {
            Data::Tasks(t) if !t.tasks.is_empty() => {
                let drop = (t.tasks.len() / 10).max(1);
                t.tasks.drain(..drop);
                t.cut = true;
            }
            Data::Handoffs(h) if !h.handoffs.is_empty() => {
                h.handoffs.remove(0);
                h.cut = true;
            }
            _ => break,
        }
    }
    data
}

/// `panel_data(tab, source, index?)`: one tab's panel source. Off the UI thread.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub fn panel_data(
    state: State<'_, Desktop>,
    attention: State<'_, crate::attention::AttentionState>,
    desk: State<'_, crate::desk_ipc::DeskState>,
    config: State<'_, Arc<crate::config::ConfigService>>,
    ui: State<'_, crate::ui_store::UiStore>,
    watch: State<'_, crate::updates::Watch>,
    tab: u32,
    source: Source,
    index: Option<u32>,
) -> Reply {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if source == Source::Desk {
        let ctx = crate::desk_ipc::Ctx {
            d: &state,
            a: &attention,
            ui: &ui,
            watch: &watch,
            stuck_after_s: crate::status_watch::notify_settings(&config, &ui).1,
        };
        return match crate::desk_ipc::one(&ctx, &desk, tab, now) {
            Some(desk) => Reply::Ok {
                ok: Box::new(Data::Desk(Box::new(desk))),
                read_at: now,
            },
            None => Reply::Unavailable {
                unavailable: Unavailable::Unreadable.sentence(),
            },
        };
    }
    panel_reply(&state, tab, source, index, now)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::local_host::LocalHost;
    use crate::pty::{ChildSpec, Events, PtyId, Size};

    struct Quiet;
    impl Events for Quiet {
        fn output(&self, _: PtyId, _: Vec<u8>) {}
        fn exit(&self, _: PtyId, _: u32) {}
    }

    fn tab_on(d: &Desktop, project: &std::path::Path) -> u32 {
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 5".into()],
            cwd: Some(project.into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        d.open_spec(&spec, project, Size { cols: 80, rows: 24 }, Arc::new(Quiet))
            .unwrap()
            .id
    }

    #[test]
    fn a_tab_reads_its_own_project_and_updates_when_the_file_changes() {
        let p = tempfile::tempdir().unwrap();
        std::fs::create_dir(p.path().join(".ai")).unwrap();
        let state = p.path().join(".ai/state.json");
        std::fs::write(&state, br#"{"tasks":[{"id":"E-1","status":"OPEN"}]}"#).unwrap();
        let d = Desktop::new(Box::new(LocalHost::new(None)));
        let tab = tab_on(&d, p.path());
        let ids = |r: Reply| match r {
            Reply::Ok { ok, .. } => match *ok {
                Data::Tasks(t) => t.tasks.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        assert_eq!(ids(panel_reply(&d, tab, Source::Tasks, None, 1)), ["E-1"]);
        // A change (different size and mtime) is read again; an unchanged file is not.
        std::fs::write(
            &state,
            br#"{"tasks":[{"id":"E-1","status":"OPEN"},{"id":"E-2","status":"OPEN"}]}"#,
        )
        .unwrap();
        assert_eq!(
            ids(panel_reply(&d, tab, Source::Tasks, None, 2)),
            ["E-1", "E-2"]
        );
        // No COMM.md: handoffs are not available, the summary still is.
        assert_eq!(
            panel_reply(&d, tab, Source::Handoffs, None, 3),
            Reply::Unavailable {
                unavailable: Unavailable::Missing.sentence()
            }
        );
        assert!(matches!(
            panel_reply(&d, tab, Source::Summary, None, 3),
            Reply::Ok { ok, .. } if matches!(*ok, Data::Summary(_))
        ));
        // "Needs you" (E-61): no COMM.md is no handoff facts, not "not available".
        assert!(matches!(
            panel_reply(&d, tab, Source::Attention, None, 3),
            Reply::Ok { ok, .. } if matches!(&*ok, Data::Attention(a)
                if a.tasks_read && !a.in_progress && a.last_handoff.is_none())
        ));
        // A closed tab: nothing is read and nothing is kept.
        d.detach(tab);
        assert_eq!(
            panel_reply(&d, tab, Source::Tasks, None, 4),
            Reply::Unavailable {
                unavailable: Unavailable::Unreadable.sentence()
            }
        );
        assert_eq!(
            panel_reply(&d, tab, Source::Attention, None, 4),
            Reply::Unavailable {
                unavailable: Unavailable::Unreadable.sentence()
            }
        );
        assert!(d.panels.lock().is_empty());
    }

    #[test]
    fn the_answer_is_capped_and_its_shape_is_ok_or_unavailable() {
        // Bigger than any parse can give (the parser caps each field), so `fit` has to cut.
        let task = |i: usize| panel_sources::Task {
            id: format!("E-{i}"),
            status: "DONE".into(),
            owner: String::new(),
            tier: String::new(),
            description: "x".repeat(4096),
            ..Default::default()
        };
        let t = Tasks {
            tasks: (0..600).map(task).collect(),
            cut: false,
            focus: String::new(),
        };
        let fitted = fit(Data::Tasks(t));
        assert!(serde_json::to_vec(&fitted).unwrap().len() <= MAX_REPLY);
        assert!(matches!(fitted, Data::Tasks(ref t) if t.cut));
        let ok = serde_json::to_value(Reply::Ok {
            ok: Box::new(Data::Handoffs(Handoffs {
                handoffs: vec![],
                cut: false,
            })),
            read_at: 7,
        })
        .unwrap();
        assert_eq!(
            ok,
            serde_json::json!({"ok": {"handoffs": [], "cut": false}, "read_at": 7})
        );
        let no = serde_json::to_value(Reply::Unavailable {
            unavailable: Unavailable::TeamMode.sentence(),
        })
        .unwrap();
        assert_eq!(
            no,
            serde_json::json!({"unavailable": "this project keeps tasks in a form this version does not read"})
        );
        // The source is a closed list: anything else does not deserialise.
        assert!(serde_json::from_str::<Source>("\"files\"").is_err());
        assert_eq!(
            serde_json::from_str::<Source>("\"summary\"").unwrap(),
            Source::Summary
        );
        assert_eq!(
            serde_json::from_str::<Source>("\"attention\"").unwrap(),
            Source::Attention
        );
    }
}
