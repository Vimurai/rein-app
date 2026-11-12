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

use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State};

use crate::host::Host;
use crate::pty::{ChildSpec, Events, PtyError, PtyHost, PtyId, Size};

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
}

/// The window core's state for the terminal stage.
pub struct Desktop {
    /// Linux / macOS (`LocalHost`) or Windows (`WslHost`): how a project is validated and
    /// what the pty child is.
    pub platform: Box<dyn Host>,
    pub host: PtyHost,
    tabs: Mutex<HashMap<u32, Tab>>,
    next_tab: AtomicU32,
}

impl Desktop {
    pub fn new(platform: Box<dyn Host>) -> Self {
        Self {
            platform,
            host: PtyHost::default(),
            tabs: Mutex::new(HashMap::new()),
            next_tab: AtomicU32::new(0),
        }
    }

    fn tabs(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Tab>> {
        self.tabs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Validate, then start `rein start` in `project` with output going to `events`.
    pub fn open(
        &self,
        project: &str,
        cols: u32,
        rows: u32,
        events: Arc<dyn Events>,
    ) -> Result<Tab, IpcError> {
        let size = validate_size(cols, rows)?;
        let project = self.platform.project(project)?;
        let spec = self.platform.rein_start(&project)?;
        self.open_spec(&spec, &project, size, events)
    }

    /// The part of [`open`](Self::open) after validation; tests pass stub children here.
    pub fn open_spec(
        &self,
        spec: &ChildSpec,
        project: &Path,
        size: Size,
        events: Arc<dyn Events>,
    ) -> Result<Tab, IpcError> {
        // The limit is checked and the tab recorded under one lock, so concurrent opens
        // (commands run on a thread pool) cannot pass it together.
        let mut tabs = self.tabs();
        if tabs.len() >= MAX_TABS {
            return Err(err("too_many_tabs", "at most 16 tabs are open at once"));
        }
        let pty = self.host.open(spec, size, events)?;
        let id = self.next_tab.fetch_add(1, Ordering::Relaxed) + 1;
        let title = project
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".into());
        let tab = Tab {
            id,
            project: project.to_string_lossy().into_owned(),
            pty,
            title,
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
    }
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

#[tauri::command(async)]
pub fn tab_open(
    app: AppHandle,
    state: State<'_, Desktop>,
    project: String,
    cols: u32,
    rows: u32,
    output: Channel<InvokeResponseBody>,
) -> Result<Tab, IpcError> {
    state.open(
        &project,
        cols,
        rows,
        Arc::new(ChannelEvents { output, app }),
    )
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
        let plain = tempfile::tempdir().unwrap();
        let p = plain.path().to_str().unwrap();
        assert_eq!(kind(d.open(p, 80, 24, q())), "no_ai");
        assert_eq!(kind(d.open("/a\u{1b}[2Jb", 80, 24, q())), "unsafe_path");
        assert_eq!(kind(d.open("/a\u{202e}b", 80, 24, q())), "unsafe_path");
        assert_eq!(kind(d.open("relative", 80, 24, q())), "unsafe_path");
        assert_eq!(
            kind(d.open("/definitely/not/here", 80, 24, q())),
            "not_a_directory"
        );
        let file = plain.path().join("f");
        std::fs::write(&file, "").unwrap();
        assert_eq!(
            kind(d.open(file.to_str().unwrap(), 80, 24, q())),
            "not_a_directory"
        );
        let proj = project();
        // A valid project, but rein is not installed under this home.
        assert_eq!(
            kind(d.open(proj.path().to_str().unwrap(), 80, 24, q())),
            "rein_missing"
        );
        assert_eq!(
            kind(d.open(proj.path().to_str().unwrap(), 1, 24, q())),
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
