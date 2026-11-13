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
    include!("ipc_commands.rs");
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
    /// the overlay talks to that one. Not sent to the web side.
    #[serde(skip)]
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

    /// The project a tab of this window runs in, as recorded when it opened — the only path a
    /// panel source reads under (panels.md §API: the web side never sends a path).
    pub fn tab_project(&self, tab: u32) -> Option<std::path::PathBuf> {
        self.tabs()
            .get(&tab)
            .map(|t| std::path::PathBuf::from(&t.project))
    }

    fn tabs(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Tab>> {
        self.tabs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Validate, then start `rein start` in `project` with output going to `events`. A
    /// directory without `.ai/` opens as a set-up tab instead, with no pty (V9 component 1).
    pub fn open(
        &self,
        project: &str,
        cols: u32,
        rows: u32,
        events: Arc<dyn Events>,
        server: crate::tmux_server::Server,
    ) -> Result<crate::project_setup::Opened, IpcError> {
        let size = validate_size(cols, rows)?;
        let (project, set_up) = self.platform.project_dir(project)?;
        if !set_up {
            return self.open_setup(project, server);
        }
        let spec = self.start_spec(&project, server, false)?;
        self.open_spec_as(&spec, &project, size, events, server, None)
            .map(crate::project_setup::Opened::Tab)
    }

    /// The child of a tab: `rein start`, or with `setup` `rein init` first (V9 component 2).
    /// V8 3a: the tmux home is made or checked before the pty opens; on failure the pty is not
    /// opened and the window shows the app's fixed sentence.
    fn start_spec(
        &self,
        project: &Path,
        server: crate::tmux_server::Server,
        setup: bool,
    ) -> Result<ChildSpec, IpcError> {
        if server == crate::tmux_server::Server::App {
            self.platform
                .tmux_setup()
                .map_err(|why| err("tmux_setup", crate::tmux_server::setup_message(why)))?;
        }
        if setup {
            self.platform.rein_setup(project, server)
        } else {
            self.platform.rein_start(project, server)
        }
    }

    /// A set-up tab: an id and the project, counted against the tab limit, no process.
    fn open_setup(
        &self,
        project: std::path::PathBuf,
        server: crate::tmux_server::Server,
    ) -> Result<crate::project_setup::Opened, IpcError> {
        let tabs = self.tabs();
        let mut setup = self
            .projects
            .setup
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if tabs.len() + setup.len() >= MAX_TABS {
            return Err(err("too_many_tabs", "at most 16 tabs are open at once"));
        }
        let id = self.next_tab.fetch_add(1, Ordering::Relaxed) + 1;
        let view = crate::project_setup::SetupView {
            id,
            project: project.to_string_lossy().into_owned(),
            title: title_of(&project),
        };
        setup.insert(id, crate::project_setup::SetupTab { project, server });
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
        let spec = self.start_spec(&s.project, s.server, true)?;
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
    fn open_spec_as(
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
        if setup.is_some_and(|t| !waiting.contains_key(&t)) {
            return Err(err("unknown_tab", "the tab is not waiting for set-up"));
        }
        let others = waiting.len() - usize::from(setup.is_some());
        if tabs.len() + others >= MAX_TABS {
            return Err(err("too_many_tabs", "at most 16 tabs are open at once"));
        }
        let pty = self.host.open(spec, size, events)?;
        let id = match setup {
            Some(t) => {
                waiting.remove(&t);
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
        let (pty, server) = {
            let tabs = self.tabs();
            let t = tabs.get(&tab)?;
            (t.pty, t.server)
        };
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
        let args = match tmux_overlay::args(&tty, mode, colours, server) {
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

// Tauri hands the command its managed states as arguments; three of the nine are those.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub fn tab_open(
    app: AppHandle,
    state: State<'_, Desktop>,
    projects: State<'_, Arc<crate::projects::ProjectStore>>,
    ui: State<'_, crate::ui_store::UiStore>,
    config: State<'_, Arc<crate::config::ConfigService>>,
    project: String,
    cols: u32,
    rows: u32,
    output: Channel<InvokeResponseBody>,
) -> Result<crate::project_setup::Opened, IpcError> {
    // V8: which tmux server this tab reaches, from the settings as they are now.
    let view = ui.view(&config.view());
    let server = crate::tmux_server::Server::from_setting(view.tmux_server.value.as_str());
    let opened = state.open(&project, cols, rows, channel_events(output, app), server)?;
    // Recent projects (E-38): the canonical path the tab runs in — only a project, never a
    // folder still waiting for set-up. A failed save only loses the list entry.
    if let crate::project_setup::Opened::Tab(tab) = &opened {
        let _ = projects.touch(&tab.project, state.platform.distro());
    }
    Ok(opened)
}

/// `projects_list` (desktop.md §API, E-38): the recent projects, newest first.
#[tauri::command(async)]
pub fn projects_list(
    projects: State<'_, Arc<crate::projects::ProjectStore>>,
) -> Vec<crate::projects::RecentView> {
    // Debug builds only: `REIN_DESKTOP_SLOW_LIST_MS` holds the answer back, so the start
    // screen's skeleton rows (V7) can be looked at and screenshotted; release builds ignore it,
    // as they do `REIN_DESKTOP_CHROME`.
    #[cfg(debug_assertions)]
    if let Some(ms) =
        crate::dirs::process_env("REIN_DESKTOP_SLOW_LIST_MS").and_then(|v| v.parse::<u64>().ok())
    {
        std::thread::sleep(Duration::from_millis(ms.min(30_000)));
    }
    projects.list()
}

/// `projects_forget` (E-38): drop one recent project, then the list.
#[tauri::command(async)]
pub fn projects_forget(
    projects: State<'_, Arc<crate::projects::ProjectStore>>,
    path: String,
) -> Result<Vec<crate::projects::RecentView>, IpcError> {
    projects
        .forget(&path)
        .map_err(|e| err("io", format!("cannot save the recent projects: {e}")))?;
    Ok(projects.list())
}

#[tauri::command(async)]
pub fn pty_write(state: State<'_, Desktop>, pty: PtyId, data: String) -> Result<(), IpcError> {
    state.write(pty, &data)
}

#[tauri::command(async)]
pub fn pty_resize(
    state: State<'_, Desktop>,
    pty: PtyId,
    cols: u32,
    rows: u32,
) -> Result<(), IpcError> {
    state.resize(pty, cols, rows)
}

#[tauri::command(async)]
pub fn pty_ack(state: State<'_, Desktop>, pty: PtyId, bytes: u32) -> Result<(), IpcError> {
    state.ack(pty, bytes)
}

#[tauri::command(async)]
pub fn tab_detach(state: State<'_, Desktop>, tab: u32) {
    state.detach(tab);
}

/// The overlay mode the settings ask for: a status mode, or `off` when the overlay is off.
pub fn overlay_mode(ui: &crate::ui_store::UiView) -> String {
    match (
        ui.tmux_overlay.value.as_bool(),
        ui.tmux_status.value.as_str(),
    ) {
        (Some(false), _) => "off".into(),
        (_, Some(m)) => m.into(),
        _ => "hidden".into(),
    }
}

/// `tab_tmux` (desktop.md §Look and feel v2 V6, D-050): apply the window's tmux look to the
/// session `tab` shows, in the active theme's `colours` — after the tab opens and on a
/// theme or setting change. Only the colours come from the web side, each checked as
/// `#rrggbb`; the mode comes from the settings. A failure is never an error here: tmux stays
/// as the user has it, the health view says why, and the answer carries the same words so
/// the window can say so once (V6, E-54) — `None` when applied or when the tab has closed.
#[tauri::command(async)]
pub fn tab_tmux(
    state: State<'_, Desktop>,
    ui: State<'_, crate::ui_store::UiStore>,
    config: State<'_, Arc<crate::config::ConfigService>>,
    tab: u32,
    colours: Colours,
) -> Result<Option<String>, IpcError> {
    let mode = overlay_mode(&ui.view(&config.view()));
    // Colours first: a refusal here is the web side's mistake, not tmux's.
    tmux_overlay::args(
        "/dev/pts/0",
        &mode,
        &colours,
        crate::tmux_server::Server::App,
    )
    .map_err(|why| err("bad_value", why))?;
    let outcome = state.tmux_overlay(tab, &mode, &colours);
    if let Some(o) = &outcome {
        state.overlay.record(o);
    }
    // Only the app's own fixed sentences reach the window (V7, E-55).
    Ok(outcome
        .and_then(Result::err)
        .map(|why| tmux_overlay::not_applied(tmux_overlay::told(&why))))
}

/// `config_get` (desktop.md §API, E-37): the active config, the file's path and the last
/// refusal. `reload` re-reads the file first (`config.reload`).
#[tauri::command(async)]
pub fn config_get(
    config: State<'_, std::sync::Arc<crate::config::ConfigService>>,
    reload: Option<bool>,
) -> crate::config::ConfigView {
    if reload == Some(true) {
        config.reload()
    } else {
        config.view()
    }
}

/// `:open` Tab completion (E-36), through the host: directories starting with `prefix`.
#[tauri::command(async)]
pub fn dir_complete(state: State<'_, Desktop>, prefix: String) -> Result<Vec<String>, IpcError> {
    state.platform.complete_dir(&prefix)
}

pub fn validate_clipboard(text: &str) -> Result<&str, IpcError> {
    if text.len() > MAX_CLIPBOARD {
        return Err(err("too_large", "text larger than 64 KiB"));
    }
    Ok(text)
}

/// One clipboard handle for the window's life: on X11 the copied text is served by the
/// program that set it, and dropping the handle can take the text with it.
static CLIPBOARD: std::sync::OnceLock<Mutex<Option<arboard::Clipboard>>> =
    std::sync::OnceLock::new();

/// `cmd.yank` (E-36): put `text` — a command the window showed — on the system clipboard.
/// Write only; reading the clipboard is a separate, user-gesture command (E-40).
#[tauri::command(async)]
pub fn clipboard_write(text: String) -> Result<(), IpcError> {
    let text = validate_clipboard(&text)?;
    let cell = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut slot = cell.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = Some(arboard::Clipboard::new().map_err(|e| err("clipboard", e.to_string()))?);
    }
    slot.as_mut()
        .map(|c| c.set_text(text))
        .transpose()
        .map_err(|e| err("clipboard", e.to_string()))?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::local_host::{LocalHost, validate_project};
    use std::sync::mpsc;
    use std::time::Duration;

    struct Quiet(Mutex<Option<mpsc::Sender<u32>>>);
    impl Events for Quiet {
        fn output(&self, _: PtyId, _: Vec<u8>) {}
        fn exit(&self, _: PtyId, code: u32) {
            if let Some(tx) = self.0.lock().unwrap().take() {
                let _ = tx.send(code);
            }
        }
    }

    fn project() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join(".ai")).unwrap();
        t
    }

    fn kind<T: std::fmt::Debug>(r: Result<T, IpcError>) -> &'static str {
        r.unwrap_err().kind
    }

    #[test]
    fn tab_open_refuses_bad_projects() {
        let d = Desktop::new(Box::new(LocalHost::new(Some("/nonexistent-home".into()))));
        let q = || -> Arc<dyn Events> { Arc::new(Quiet(Mutex::new(None))) };
        // V9 (E-63): a directory without `.ai/` is not refused — it opens as a set-up tab,
        // with no process, counted against the tab limit, forgotten when closed.
        let plain = tempfile::tempdir().unwrap();
        let p = plain.path().to_str().unwrap();
        let canon = plain.path().canonicalize().unwrap();
        let id = match d.open(p, 80, 24, q(), crate::tmux_server::Server::User) {
            Ok(crate::project_setup::Opened::SetUp { setup }) => {
                assert_eq!(setup.project, canon.to_string_lossy());
                setup.id
            }
            other => panic!("{other:?}"),
        };
        assert!(d.tabs().is_empty(), "no pty for a set-up tab");
        assert!(
            d.tab_project(id).is_none(),
            "not a project tab until set up"
        );
        for _ in 1..MAX_TABS {
            d.open(p, 80, 24, q(), crate::tmux_server::Server::User)
                .unwrap();
        }
        assert_eq!(
            kind(d.open(p, 80, 24, q(), crate::tmux_server::Server::User)),
            "too_many_tabs"
        );
        for t in 0..=MAX_TABS as u32 + 1 {
            d.detach(t);
        }
        assert!(d.projects.setup.lock().unwrap().is_empty());
        // Set up on a host without rein: refused before any process, the tab still waiting.
        let id = match d.open(p, 80, 24, q(), crate::tmux_server::Server::User) {
            Ok(crate::project_setup::Opened::SetUp { setup }) => setup.id,
            other => panic!("{other:?}"),
        };
        assert_eq!(kind(d.setup(id, 80, 24, q())), "rein_missing");
        assert!(d.projects.setup_tab(id).is_some());
        assert_eq!(kind(d.setup(id + 100, 80, 24, q())), "unknown_tab");
        d.detach(id);
        assert_eq!(
            kind(d.open(
                "/a\u{1b}[2Jb",
                80,
                24,
                q(),
                crate::tmux_server::Server::User
            )),
            "unsafe_path"
        );
        assert_eq!(
            kind(d.open("/a\u{202e}b", 80, 24, q(), crate::tmux_server::Server::User)),
            "unsafe_path"
        );
        assert_eq!(
            kind(d.open("relative", 80, 24, q(), crate::tmux_server::Server::User)),
            "unsafe_path"
        );
        assert_eq!(
            kind(d.open(
                "/definitely/not/here",
                80,
                24,
                q(),
                crate::tmux_server::Server::User
            )),
            "not_a_directory"
        );
        let file = plain.path().join("f");
        std::fs::write(&file, "").unwrap();
        assert_eq!(
            kind(d.open(
                file.to_str().unwrap(),
                80,
                24,
                q(),
                crate::tmux_server::Server::User
            )),
            "not_a_directory"
        );
        let proj = project();
        // A valid project, but rein is not installed under this home.
        assert_eq!(
            kind(d.open(
                proj.path().to_str().unwrap(),
                80,
                24,
                q(),
                crate::tmux_server::Server::User
            )),
            "rein_missing"
        );
        assert_eq!(
            kind(d.open(
                proj.path().to_str().unwrap(),
                1,
                24,
                q(),
                crate::tmux_server::Server::User
            )),
            "out_of_range"
        );
    }

    #[test]
    fn a_symlink_to_a_project_opens_as_its_canonical_path() {
        let proj = project();
        let link_dir = tempfile::tempdir().unwrap();
        let link = link_dir.path().join("link");
        std::os::unix::fs::symlink(proj.path(), &link).unwrap();
        assert_eq!(
            validate_project(link.to_str().unwrap()).unwrap(),
            proj.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn a_clipboard_write_is_bounded() {
        assert_eq!(
            kind(validate_clipboard(&"x".repeat(MAX_CLIPBOARD + 1))),
            "too_large"
        );
        assert!(validate_clipboard(&"x".repeat(MAX_CLIPBOARD)).is_ok());
    }

    #[test]
    fn resize_and_write_are_bounded() {
        for (c, r) in [
            (1, 24),
            (501, 24),
            (80, 1),
            (80, 201),
            (0, 0),
            (u32::MAX, 24),
        ] {
            assert_eq!(kind(validate_size(c, r)), "out_of_range", "{c}x{r}");
        }
        assert!(validate_size(2, 2).is_ok() && validate_size(500, 200).is_ok());
        assert_eq!(
            kind(validate_write(&"x".repeat(MAX_WRITE + 1))),
            "too_large"
        );
        assert!(validate_write(&"x".repeat(MAX_WRITE)).is_ok());
    }

    #[test]
    fn unknown_ptys_and_oversized_acks_are_refused() {
        let d = Desktop::new(Box::new(LocalHost::new(None)));
        assert_eq!(kind(d.write(42, "x")), "unknown_pty");
        assert_eq!(kind(d.resize(42, 80, 24)), "unknown_pty");
        assert_eq!(kind(d.ack(42, 1)), "unknown_pty");
        d.detach(42); // unknown tab: no error

        let proj = project();
        let (tx, rx) = mpsc::channel();
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "cat".into()],
            cwd: Some(proj.path().into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        let tab = d
            .open_spec(
                &spec,
                proj.path(),
                Size { cols: 80, rows: 24 },
                Arc::new(Quiet(Mutex::new(Some(tx)))),
            )
            .unwrap();
        // Nothing has been sent yet, so any ack is more than is outstanding.
        assert_eq!(kind(d.ack(tab.pty, 1)), "ack_too_large");
        assert_eq!(
            d.host.unacked(tab.pty).unwrap(),
            0,
            "a refused ack changes nothing"
        );
        assert_eq!(
            kind(d.write(tab.pty, &"x".repeat(MAX_WRITE + 1))),
            "too_large"
        );
        assert_eq!(kind(d.resize(tab.pty, 80, 999)), "out_of_range");
        d.detach(tab.id);
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(kind(d.write(tab.pty, "x")), "unknown_pty");
    }

    #[test]
    fn at_most_sixteen_tabs_even_when_opened_concurrently() {
        let d = Arc::new(Desktop::new(Box::new(LocalHost::new(None))));
        let proj = project();
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "cat".into()],
            cwd: Some(proj.path().into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        let handles: Vec<_> = (0..24)
            .map(|_| {
                let (d, spec, p) = (Arc::clone(&d), spec.clone(), proj.path().to_path_buf());
                std::thread::spawn(move || {
                    let q: Arc<dyn Events> = Arc::new(Quiet(Mutex::new(None)));
                    d.open_spec(&spec, &p, Size { cols: 80, rows: 24 }, q)
                        .is_ok()
                })
            })
            .collect();
        let opened = handles
            .into_iter()
            .filter(|_| true)
            .map(|h| h.join().unwrap())
            .filter(|&ok| ok)
            .count();
        assert_eq!(opened, MAX_TABS);
        d.detach_all();
    }

    /// A host that records the overlay's words and answers with `answer`.
    struct OverlayHost {
        seen: Arc<Mutex<Vec<Vec<String>>>>,
        answer: Result<(), String>,
        /// Stands in for a script still waiting for a tmux client that never attaches: the
        /// run lasts until `alive` turns false (at most 10 s).
        waits: bool,
    }
    impl Host for OverlayHost {
        fn project_dir(&self, _: &str) -> Result<(std::path::PathBuf, bool), IpcError> {
            unreachable!()
        }
        fn rein_setup(
            &self,
            _: &std::path::Path,
            _: crate::tmux_server::Server,
        ) -> Result<crate::pty::ChildSpec, IpcError> {
            unreachable!()
        }
        fn rein_command(
            &self,
            _: &std::path::Path,
            _: crate::project_setup::ReinRun,
        ) -> Result<std::process::Command, IpcError> {
            unreachable!()
        }
        fn rein_start(
            &self,
            _: &Path,
            _: crate::tmux_server::Server,
        ) -> Result<ChildSpec, IpcError> {
            unreachable!()
        }
        fn tmux_setup(&self) -> Result<(), &'static str> {
            unreachable!()
        }
        fn read_project_file(
            &self,
            _: &Path,
            _: crate::project_file::ProjectFile,
            _: bool,
        ) -> Result<crate::project_file::FileBytes, crate::project_file::Unavailable> {
            unreachable!()
        }
        fn complete_dir(&self, _: &str) -> Result<Vec<String>, IpcError> {
            unreachable!()
        }
        fn doctor(&self) -> crate::cli::HealthView {
            unreachable!()
        }
        fn run_cli(
            &self,
            _: &crate::cli::CliAction,
            _: &mut dyn FnMut(String),
        ) -> Result<Option<i32>, IpcError> {
            unreachable!()
        }
        fn tmux_overlay(&self, args: &[String], alive: &dyn Fn() -> bool) -> Result<(), String> {
            self.seen.lock().unwrap().push(args.to_vec());
            let until = std::time::Instant::now() + Duration::from_secs(10);
            while self.waits && alive() && std::time::Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
            self.answer.clone()
        }
        fn tmux_agents(&self, _: &[String]) -> Result<Vec<u8>, &'static str> {
            unreachable!()
        }
        fn rein_kill(&self, _: &Path, _: crate::tmux_server::Server) -> Result<(), &'static str> {
            unreachable!()
        }
        fn dialog_start(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    #[test]
    fn the_overlay_is_run_for_the_tabs_own_tty_and_never_for_a_closed_tab() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let d = Desktop::new(Box::new(OverlayHost {
            seen: Arc::clone(&seen),
            answer: Err("tmux is older than 3.2".into()),
            waits: false,
        }));
        let proj = project();
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "cat".into()],
            cwd: Some(proj.path().into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        let q: Arc<dyn Events> = Arc::new(Quiet(Mutex::new(None)));
        let tab = d
            .open_spec(&spec, proj.path(), Size { cols: 80, rows: 24 }, q)
            .unwrap();
        let c = Colours {
            bg: "#1e1e2e".into(),
            fg: "#cdd6f4".into(),
            muted: "#a6adc8".into(),
            border: "#6c7086".into(),
            accent: "#cba6f7".into(),
        };
        let out = d.tmux_overlay(tab.id, "app", &c);
        assert_eq!(out, Some(Err("tmux is older than 3.2".into())));
        let words = seen.lock().unwrap()[0].clone();
        assert_eq!(Some(words[0].clone()), d.host.tty(tab.pty).unwrap());
        assert_eq!(&words[1..3], ["app", "#1e1e2e"]);
        // A bad colour never reaches the host.
        let mut bad = c.clone();
        bad.bg = "#1e1e2e; kill-server".into();
        assert!(matches!(d.tmux_overlay(tab.id, "app", &bad), Some(Err(_))));
        assert_eq!(seen.lock().unwrap().len(), 1);
        d.detach(tab.id);
        assert_eq!(d.tmux_overlay(tab.id, "app", &c), None, "a closed tab");
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    /// E-56: while the overlay waits for the tab's tmux client (a fresh `rein start` attaches
    /// late; a failed one never), the tab's terminal ending stops the run at once, and nothing
    /// is recorded for a terminal that is gone.
    #[test]
    fn the_overlay_stops_waiting_when_the_tabs_terminal_ends() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let d = Desktop::new(Box::new(OverlayHost {
            seen: Arc::clone(&seen),
            answer: Err("the tab's tmux client was not attached within 60 seconds".into()),
            waits: true,
        }));
        let proj = project();
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 0.3".into()],
            cwd: Some(proj.path().into()),
            env_remove: crate::local_host::ENV_REMOVED,
            env_set: crate::local_host::ENV_SET,
            tty_line: false,
        };
        let (tx, rx) = mpsc::channel();
        let q: Arc<dyn Events> = Arc::new(Quiet(Mutex::new(Some(tx))));
        let tab = d
            .open_spec(&spec, proj.path(), Size { cols: 80, rows: 24 }, q)
            .unwrap();
        let c = Colours {
            bg: "#1e1e2e".into(),
            fg: "#cdd6f4".into(),
            muted: "#a6adc8".into(),
            border: "#6c7086".into(),
            accent: "#cba6f7".into(),
        };
        let t0 = std::time::Instant::now();
        assert_eq!(
            d.tmux_overlay(tab.id, "hidden", &c),
            None,
            "nothing to report"
        );
        assert!(
            t0.elapsed() < Duration::from_secs(3),
            "{:?}: waited past the exit",
            t0.elapsed()
        );
        assert_eq!(seen.lock().unwrap().len(), 1, "the script was started");
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    #[test]
    fn the_overlay_mode_follows_the_settings() {
        let t = tempfile::tempdir().unwrap();
        let ui = crate::ui_store::UiStore::open(Some(t.path().to_path_buf()));
        let cfg = crate::config::ConfigService::open(Some(t.path().to_path_buf())).view();
        assert_eq!(overlay_mode(&ui.view(&cfg)), "hidden");
        ui.set(&cfg, "tmux_status", &serde_json::Value::from("keep"))
            .unwrap();
        assert_eq!(overlay_mode(&ui.view(&cfg)), "keep");
        ui.set(&cfg, "tmux_overlay", &serde_json::Value::from(false))
            .unwrap();
        assert_eq!(overlay_mode(&ui.view(&cfg)), "off");
    }

    #[test]
    fn the_command_table_is_the_one_the_capability_grants() {
        let cap: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/main.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let granted: Vec<String> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| {
                p.as_str()?
                    .strip_prefix("allow-")
                    .map(|s| s.replace('-', "_"))
            })
            .collect();
        let mut want: Vec<String> = commands::COMMANDS.iter().map(|s| s.to_string()).collect();
        let mut got = granted.clone();
        want.sort();
        got.sort();
        assert_eq!(got, want);
    }
}
