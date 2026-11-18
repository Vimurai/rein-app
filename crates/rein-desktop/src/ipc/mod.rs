//! The IPC command table for the terminal stage (desktop.md §API, E-35): `tab_open`,
//! `pty_write`, `pty_resize`, `pty_ack`, `tab_detach`; event `pty_exit`.
//!
//! The web view is untrusted (§Security 1): every input is validated here, in Rust, before it
//! reaches the `PtyHost`. No command takes a program name or an argument vector — the only
//! child is `rein start` (§Security 2).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State};

use crate::host::Host;
use crate::pty::{ChildSpec, Events, PtyError, PtyHost, PtyId, Size};
use crate::tmux_overlay::{self, Colours, OverlayHealth};

/// How long `tab_tmux` waits for a tab's tty (V6). On Windows the start script prints it after
/// the login shell's start-up files, which can take seconds; the wait ends early when the tab
/// closes or its terminal ends (E-56).
pub const TTY_WAIT: Duration = tmux_overlay::CLIENT_WAIT;

#[cfg(all(test, unix))]
mod commands {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ipc_commands.rs"));
}

pub const MAX_WRITE: usize = 64 * 1024;
pub const COLS: std::ops::RangeInclusive<u32> = 2..=500;
pub const ROWS: std::ops::RangeInclusive<u32> = 2..=200;
/// desktop.md §Execution Constraints.
pub const MAX_TABS: usize = 16;
pub const EVENT_PTY_EXIT: &str = "pty_exit";
/// The window label the capability file names.
pub const MAIN_WINDOW: &str = "main";
/// `clipboard_write` (desktop.md §API): the most text one copy may carry.
pub const MAX_CLIPBOARD: usize = 64 * 1024;

/// What a tab's open does once the app's server was asked (V12 components 2, 5, 9).
pub(crate) enum Start {
    /// Start (or attach with) this child.
    Spec(ChildSpec),
    /// A restore with no live session: the "not running" card.
    NotRunning,
    /// Two untagged sessions started in the folder: the card, Close project only.
    Two,
}

/// An IPC refusal: a stable `kind` for the UI and a plain-words `message`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct IpcError {
    pub kind: &'static str,
    pub message: String,
}

pub fn err(kind: &'static str, message: impl Into<String>) -> IpcError {
    IpcError {
        kind,
        message: message.into(),
    }
}

impl From<PtyError> for IpcError {
    fn from(e: PtyError) -> Self {
        let kind = match e {
            PtyError::UnknownPty => "unknown_pty",
            PtyError::AckTooLarge => "ack_too_large",
            PtyError::Spawn(_) => "spawn",
            PtyError::Io(_) => "io",
        };
        err(kind, e.to_string())
    }
}

pub fn validate_size(cols: u32, rows: u32) -> Result<Size, IpcError> {
    if !COLS.contains(&cols) || !ROWS.contains(&rows) {
        return Err(err(
            "out_of_range",
            format!("terminal size {cols}x{rows} is outside 2–500 columns by 2–200 rows"),
        ));
    }
    // Both ranges fit in u16.
    Ok(Size {
        cols: cols as u16,
        rows: rows as u16,
    })
}

pub fn validate_write(data: &str) -> Result<&[u8], IpcError> {
    if data.len() > MAX_WRITE {
        return Err(err("too_large", "input larger than 64 KiB"));
    }
    Ok(data.as_bytes())
}

/// A tab as the web side sees it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Tab {
    pub id: u32,
    pub project: String,
    pub pty: PtyId,
    pub title: String,
    /// The tmux server the tab's `rein start` reaches (V8), as it was when the tab opened;
    /// the overlay talks to that one. Sent read-only (V11: the close wording and the confirm
    /// depend on it); the web side never names a server in a request.
    pub server: crate::tmux_server::Server,
}

/// The window core's state for the terminal stage.
pub struct Desktop {
    /// Linux / macOS (`LocalHost`) or Windows (`WslHost`): how a project is validated and
    /// what the pty child is.
    pub platform: Box<dyn Host>,
    pub host: PtyHost,
    tabs: Mutex<HashMap<u32, Tab>>,
    next_tab: AtomicU32,
    /// The last tmux overlay failure, shown in the health view (V6).
    pub overlay: OverlayHealth,
    /// One overlay script at a time, and per tab only the newest request runs: an older one
    /// still waiting is dropped (security review of E-51, F4).
    overlay_run: Mutex<()>,
    overlay_seq: Mutex<HashMap<u32, u64>>,
    /// The panels' parsed sources per tab, kept while the file is unchanged (E-59).
    pub panels: crate::panel_ipc::PanelCache,
    /// The agent-exit bar's per-tab memory (V9 component 4, E-62).
    pub agents: crate::agents::AgentWatch,
    /// Set-up tabs and the out-of-date checks (V9 components 1–3, E-63).
    pub projects: crate::project_setup::ProjectState,
    /// Restored tabs with no process: "not running" or "missing" cards (V12 5, E-70).
    pub(crate) cards: Mutex<HashMap<u32, crate::restore::Card>>,
}

