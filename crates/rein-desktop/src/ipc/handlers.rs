//! The Tauri commands of the IPC table (desktop.md §API). Moved from ipc.rs unchanged (E-83,
//! D-061 addendum 5 ruling 3); the types and `Desktop` they call are in `ipc/mod.rs`.

use super::*;

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
    restore: Option<bool>,
) -> Result<crate::project_setup::Opened, IpcError> {
    // V8: which tmux server this tab reaches, from the settings as they are now.
    let view = ui.view(&config.view());
    let server = crate::tmux_server::Server::from_setting(view.tmux_server.value.as_str());
    let events = channel_events(output, app);
    let restore = restore.unwrap_or(false);
    let opened = state.open_as(&project, cols, rows, events, server, restore)?;
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
    crate::debug_trace::pty_resize(pty, cols, rows);
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
        None,
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
