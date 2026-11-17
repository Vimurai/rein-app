//! Open tabs remembered and restored (desktop.md §Look and feel v2 V12 components 4–6,
//! §Data model, §Interface contracts; D-061; E-70). The app's state file keeps the open
//! project tabs in tab order and which one is current (`projects.rs`). At start the web side
//! asks for the record and opens each entry with `tab_open(…, restore)`: a live session is
//! attached to; a project whose session is gone opens as a **card** with no process
//! ("<project> is not running."), and a folder that is gone as a "missing" card. The card's
//! Start runs `rein start` in place of the card (`tab_start`). The web side sends the record
//! as the core ids of its tabs — never a path: the core names each tab's project itself.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, State};

use crate::ipc::{Desktop, IpcError, Tab, channel_events, err, validate_size};
use crate::projects::{OpenTab, ProjectStore};
use crate::tmux_server::Server;

/// A restored tab with no process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub project: PathBuf,
    pub server: Server,
    /// The folder is gone: only Close project is offered.
    pub missing: bool,
}

/// A card as the web side sees it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CardView {
    pub id: u32,
    pub project: String,
    pub title: String,
    pub missing: bool,
}

/// The project a tab of this window stands for: a started tab, a set-up tab or a card.
pub fn project_of(d: &Desktop, tab: u32) -> Option<PathBuf> {
    d.tab_project(tab)
        .or_else(|| d.projects.setup_tab(tab).map(|s| s.project))
        .or_else(|| {
            d.cards
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&tab)
                .map(|c| c.project.clone())
        })
}

/// The record for the tabs `ids` (tab order) with `active` the current one's place. An id
/// the core does not know is left out (and `active` follows the entries kept).
pub fn record_of(d: &Desktop, ids: &[u32], active: Option<usize>) -> (Vec<OpenTab>, Option<usize>) {
    let distro = d.platform.distro();
    let mut open = Vec::new();
    let mut at = None;
    for (i, id) in ids.iter().enumerate() {
        if let Some(p) = project_of(d, *id) {
            if active == Some(i) {
                at = Some(open.len());
            }
            open.push(OpenTab {
                path: p.to_string_lossy().into_owned(),
                distro: distro.clone(),
            });
        }
    }
    (open, at)
}

/// The card's Start (V12 component 5): `rein start` in place of the card, under its id.
pub fn start(
    d: &Desktop,
    tab: u32,
    cols: u32,
    rows: u32,
    events: Arc<dyn crate::pty::Events>,
) -> Result<Tab, IpcError> {
    let size = validate_size(cols, rows)?;
    let card = d
        .cards
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&tab)
        .cloned()
        .ok_or_else(|| err("unknown_tab", "the tab is not waiting to start"))?;
    if card.missing {
        return Err(err("missing", "the project's folder is gone"));
    }
    let spec = d
        .start_spec(&card.project, card.server, false, false)?
        .ok_or_else(|| err("spawn", "nothing to start"))?;
    // Taken out of the cards under the tabs lock in `open_spec_as`.
    d.open_spec_as(&spec, &card.project, size, events, card.server, Some(tab))
}

/// The record as `tabs_restore` gives it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Restore {
    pub tabs: Vec<String>,
    pub active: Option<usize>,
    /// The page was loaded again while tabs were held (V12 10): the command-line project was
    /// handled at the first load and is not opened (or focused) again.
    pub reloaded: bool,
}

/// `tabs_restore()`: the recorded tabs of this machine (on Windows: of this window's distro),
/// in order, and the current one. The web side opens each with `tab_open(…, restore)`, whose
/// validation holds a recorded path to the rule a picked one meets.
#[tauri::command(async)]
pub fn tabs_restore(state: State<'_, Desktop>, projects: State<'_, Arc<ProjectStore>>) -> Restore {
    // The page asks once per load: tabs still held mean it was reloaded (V12 10, E-89).
    let held = state.reset_page();
    if cfg!(debug_assertions) && held > 0 {
        eprintln!("page-reset held={held}"); // read by tests/e2e/reload-screens.sh
    }
    let (open, active) = projects.open_tabs();
    let distro = state.platform.distro();
    let mut tabs = Vec::new();
    let mut at = None;
    for (i, t) in open.into_iter().enumerate() {
        // A tab recorded for another distro is not this window's (Windows only).
        if t.distro.is_some() && distro.is_some() && t.distro != distro {
            continue;
        }
        if active == Some(i) {
            at = Some(tabs.len());
        }
        tabs.push(t.path);
    }
    Restore {
        tabs,
        active: at,
        reloaded: held > 0,
    }
}

/// `tabs_record(tabs, active)`: the web side's tabs as core ids, in tab order.
#[tauri::command(async)]
pub fn tabs_record(
    state: State<'_, Desktop>,
    projects: State<'_, Arc<ProjectStore>>,
    tabs: Vec<u32>,
    active: Option<usize>,
) -> Result<(), IpcError> {
    let ids: Vec<u32> = tabs
        .into_iter()
        .take(crate::projects::MAX_OPEN * 2)
        .collect();
    let (open, active) = record_of(&state, &ids, active);
    projects
        .set_open(open, active)
        .map_err(|_| err("io", "the open tabs could not be saved"))
}

