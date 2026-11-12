//! IPC for the click-first layer (desktop.md §API, §Security 6, D-034; E-40): the folder
//! dialog (`project_choose`), paste (`clipboard_read`) and the app's own settings
//! (`ui_get` / `ui_set`). A click resolves to an `Action` through the same table as a key,
//! so these add no power: the dialog's answer passes the same host validation as
//! `tab_open`, paste is read only from a user's paste action, and `ui_set` takes only the
//! listed fields.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::config::ConfigService;
use crate::ipc::{Desktop, IpcError, MAIN_WINDOW, err};
use crate::ui_store::{UiStore, UiView};

/// The most text one paste may carry (desktop.md §API `clipboard_read`).
pub const MAX_PASTE: usize = 1024 * 1024;

/// The system folder dialog (D-046: `rfd`, no Tauri plugin), starting at the host's home.
/// `Ok(None)`: cancelled. A chosen folder is validated exactly as `tab_open` validates and
/// returned canonical.
#[tauri::command(async)]
pub fn project_choose(
    app: AppHandle,
    desktop: State<'_, Desktop>,
) -> Result<Option<String>, IpcError> {
    let start = desktop.platform.dialog_start();
    let Some(path) = pick_folder(&app, start)? else {
        return Ok(None);
    };
    let project = desktop.platform.project(&path.to_string_lossy())?;
    Ok(Some(project.to_string_lossy().into_owned()))
}

fn dialog(start: Option<std::path::PathBuf>) -> rfd::FileDialog {
    let d = rfd::FileDialog::new().set_title("Open a Rein project");
    match start {
        Some(dir) => d.set_directory(dir),
        None => d,
    }
}

/// Linux and Windows: from this IPC worker thread. rfd's GTK backend posts the dialog to the
/// default GLib main loop — the window's — and waits; calling it *on* the main thread would
/// wait on itself and freeze the window. The Windows COM dialog runs on any thread.
#[cfg(not(target_os = "macos"))]
fn pick_folder(
    _app: &AppHandle,
    start: Option<std::path::PathBuf>,
) -> Result<Option<std::path::PathBuf>, IpcError> {
    Ok(dialog(start).pick_folder())
}

/// macOS: AppKit dialogs must run on the main thread.
#[cfg(target_os = "macos")]
fn pick_folder(
    app: &AppHandle,
    start: Option<std::path::PathBuf>,
) -> Result<Option<std::path::PathBuf>, IpcError> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(dialog(start).pick_folder());
    })
    .map_err(|e| err("dialog", e.to_string()))?;
    rx.recv()
        .map_err(|_| err("dialog", "the folder dialog did not answer"))
}

/// Paste (`term.paste`, a user's action only): the clipboard's text, at most 1 MiB.
#[tauri::command(async)]
pub fn clipboard_read() -> Result<String, IpcError> {
    let text = arboard::Clipboard::new()
        .and_then(|mut c| c.get_text())
        .map_err(|e| err("clipboard", e.to_string()))?;
    validate_paste(text)
}

pub fn validate_paste(text: String) -> Result<String, IpcError> {
    if text.len() > MAX_PASTE {
        return Err(err(
            "too_large",
            "the clipboard holds more than 1 MiB of text",
        ));
    }
    Ok(text)
}

#[tauri::command(async)]
pub fn ui_get(ui: State<'_, UiStore>, config: State<'_, Arc<ConfigService>>) -> UiView {
    ui.view(&config.view())
}

/// One listed field of ui.json, type-checked; a field config.toml sets is refused. The title
/// bar applies at once.
#[tauri::command(async)]
pub fn ui_set(
    app: AppHandle,
    ui: State<'_, UiStore>,
    config: State<'_, Arc<ConfigService>>,
    field: String,
    value: Value,
) -> Result<UiView, IpcError> {
    let cfg = config.view();
    ui.set(&cfg, &field, &value)?;
    let view = ui.view(&cfg);
    if field == "decorations"
        && let (Some(w), Some(on)) = (
            app.get_webview_window(MAIN_WINDOW),
            view.decorations.value.as_bool(),
        )
    {
        let _ = w.set_decorations(on);
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paste_is_bounded() {
        assert_eq!(
            validate_paste("x".repeat(MAX_PASTE + 1)).unwrap_err().kind,
            "too_large"
        );
        assert!(validate_paste("x".repeat(MAX_PASTE)).is_ok());
    }
}