impl Desktop {
    pub fn new(platform: Box<dyn Host>) -> Self {
        Self {
            platform,
            host: PtyHost::default(),
            tabs: Mutex::new(HashMap::new()),
            next_tab: AtomicU32::new(0),
            overlay: OverlayHealth::default(),
            overlay_run: Mutex::new(()),
            overlay_seq: Mutex::new(HashMap::new()),
            panels: crate::panel_ipc::PanelCache::default(),
            agents: crate::agents::AgentWatch::default(),
            projects: crate::project_setup::ProjectState::default(),
            cards: Mutex::new(HashMap::new()),
        }
    }

    /// A tab's pty, tmux server and project, as recorded when it opened (V9: the agents'
    /// status and the close act on these, never on anything the web side sends).
    pub fn tab_target(
        &self,
        tab: u32,
    ) -> Option<(PtyId, crate::tmux_server::Server, std::path::PathBuf)> {
        self.tabs()
            .get(&tab)
            .map(|t| (t.pty, t.server, std::path::PathBuf::from(&t.project)))
    }

    /// The project tag of `project` (V12 component 1): with the distro on Windows.
    fn tag_of(&self, project: &Path) -> String {
        crate::session_tag::tag(self.platform.distro().as_deref(), project)
    }

    /// The project a tab of this window runs in, as recorded when it opened — the only path a
    /// panel source reads under (panels.md §API: the web side never sends a path).
    pub fn tab_project(&self, tab: u32) -> Option<std::path::PathBuf> {
        self.tabs()
            .get(&tab)
            .map(|t| std::path::PathBuf::from(&t.project))
    }

    /// Every tab with a process, in the order they opened: (core id, pty, server).
    pub fn running_tabs(&self) -> Vec<(u32, PtyId, crate::tmux_server::Server)> {
        let mut v: Vec<_> = self
            .tabs()
            .values()
            .map(|t| (t.id, t.pty, t.server))
            .collect();
        v.sort_by_key(|t| t.0);
        v
    }

    /// The set-up tabs (no process: the folder is not set up), by core id.
    pub fn setup_tabs(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .projects
            .setup
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .copied()
            .collect();
        v.sort_unstable();
        v
    }