/// `tab_start(tab, cols, rows, output)`: the card's Start.
#[tauri::command(async)]
pub fn tab_start(
    app: AppHandle,
    state: State<'_, Desktop>,
    projects: State<'_, Arc<ProjectStore>>,
    tab: u32,
    cols: u32,
    rows: u32,
    output: Channel<InvokeResponseBody>,
) -> Result<Tab, IpcError> {
    let opened = start(&state, tab, cols, rows, channel_events(output, app))?;
    let _ = projects.touch(&opened.project, state.platform.distro());
    Ok(opened)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::lifecycle::tests::{Quiet, Rec};
    use crate::project_setup::Opened;
    use std::sync::Mutex;

    fn desk(live: &[&str]) -> (Desktop, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let tags = live
            .iter()
            .map(|p| crate::session_tag::tag(None, std::path::Path::new(p)))
            .collect();
        (Desktop::new(Box::new(Rec(Arc::clone(&log), tags))), log)
    }
    fn restore(d: &Desktop, p: &str, server: Server) -> Opened {
        d.open_as(p, 80, 24, Arc::new(Quiet), server, true).unwrap()
    }

    #[test]
    fn a_restored_tab_attaches_or_waits_as_a_card_and_never_starts_by_itself() {
        let (d, log) = desk(&["/p/live"]);
        let take = || std::mem::take(&mut *log.lock().unwrap());
        // Live: attached, no rein start.
        assert!(matches!(
            restore(&d, "/p/live", Server::App),
            Opened::Tab(_)
        ));
        assert_eq!(take(), ["setup live", "attach /p/live"]);
        // Not live: a card, no process.
        let Opened::Card { card } = restore(&d, "/p/dead", Server::App) else {
            panic!("a card")
        };
        assert_eq!(
            (card.project.as_str(), card.title.as_str(), card.missing),
            ("/p/dead", "dead", false)
        );
        assert_eq!(take(), ["setup none"], "looked up, nothing started");
        // The folder is gone: a missing card, not even a lookup.
        let Opened::Card { card: gone } = restore(&d, "/gone/x", Server::App) else {
            panic!("a missing card")
        };
        assert!(gone.missing);
        assert_eq!(take(), Vec::<String>::new());
        // Without restore a gone folder is the error it always was.
        assert!(
            d.open_as("/gone/x", 80, 24, Arc::new(Quiet), Server::App, false)
                .is_err()
        );
        // The user's server: opened as today (rein start), never a card.
        assert!(matches!(restore(&d, "/p/u", Server::User), Opened::Tab(_)));
        assert_eq!(take(), ["start /p/u user"]);
        // Start: rein start in place of the card, under its id; a missing card cannot start.
        let started = start(&d, card.id, 80, 24, Arc::new(Quiet)).unwrap();
        assert_eq!(started.id, card.id);
        assert_eq!(take(), ["setup none", "start /p/dead app"]);
        assert!(
            start(&d, card.id, 80, 24, Arc::new(Quiet)).is_err(),
            "no longer a card"
        );
        assert_eq!(
            start(&d, gone.id, 80, 24, Arc::new(Quiet))
                .unwrap_err()
                .kind,
            "missing"
        );
        d.detach(gone.id);
        assert!(
            project_of(&d, gone.id).is_none(),
            "closing a card forgets it"
        );
        d.detach_all();
    }

    /// V12 10 (E-89): a page load while tabs are held ends their ptys and forgets set-up
    /// tabs and cards, so the restore that follows attaches each session once.
    #[test]
    fn a_reload_detaches_every_held_tab_and_kills_nothing() {
        let (d, log) = desk(&["/p/live"]);
        let take = || std::mem::take(&mut *log.lock().unwrap());
        assert!(matches!(
            restore(&d, "/p/live", Server::App),
            Opened::Tab(_)
        ));
        let Opened::Card { card } = restore(&d, "/p/dead", Server::App) else {
            panic!()
        };
        take();
        assert_eq!(d.reset_page(), 2);
        assert!(project_of(&d, card.id).is_none());
        assert_eq!(
            take(),
            Vec::<String>::new(),
            "nothing killed, nothing started"
        );
        assert_eq!(d.reset_page(), 0, "a first load holds nothing");
        // The same project attaches again after the reload.
        assert!(matches!(
            restore(&d, "/p/live", Server::App),
            Opened::Tab(_)
        ));
        assert_eq!(take(), ["setup live", "attach /p/live"]);
        d.detach_all();
    }

    #[test]
    fn the_record_names_each_tab_by_its_own_project_and_cards_count_against_the_limit() {
        let (d, _) = desk(&[]);
        let a = match restore(&d, "/p/a", Server::User) {
            Opened::Tab(t) => t.id,
            o => panic!("{o:?}"),
        };
        let Opened::Card { card } = restore(&d, "/p/b", Server::App) else {
            panic!()
        };
        let (open, active) = record_of(&d, &[card.id, 999, a], Some(2));
        let paths: Vec<&str> = open.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(
            paths,
            ["/p/b", "/p/a"],
            "tab order kept; an unknown id left out"
        );
        assert_eq!(active, Some(1), "active follows its tab");
        assert_eq!(record_of(&d, &[999], Some(0)), (vec![], None));
        // 16 tabs at most, cards included.
        for i in 0..14 {
            restore(&d, &format!("/p/c{i}"), Server::App);
        }
        assert_eq!(
            d.open_as("/p/x", 80, 24, Arc::new(Quiet), Server::App, true)
                .unwrap_err()
                .kind,
            "too_many_tabs"
        );
        d.detach_all();
    }
}