    fn tabs(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Tab>> {
        self.tabs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Validate, then start `rein start` in `project` with output going to `events`. A
    /// directory without `.ai/` opens as a set-up tab instead, with no pty (V9 component 1).
    #[cfg(all(test, unix))]
    pub fn open(
        &self,
        project: &str,
        cols: u32,
        rows: u32,
        events: Arc<dyn Events>,
        server: crate::tmux_server::Server,
    ) -> Result<crate::project_setup::Opened, IpcError> {
        self.open_as(project, cols, rows, events, server, false)
    }

    /// [`open`](Self::open); with `restore` (a tab from the open-tabs record, V12 components
    /// 4–5) a folder that is gone, or a project whose session is not alive on the app's
    /// server, opens as a card with no process instead.
    pub fn open_as(
        &self,
        project: &str,
        cols: u32,
        rows: u32,
        events: Arc<dyn Events>,
        server: crate::tmux_server::Server,
        restore: bool,
    ) -> Result<crate::project_setup::Opened, IpcError> {
        let size = validate_size(cols, rows)?;
        let (project, found) = match self.platform.project_dir(project) {
            Err(e) if restore && e.kind == "not_a_directory" => {
                let gone =
                    crate::project_path::check(project).map_err(|why| err("unsafe_path", why))?;
                return self.open_card(gone, server, crate::restore::CardKind::Missing);
            }
            other => other?,
        };
        match found {
            crate::host::Found::Project => {}
            crate::host::Found::Plain => return self.open_setup(project, server, None),
            crate::host::Found::Inside(parent) => {
                return self.open_setup(project, server, Some(parent));
            }
        }
        let spec = match self.start_spec(&project, server, false, restore)? {
            Start::Spec(s) => s,
            Start::NotRunning => {
                return self.open_card(project, server, crate::restore::CardKind::NotRunning);
            }
            Start::Two => {
                return self.open_card(project, server, crate::restore::CardKind::TwoSessions);
            }
        };
        self.open_spec_as(&spec, &project, size, events, server, None)
            .map(crate::project_setup::Opened::Tab)
    }

    /// The child of a tab: `rein start`, or with `setup` `rein init` first (V9 component 2).
    /// V8 3a: the tmux home is made or checked before the pty opens; on failure the pty is not
    /// opened and the window shows the app's fixed sentence. V12 components 2–3: on the app's
    /// server the same setup process answers whether a session tagged for this project is
    /// alive — then the child attaches to it and `rein start` is never run on it.
    /// With `restore`, [`Start::NotRunning`] when no session is alive for the project:
    /// nothing starts. V12 9 (E-90): an untagged session started in the folder is adopted;
    /// two such sessions give [`Start::Two`] — `rein start` never runs while one is alive.
    pub(crate) fn start_spec(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
        setup: bool,
        restore: bool,
    ) -> Result<Start, IpcError> {
        use crate::tmux_server::Lookup;
        if server == crate::tmux_server::Server::App {
            let tag = self.tag_of(project);
            let rows = self
                .platform
                .tmux_setup(&tag)
                .map_err(|why| err("tmux_setup", crate::tmux_server::setup_message(why)))?;
            // On Linux / macOS a session's path is made canonical here; inside WSL it is
            // compared as written (the project's own path came from `pwd -P` there).
            let wsl = self.platform.in_wsl();
            let canon = |p: &str| {
                if wsl {
                    return p.to_string();
                }
                std::fs::canonicalize(p)
                    .map_or_else(|_| p.to_string(), |c| c.to_string_lossy().into_owned())
            };
            let here = project.to_string_lossy();
            match crate::tmux_server::decide(&rows, &tag, &here, &canon) {
                Lookup::Live => {
                    return self
                        .platform
                        .rein_attach(project, &tag, None)
                        .map(Start::Spec);
                }
                Lookup::Adopt(id) => {
                    return self
                        .platform
                        .rein_attach(project, &tag, Some(&id))
                        .map(Start::Spec);
                }
                Lookup::Two => return Ok(Start::Two),
                Lookup::None if restore => return Ok(Start::NotRunning),
                Lookup::None => {}
            }
        }
        if setup {
            self.platform.rein_setup(project, server).map(Start::Spec)
        } else {
            self.platform.rein_start(project, server).map(Start::Spec)
        }
    }

    /// A card tab (V12 components 5, 9): an id and the project, counted against the tab
    /// limit, no process.
    fn open_card(
        &self,
        project: std::path::PathBuf,
        server: crate::tmux_server::Server,
        kind: crate::restore::CardKind,
    ) -> Result<crate::project_setup::Opened, IpcError> {
        let tabs = self.tabs();
        let waiting = self
            .projects
            .setup
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut cards = self.cards.lock().unwrap_or_else(|e| e.into_inner());
        if tabs.len() + waiting.len() + cards.len() >= MAX_TABS {
            return Err(err("too_many_tabs", "at most 16 tabs are open at once"));
        }
        let id = self.next_tab.fetch_add(1, Ordering::Relaxed) + 1;
        let view = crate::restore::CardView {
            id,
            project: project.to_string_lossy().into_owned(),
            title: title_of(&project),
            kind,
        };
        cards.insert(
            id,
            crate::restore::Card {
                project,
                server,
                kind,
            },
        );
        Ok(crate::project_setup::Opened::Card { card: view })
    }

    /// A set-up tab: an id and the project, counted against the tab limit, no process.
    fn open_setup(
        &self,
        project: std::path::PathBuf,
        server: crate::tmux_server::Server,
        inside: Option<std::path::PathBuf>,
    ) -> Result<crate::project_setup::Opened, IpcError> {
        let tabs = self.tabs();
        let mut setup = self
            .projects
            .setup
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let cards = self.cards.lock().unwrap_or_else(|e| e.into_inner()).len();
        if tabs.len() + setup.len() + cards >= MAX_TABS {
            return Err(err("too_many_tabs", "at most 16 tabs are open at once"));
        }
        let id = self.next_tab.fetch_add(1, Ordering::Relaxed) + 1;
        let view = crate::project_setup::SetupView {
            id,
            project: project.to_string_lossy().into_owned(),
            title: title_of(&project),
            inside: inside.as_ref().map(|p| p.to_string_lossy().into_owned()),
        };
        setup.insert(
            id,
            crate::project_setup::SetupTab {
                project,
                server,
                inside,
            },
        );
        Ok(crate::project_setup::Opened::SetUp { setup: view })
    }

    /// Set up this project (V9 component 2): the set-up tab `tab` gets its terminal, running
    /// `rein init` and then the tab's start, under the same id.
    pub fn setup(
        &self,
        tab: u32,
        cols: u32,
        rows: u32,
        events: Arc<dyn Events>,
    ) -> Result<Tab, IpcError> {
        let size = validate_size(cols, rows)?;
        let s = self
            .projects
            .setup_tab(tab)
            .ok_or_else(|| err("unknown_tab", "the tab is not waiting for set-up"))?;
        // A folder inside a Rein project is never set up (E-63 rework 3).
        if s.inside.is_some() {
            return Err(err(
                "inside_project",
                "this folder is inside a Rein project — open that project instead",
            ));
        }
        let Start::Spec(spec) = self.start_spec(&s.project, s.server, true, false)? else {
            return Err(err(
                "two_sessions",
                "two sessions are already running for this folder — close one first",
            ));
        };
        // Taken out under the tabs lock in `open_spec_as`, so it counts once throughout.
        self.open_spec_as(&spec, &s.project, size, events, s.server, Some(tab))
    }

    /// The part of [`open`](Self::open) after validation; tests pass stub children here.
    #[cfg(all(test, unix))]
    pub fn open_spec(
        &self,
        spec: &ChildSpec,
        project: &Path,
        size: Size,
        events: Arc<dyn Events>,
    ) -> Result<Tab, IpcError> {
        self.open_spec_as(
            spec,
            project,
            size,
            events,
            crate::tmux_server::Server::App,
            None,
        )
    }

    /// [`open_spec`](Self::open_spec), recording the tmux server the child reaches; `setup`:
    /// the set-up tab this becomes (its id kept).
    pub(crate) fn open_spec_as(
        &self,
        spec: &ChildSpec,
        project: &Path,
        size: Size,
        events: Arc<dyn Events>,
        server: crate::tmux_server::Server,
        setup: Option<u32>,
    ) -> Result<Tab, IpcError> {
        // The limit is checked and the tab recorded under one lock, so concurrent opens
        // (commands run on a thread pool) cannot pass it together.
        let mut tabs = self.tabs();
        let mut waiting = self
            .projects
            .setup
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut cards = self.cards.lock().unwrap_or_else(|e| e.into_inner());
        if setup.is_some_and(|t| !waiting.contains_key(&t) && !cards.contains_key(&t)) {
            return Err(err("unknown_tab", "the tab is not waiting for set-up"));
        }
        let others = waiting.len() + cards.len() - usize::from(setup.is_some());
        if tabs.len() + others >= MAX_TABS {
            return Err(err("too_many_tabs", "at most 16 tabs are open at once"));
        }
        let pty = self.host.open(spec, size, events)?;
        let id = match setup {
            Some(t) => {
                waiting.remove(&t);
                cards.remove(&t);
                t
            }
            None => self.next_tab.fetch_add(1, Ordering::Relaxed) + 1,
        };
        let title = title_of(project);
        let tab = Tab {
            id,
            project: project.to_string_lossy().into_owned(),
            pty,
            title,
            server,
        };
        tabs.insert(id, tab.clone());
        Ok(tab)
    }

    pub fn write(&self, pty: PtyId, data: &str) -> Result<(), IpcError> {
        Ok(self.host.write(pty, validate_write(data)?)?)
    }

    pub fn resize(&self, pty: PtyId, cols: u32, rows: u32) -> Result<(), IpcError> {
        Ok(self.host.resize(pty, validate_size(cols, rows)?)?)
    }

    pub fn ack(&self, pty: PtyId, bytes: u32) -> Result<(), IpcError> {
        Ok(self.host.ack(pty, bytes)?)
    }

    /// Style the tmux session `tab` shows (V6, D-050): `mode` is a status mode or `off`. The
    /// tab's tty is waited for (on Windows the start script prints it) at most
    /// [`TTY_WAIT`]. `None`: the tab is not open (closed meanwhile) — nothing to report.
    pub fn tmux_overlay(
        &self,
        tab: u32,
        mode: &str,
        colours: &Colours,
    ) -> Option<Result<(), String>> {
        let (pty, server, project) = {
            let tabs = self.tabs();
            let t = tabs.get(&tab)?;
            (t.pty, t.server, std::path::PathBuf::from(&t.project))
        };
        // V12 component 1: the attach step tags the session on the app's server only.
        let tag = (server == crate::tmux_server::Server::App).then(|| self.tag_of(&project));
        let seq = |f: &mut dyn FnMut(&mut HashMap<u32, u64>) -> u64| {
            f(&mut self.overlay_seq.lock().unwrap_or_else(|e| e.into_inner()))
        };
        let mine = seq(&mut |m| {
            let n = m.entry(tab).or_default();
            *n += 1;
            *n
        });
        // The tab is still open on the same pty and its child is running (E-56): every wait
        // below — the tty, then the script's wait for the tmux client — ends when it is not.
        let alive = || self.tabs().get(&tab).map(|t| t.pty) == Some(pty) && self.host.alive(pty);
        let deadline = Instant::now() + TTY_WAIT;
        let tty = loop {
            match self.host.tty(pty) {
                Ok(Some(t)) => break t,
                Ok(None) if !alive() => return None,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                Ok(None) => return Some(Err(tmux_overlay::TTY_NOT_SEEN.into())),
                Err(_) => return None,
            }
        };
        let args = match tmux_overlay::args(&tty, mode, colours, server, tag.as_deref()) {
            Ok(a) => a,
            Err(why) => return Some(Err(why)),
        };
        let _one = self.overlay_run.lock().unwrap_or_else(|e| e.into_inner());
        if seq(&mut |m| m.get(&tab).copied().unwrap_or_default()) != mine {
            return None; // a newer request for this tab runs instead
        }
        let outcome = self.platform.tmux_overlay(&args, &alive);
        // A tab closed (or a terminal that ended) while the script ran: nothing to report —
        // a styled session restored itself through the hook.
        alive().then_some(outcome)
    }

    /// The page loaded again while the core still held tabs (a reload, V12 component 10):
    /// end their ptys — a detach, nothing is killed, the sessions stay — and forget the
    /// set-up tabs and cards, so the page's start-up restore brings the same tabs back with
    /// one client each. Returns how many were held.
    pub fn reset_page(&self) -> usize {
        let mut ids: Vec<u32> = self.tabs().keys().copied().collect();
        ids.extend(
            self.projects
                .setup
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .keys()
                .copied(),
        );
        ids.extend(
            self.cards
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .keys()
                .copied(),
        );
        for id in &ids {
            self.detach(*id);
        }
        ids.len()
    }

    /// The window is closing: detach every tab and reap every child before the process ends.
    pub fn detach_all(&self) {
        self.tabs().clear();
        self.host.detach_all();
    }

    /// Close the tab and detach its pty. An unknown tab is a no-op (desktop.md §API: no error).
    pub fn detach(&self, tab: u32) {
        if let Some(t) = self.tabs().remove(&tab) {
            let _ = self.host.detach(t.pty);
        }
        self.overlay_seq
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&tab);
        self.panels.forget(tab);
        self.agents.forget(tab);
        self.projects.forget(tab);
        self.cards
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&tab);
    }
}

/// A tab's title: its directory's name.
fn title_of(project: &Path) -> String {
    project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into())
}

/// The events of a tab that the web side draws: output on `output`, exit as `pty_exit`.
pub fn channel_events(output: Channel<InvokeResponseBody>, app: AppHandle) -> Arc<dyn Events> {
    Arc::new(ChannelEvents { output, app })
}

/// Output to the tab's channel as raw bytes; exit as the `pty_exit` event.
struct ChannelEvents {
    output: Channel<InvokeResponseBody>,
    app: AppHandle,
}

#[derive(Clone, Serialize)]
struct PtyExit {
    id: PtyId,
    code: u32,
}

impl Events for ChannelEvents {
    fn output(&self, _: PtyId, bytes: Vec<u8>) {
        let _ = self.output.send(InvokeResponseBody::Raw(bytes));
    }
    fn exit(&self, id: PtyId, code: u32) {
        let _ = self.app.emit(EVENT_PTY_EXIT, PtyExit { id, code });
    }
}

// The Tauri command functions (E-83: moved, unchanged) and their tests.
mod handlers;
pub use handlers::*;

#[cfg(all(test, unix))]
mod tests;
