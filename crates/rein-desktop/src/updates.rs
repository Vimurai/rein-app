//! The update watch (updates.md §Components 1, 2 and 4 for Rein, §Data Model, §API, D-065).
//!
//! One watch in the core holds what the window knows about updates and tells the web side
//! (`updates`) when it changes; the web side never checks on a timer of its own. Two facts
//! are watched here:
//! - the installed Rein's `~/.rein/VERSION` — a file read every 5 s on Linux / macOS, one
//!   constant read-only `wsl.exe` line every 30 s and at once on focus on Windows. A change
//!   (never the first read) re-runs the install health check; the web side re-runs the
//!   project check of its open tabs on the event;
//! - the newest published Rein — `rein-app update --check` through the CLI runner, 30 s after
//!   start, every `interval_min`, and on focus when the last check is 10 minutes old.
//!
//! Nothing runs while a CLI run of the window's own is in progress; its end triggers one read.
//! The schedule is a pure function of a clock passed in ([`Watch::step`]).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::app_update::{self, Doc, Job, Offer};
use crate::cli::{CliAction, ReleaseCheck};
use crate::cli_ipc::{CliState, EVENT_HEALTH_CHANGED};
use crate::config::{ConfigService, Updates};
use crate::ipc::{Desktop, IpcError, err};
use crate::ui_store::UiStore;
use rein_release::SignatureVerifier;

pub const EVENT_UPDATES: &str = "updates";
/// The first published check this long after start.
pub const START_DELAY_S: u64 = 30;
/// A focus checks again only when the last check is at least this old.
pub const FOCUS_MIN_S: u64 = 600;
/// The installed-version read: a file on Linux / macOS, `wsl.exe` on Windows.
pub const LOCAL_PERIOD_S: u64 = 5;
pub const WSL_PERIOD_S: u64 = 30;
/// The `wsl.exe` read's limit.
pub const VERSION_LIMIT: Duration = Duration::from_secs(5);
/// The longest version the window accepts; anything longer is "unknown".
pub const VERSION_MAX: usize = 64;
/// How often the watch's thread wakes; it does nothing unless something is due.
pub const TICK: Duration = Duration::from_secs(1);

/// The versions answered "Later" (kept in `ui.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Asked {
    pub rein: Option<String>,
    pub app: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReinFacts {
    pub installed: Option<String>,
    pub newest: Option<String>,
    /// Unix seconds of the last answered check.
    pub checked_at: Option<u64>,
    /// The last check could not reach the release server; `newest` is the last known.
    pub offline: bool,
}

/// The window's own update (component 3, E-86).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppFacts {
    pub running: &'static str,
    /// A newer published window, when one is offered.
    pub newest: Option<String>,
    pub notes: Option<String>,
    /// `in_place` (Windows: the core installs it) | `command` (Linux / macOS: the package
    /// manager's line) | `by_hand` (signed for another key: install it by hand) | `none`.
    pub mode: &'static str,
    pub command: Option<String>,
    /// The version this window was updated from, once after the restart ("Updated to X.").
    pub updated_from: Option<String>,
}

/// `UpdatesState` (updates.md §Data Model), sent whole on every change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdatesState {
    pub rein: ReinFacts,
    pub app: AppFacts,
    /// The window's own install (component 3): `idle`, `downloading`, `verifying`,
    /// `restarting` or `failed` with its step.
    pub job: Job,
    pub asked: Asked,
}

impl UpdatesState {
    pub fn new(asked: Asked, updated_from: Option<String>) -> Self {
        Self {
            rein: ReinFacts::default(),
            app: AppFacts {
                running: env!("CARGO_PKG_VERSION"),
                newest: None,
                notes: None,
                mode: "none",
                command: None,
                updated_from,
            },
            job: Job::Idle,
            asked,
        }
    }
}

/// The first line of `VERSION`: at most 64 bytes of `[0-9A-Za-z.+-]`, else `None` (unknown).
pub fn parse_version(bytes: &[u8]) -> Option<String> {
    let line = bytes.split(|b| *b == b'\n').next()?;
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let ok = (1..=VERSION_MAX).contains(&line.len())
        && line
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'));
    ok.then(|| String::from_utf8_lossy(line).into_owned())
}

/// At most 65 bytes of `path` (one more than a version, so a longer one reads as unknown).
#[cfg(unix)] // LocalHost (Linux, macOS); WslHost reads through wsl.exe
pub fn read_version_file(path: &std::path::Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(VERSION_MAX as u64 + 1)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// What is due at one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Due {
    pub read: bool,
    pub check: bool,
}

/// When to read and when to check, on a monotonic clock in seconds since the watch began.
#[derive(Debug, Default)]
pub struct Schedule {
    last_read: Option<u64>,
    last_check: Option<u64>,
    /// The last version read: `None` before the first read.
    installed: Option<Option<String>>,
    was_busy: bool,
}

impl Schedule {
    /// What is due at `now`. `focus`: the window gained focus since the last tick; `busy`: a
    /// CLI run of the window's own is in progress (nothing is due, and its end is one read).
    pub fn due(&mut self, now: u64, cfg: &Updates, period: u64, focus: bool, busy: bool) -> Due {
        if !cfg.check {
            return Due::default();
        }
        if busy {
            self.was_busy = true;
            return Due::default();
        }
        let ended = std::mem::take(&mut self.was_busy);
        let read = ended || focus || self.last_read.is_none_or(|l| now >= l + period);
        let check = now >= START_DELAY_S
            && self.last_check.is_none_or(|l| {
                now >= l + u64::from(cfg.interval_min) * 60 || (focus && now >= l + FOCUS_MIN_S)
            });
        Due { read, check }
    }

    /// A read ended at `now`; `Some(v)` when it answered. True when the installed version
    /// changed since the last answered read (never on the first).
    pub fn read_done(&mut self, now: u64, version: Option<Option<String>>) -> bool {
        self.last_read = Some(now);
        let Some(v) = version else { return false };
        let changed = matches!(&self.installed, Some(prev) if *prev != v);
        self.installed = Some(v);
        changed
    }

    pub fn check_done(&mut self, now: u64) {
        self.last_check = Some(now);
    }
}

/// Fold a check's answer into the facts: 0 newer and 10 current answer, 11 offline keeps the
/// last known newest, anything else is not an answer.
pub fn apply_check(rein: &mut ReinFacts, c: &ReleaseCheck, wall: u64) {
    match c.code {
        Some(0 | 10) => {
            rein.newest = c.newest.clone();
            if rein.installed.is_none() {
                rein.installed = c.installed.clone();
            }
            rein.checked_at = Some(wall);
            rein.offline = false;
        }
        Some(11) => rein.offline = true,
        _ => {}
    }
}

/// Fold a parsed document into the window's facts; the document to install when it is
/// offered in place. Nothing newer clears the offer.
pub fn apply_doc(app: &mut AppFacts, doc: &Doc, baked_key_id: Option<&str>) -> Option<Doc> {
    let o = app_update::offer(doc, app.running, baked_key_id);
    let (mode, command) = app_update::mode(o);
    app.mode = mode;
    app.command = command.map(Into::into);
    if o == Offer::Nothing {
        app.newest = None;
        app.notes = None;
        return None;
    }
    app.newest = Some(doc.version.to_string());
    app.notes = Some(doc.notes.clone());
    (mode == "in_place").then(|| doc.clone())
}

/// What one step needs from the window: the host, the CLI runner and the web side.
pub trait Io {
    fn busy(&self) -> bool;
    fn version(&self) -> Option<Vec<u8>>;
    fn check(&self) -> Result<ReleaseCheck, IpcError>;
    fn emit(&self, state: &UpdatesState);
    /// The window's document (`desktop-latest.json`) and its `.minisig`, or `None` when there
    /// is no answer (offline, no release yet): the last known offer stands.
    fn app_doc(&self) -> Option<(Vec<u8>, Vec<u8>)>;
    /// The verifier with the baked key, and the installer URL prefixes.
    fn app_trust(
        &self,
    ) -> (
        Box<dyn SignatureVerifier + Send + Sync>,
        &'static [&'static str],
    );
    /// The installed version changed: the install health check runs again.
    fn installed_changed(&self);
}

/// The watch: the state the web side draws and the schedule behind it.
pub struct Watch {
    state: Mutex<UpdatesState>,
    schedule: Mutex<Schedule>,
    focus: AtomicBool,
    /// The document last verified as an in-place offer: the only one `app_update_start` installs.
    doc: Mutex<Option<Doc>>,
    /// Cancel of the running download.
    cancel: AtomicBool,
}

impl Watch {
    pub fn new(asked: Asked, updated_from: Option<String>) -> Self {
        Self {
            state: Mutex::new(UpdatesState::new(asked, updated_from)),
            schedule: Mutex::new(Schedule::default()),
            focus: AtomicBool::new(false),
            doc: Mutex::new(None),
            cancel: AtomicBool::new(false),
        }
    }

    /// The window's document: verify its signature, parse it and fold it in. A document whose
    /// signature does not verify with the baked key, or that does not parse, changes nothing.
    pub fn app_checked(
        &self,
        bytes: &[u8],
        sig: &[u8],
        verifier: &dyn SignatureVerifier,
        prefixes: &[&str],
    ) -> Option<UpdatesState> {
        let doc = app_update::verified_doc(bytes, sig, verifier, prefixes).ok()?;
        let baked = verifier.key_id().ok();
        let mut keep = None;
        let s = self.update(|s| keep = apply_doc(&mut s.app, &doc, baked.as_deref()));
        *self.doc.lock().unwrap_or_else(|e| e.into_inner()) = keep;
        s
    }

    /// One check of the window's document, unless `[updates] app = false` or a job runs.
    fn app_check(&self, cfg: &Updates, io: &dyn Io) -> Option<UpdatesState> {
        if !cfg.app || self.state().job.running() {
            return None;
        }
        let (bytes, sig) = io.app_doc()?;
        let (verifier, prefixes) = io.app_trust();
        self.app_checked(&bytes, &sig, verifier.as_ref(), prefixes)
    }

    pub fn state(&self) -> UpdatesState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The window gained focus: the next tick reads, and checks if the last check is old.
    pub fn focused(&self) {
        self.focus.store(true, Ordering::Release);
    }

    /// Change the state; the new state when it differs.
    pub fn update(&self, f: impl FnOnce(&mut UpdatesState)) -> Option<UpdatesState> {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let before = s.clone();
        f(&mut s);
        (*s != before).then(|| s.clone())
    }

    /// One tick at `now` (monotonic seconds) and `wall` (Unix seconds).
    pub fn step(&self, now: u64, wall: u64, cfg: &Updates, period: u64, io: &dyn Io) {
        let focus = self.focus.swap(false, Ordering::AcqRel);
        let due = self.schedule.lock().unwrap_or_else(|e| e.into_inner()).due(
            now,
            cfg,
            period,
            focus,
            io.busy(),
        );
        if due.read {
            let v = io.version().map(|b| parse_version(&b));
            let changed = self
                .schedule
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .read_done(now, v.clone());
            if let Some(v) = v
                && let Some(s) = self.update(|s| s.rein.installed = v)
            {
                io.emit(&s);
            }
            if changed {
                io.installed_changed();
            }
        }
        if due.check {
            match io.check() {
                Ok(c) => {
                    self.schedule
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .check_done(now);
                    if let Some(s) = self.update(|s| apply_check(&mut s.rein, &c, wall)) {
                        io.emit(&s);
                    }
                }
                // A run began between the tick's look and the check: due again next tick.
                Err(e) if e.kind == "busy" => return,
                Err(_) => self
                    .schedule
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .check_done(now),
            }
            // The window's own document rides the same schedule (component 3 "Check").
            if let Some(s) = self.app_check(cfg, io) {
                io.emit(&s);
            }
        }
    }

    /// A check's answer from elsewhere (the Update dialog, Check now) counts as a check.
    pub fn checked(&self, c: &ReleaseCheck, app: &AppHandle) {
        if let Some(s) = self.update(|s| apply_check(&mut s.rein, c, wall_now())) {
            let _ = app.emit(EVENT_UPDATES, s);
        }
    }
}

fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The window's side of [`Io`].
struct AppIo<'a> {
    app: &'a AppHandle,
}

impl Io for AppIo<'_> {
    fn busy(&self) -> bool {
        self.app.state::<CliState>().busy.busy()
    }

    fn version(&self) -> Option<Vec<u8>> {
        self.app.state::<Desktop>().platform.rein_version()
    }

    fn check(&self) -> Result<ReleaseCheck, IpcError> {
        let desktop = self.app.state::<Desktop>();
        let run = self
            .app
            .state::<CliState>()
            .collect(desktop.platform.as_ref(), &CliAction::UpdateCheck)?;
        Ok(crate::cli::release_check_from(run))
    }

    fn emit(&self, state: &UpdatesState) {
        let _ = self.app.emit(EVENT_UPDATES, state.clone());
    }

    fn app_doc(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        let (url, policy, _) = app_update::endpoint();
        let http =
            rein_release::ReqwestHttp::with_timeout(policy, app_update::CHECK_TIMEOUT).ok()?;
        app_update::fetch_doc(&http, &url).ok()
    }

    fn app_trust(
        &self,
    ) -> (
        Box<dyn SignatureVerifier + Send + Sync>,
        &'static [&'static str],
    ) {
        let (_, _, prefixes) = app_update::endpoint();
        (app_update::verifier(), prefixes)
    }

    fn installed_changed(&self) {
        let desktop = self.app.state::<Desktop>();
        let (view, fresh) = self
            .app
            .state::<CliState>()
            .health(desktop.platform.as_ref(), true);
        if fresh {
            let _ = self.app.emit(EVENT_HEALTH_CHANGED, view);
        }
    }
}

/// The window's download directory, when the app has a cache directory.
fn update_dir() -> Option<std::path::PathBuf> {
    crate::dirs::cache_dir(&crate::dirs::process_env).map(|c| app_update::update_dir(&c))
}

/// The watch's thread: one step per second for the life of the window. An installer a previous
/// run left is deleted first (updates.md §Security).
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        if let Some(dir) = update_dir() {
            app_update::clean(&dir);
        }
        let began = std::time::Instant::now();
        let period = if app.state::<Desktop>().platform.in_wsl() {
            WSL_PERIOD_S
        } else {
            LOCAL_PERIOD_S
        };
        loop {
            let cfg = app
                .state::<std::sync::Arc<ConfigService>>()
                .view()
                .config
                .updates;
            app.state::<Watch>().step(
                began.elapsed().as_secs(),
                wall_now(),
                &cfg,
                period,
                &AppIo { app: &app },
            );
            std::thread::sleep(TICK);
        }
    });
}

/// `updates_get()`: the state, for a web view that has just loaded.
#[tauri::command]
pub fn updates_get(watch: State<'_, Watch>) -> UpdatesState {
    watch.state()
}

/// `updates_check()`: Check now; refused while a check or another run is in progress.
#[tauri::command(async)]
pub fn updates_check(app: AppHandle, watch: State<'_, Watch>) -> Result<UpdatesState, IpcError> {
    let io = AppIo { app: &app };
    let c = io.check()?;
    watch.checked(&c, &app);
    let cfg = app
        .state::<std::sync::Arc<ConfigService>>()
        .view()
        .config
        .updates;
    if let Some(s) = watch.app_check(&cfg, &io) {
        let _ = app.emit(EVENT_UPDATES, s);
    }
    Ok(watch.state())
}

/// Set the job and tell the web side.
fn set_job(app: &AppHandle, watch: &Watch, job: Job) {
    if let Some(s) = watch.update(|s| s.job = job) {
        let _ = app.emit(EVENT_UPDATES, s);
    }
}

/// `app_update_start()` (updates.md §API, component 3 "Install"): on the user's click only. No
/// argument: the core installs only the document it last verified as an in-place offer.
/// Download → signature → sha256 → the ordinary quit → the installer, detached → exit. A
/// failed step answers its fixed sentence; Cancel answers the state, back to idle.
#[tauri::command(async)]
pub fn app_update_start(app: AppHandle, watch: State<'_, Watch>) -> Result<UpdatesState, IpcError> {
    let cfg = app
        .state::<std::sync::Arc<ConfigService>>()
        .view()
        .config
        .updates;
    let doc = watch.doc.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(doc) = doc.filter(|_| cfg.app) else {
        return Err(err("no_update", "There is no update to install."));
    };
    if app.state::<CliState>().busy.busy() {
        return Err(err("busy", "Wait for the running command to finish."));
    }
    {
        // Claim the job under the state's lock: a second click is refused.
        let mut s = watch.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.job.running() {
            return Err(err("busy", "The update is already running."));
        }
        s.job = Job::Downloading {
            done: 0,
            total: doc.size,
        };
    }
    watch.cancel.store(false, Ordering::Release);
    let Some(dir) = update_dir() else {
        set_job(&app, &watch, Job::Failed(app_update::Step::Fetch));
        let step = app_update::Step::Fetch;
        return Err(err(step.kind(), step.sentence()));
    };
    let (_, policy, _) = app_update::endpoint();
    let verifier = app_update::verifier();
    let run = format!("{}-{}", std::process::id(), wall_now());
    let outcome =
        match rein_release::ReqwestHttp::with_timeout(policy, app_update::DOWNLOAD_TIMEOUT) {
            Err(_) => Err(app_update::Step::Fetch),
            Ok(http) => app_update::install(
                &doc,
                &app_update::Ctx {
                    http: &http,
                    verifier: verifier.as_ref(),
                    dir: &dir,
                    run: &run,
                    cancel: &watch.cancel,
                },
                &mut |job| set_job(&app, &watch, job),
                // The ordinary quit (D-066 addendum 1 r1): the version this window had, so the next
                // one says "Updated to"; every tab detached and its pty reaped. The sessions run on.
                &mut || {
                    let _ = app
                        .state::<UiStore>()
                        .set_updated_from(env!("CARGO_PKG_VERSION"));
                    app.state::<Desktop>().detach_all();
                },
                &mut app_update::start_installer,
            ),
        };
    match outcome {
        // No work after the start: exit now.
        Ok(app_update::Outcome::Started) => {
            app.exit(0);
            Ok(watch.state())
        }
        Ok(app_update::Outcome::Cancelled) => {
            set_job(&app, &watch, Job::Idle);
            Ok(watch.state())
        }
        Err(step) => {
            set_job(&app, &watch, Job::Failed(step));
            Err(err(step.kind(), step.sentence()))
        }
    }
}

/// `app_update_cancel()`: stop the running download; the file is deleted, nothing starts. Once
/// the download is complete there is nothing to cancel.
#[tauri::command]
pub fn app_update_cancel(watch: State<'_, Watch>) -> UpdatesState {
    if matches!(watch.state().job, Job::Downloading { .. }) {
        watch.cancel.store(true, Ordering::Release);
    }
    watch.state()
}

/// `updates_later(which)`: `rein` | `app` — the known newest version does not ask again.
#[tauri::command]
pub fn updates_later(
    app: AppHandle,
    watch: State<'_, Watch>,
    ui: State<'_, UiStore>,
    which: String,
) -> Result<UpdatesState, IpcError> {
    let mut asked = watch.state().asked;
    let s = watch.state();
    match which.as_str() {
        "rein" => asked.rein = s.rein.newest,
        "app" => asked.app = s.app.newest,
        _ => return Err(err("bad_kind", "rein or app")),
    }
    ui.set_asked(asked.clone())?;
    if let Some(s) = watch.update(|s| s.asked = asked) {
        let _ = app.emit(EVENT_UPDATES, s);
    }
    Ok(watch.state())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn cfg() -> Updates {
        Updates::default()
    }

    #[test]
    fn a_version_is_one_short_line_of_safe_bytes() {
        assert_eq!(parse_version(b"0.5.0\n"), Some("0.5.0".into()));
        assert_eq!(
            parse_version(b"0.6.0-rc.1+g12\r\n"),
            Some("0.6.0-rc.1+g12".into())
        );
        assert_eq!(parse_version(b"1.0.0"), Some("1.0.0".into()));
        for bad in [
            &b""[..],
            b"\n0.5.0",
            b"0.5.0 beta",
            b"0.5.0;rm",
            b"\x1b[31m0.5",
            &[b'1'; 65],
        ] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
        assert_eq!(parse_version(&[b'1'; 64]).map(|v| v.len()), Some(64));
    }

    /// The schedule: start delay, interval, focus rule, nothing during a run, a read at its end.
    #[test]
    fn the_schedule_follows_the_clock() {
        let mut s = Schedule::default();
        let c = cfg();
        // Start: the first read at once, no check before 30 s.
        assert_eq!(
            s.due(0, &c, 5, false, false),
            Due {
                read: true,
                check: false
            }
        );
        s.read_done(0, Some(Some("0.5.0".into())));
        assert_eq!(s.due(4, &c, 5, false, false), Due::default());
        assert_eq!(
            s.due(5, &c, 5, false, false),
            Due {
                read: true,
                check: false
            }
        );
        s.read_done(5, Some(Some("0.5.0".into())));
        assert!(!s.due(29, &c, 5, false, false).check);
        assert!(s.due(30, &c, 5, false, false).check);
        s.check_done(30);
        // The interval: 30 minutes after the last check, not before.
        assert!(!s.due(30 + 1799, &c, 5, false, false).check);
        assert!(s.due(30 + 1800, &c, 5, false, false).check);
        s.check_done(1830);
        // Focus: a read at once; a check only when the last one is 10 minutes old.
        assert_eq!(
            s.due(1831, &c, 30, true, false),
            Due {
                read: true,
                check: false
            }
        );
        s.read_done(1831, Some(Some("0.5.0".into())));
        assert_eq!(
            s.due(1830 + 600, &c, 30, true, false),
            Due {
                read: true,
                check: true
            }
        );
        s.check_done(2430);
        s.read_done(2430, Some(Some("0.5.0".into())));
        // During a run nothing is due, even when it would be; its end is one read.
        assert_eq!(s.due(4300, &c, 30, true, true), Due::default());
        assert_eq!(s.due(4301, &c, 30, false, true), Due::default());
        let end = s.due(4302, &c, 30, false, false);
        assert!(end.read && end.check, "{end:?}");
        // check = false: nothing at all.
        let off = Updates {
            check: false,
            ..cfg()
        };
        assert_eq!(
            Schedule::default().due(10_000, &off, 5, true, false),
            Due::default()
        );
        // interval_min is read each tick.
        let mut s = Schedule::default();
        s.check_done(100);
        let ten = Updates {
            interval_min: 10,
            ..cfg()
        };
        assert!(s.due(700, &ten, 5, false, false).check);
    }

    #[test]
    fn a_change_is_seen_once_and_never_on_the_first_read_or_a_failed_one() {
        let mut s = Schedule::default();
        assert!(!s.read_done(0, Some(Some("0.5.0".into()))));
        assert!(!s.read_done(5, Some(Some("0.5.0".into()))));
        assert!(
            !s.read_done(10, None),
            "a read that did not answer is no change"
        );
        assert!(s.read_done(15, Some(Some("0.5.1".into()))));
        assert!(!s.read_done(20, Some(Some("0.5.1".into()))));
        assert!(
            s.read_done(25, Some(None)),
            "a garbled file is a change to unknown"
        );
    }

    #[test]
    fn a_check_answers_with_0_or_10_and_offline_keeps_the_last_newest() {
        let check = |code, newest: Option<&str>| ReleaseCheck {
            installed: Some("0.5.0".into()),
            newest: newest.map(Into::into),
            lines: vec![],
            code: Some(code),
        };
        let mut r = ReinFacts::default();
        apply_check(&mut r, &check(0, Some("0.5.1")), 7);
        assert_eq!(
            r,
            ReinFacts {
                installed: Some("0.5.0".into()),
                newest: Some("0.5.1".into()),
                checked_at: Some(7),
                offline: false
            }
        );
        apply_check(&mut r, &check(11, None), 8);
        assert_eq!(
            (r.newest.as_deref(), r.offline, r.checked_at),
            (Some("0.5.1"), true, Some(7))
        );
        apply_check(&mut r, &check(2, None), 9);
        assert_eq!(r.newest.as_deref(), Some("0.5.1"));
        apply_check(&mut r, &check(10, Some("0.5.1")), 10);
        assert_eq!((r.offline, r.checked_at), (false, Some(10)));
    }

    #[derive(Default)]
    struct Fake {
        busy: Cell<bool>,
        version: RefCell<Option<Vec<u8>>>,
        newest: RefCell<Option<&'static str>>,
        checks: Cell<u32>,
        emitted: RefCell<Vec<UpdatesState>>,
        health: Cell<u32>,
        doc: RefCell<Option<Vec<u8>>>,
        key_id: RefCell<Option<String>>,
        app_docs: Cell<u32>,
    }

    impl Io for Fake {
        fn busy(&self) -> bool {
            self.busy.get()
        }
        fn version(&self) -> Option<Vec<u8>> {
            self.version.borrow().clone()
        }
        fn check(&self) -> Result<ReleaseCheck, IpcError> {
            self.checks.set(self.checks.get() + 1);
            Ok(ReleaseCheck {
                installed: None,
                newest: self.newest.borrow().map(Into::into),
                lines: vec![],
                code: Some(0),
            })
        }
        fn emit(&self, state: &UpdatesState) {
            self.emitted.borrow_mut().push(state.clone());
        }
        fn installed_changed(&self) {
            self.health.set(self.health.get() + 1);
        }
        fn app_doc(&self) -> Option<(Vec<u8>, Vec<u8>)> {
            self.app_docs.set(self.app_docs.get() + 1);
            self.doc.borrow().clone().map(|d| (d, b"signed".to_vec()))
        }
        fn app_trust(
            &self,
        ) -> (
            Box<dyn SignatureVerifier + Send + Sync>,
            &'static [&'static str],
        ) {
            (
                Box::new(Signed(self.key_id.borrow().clone().unwrap_or_default())),
                crate::app_update::ASSET_PREFIXES,
            )
        }
    }

    /// A verifier for one key id: a signature is good when it reads `signed`.
    struct Signed(String);

    impl SignatureVerifier for Signed {
        fn key_id(&self) -> Result<String, String> {
            Ok(self.0.clone())
        }
        fn verify(&self, _data: &mut dyn std::io::Read, signature: &str) -> Result<(), String> {
            (signature == "signed")
                .then_some(())
                .ok_or("bad signature".into())
        }
    }

    /// A changed VERSION under a running window: one event with the new version and one health
    /// check, however many reads follow; no check while a run holds the runner.
    #[test]
    fn a_version_change_is_told_once_and_rechecks_health_once() {
        let w = Watch::new(Asked::default(), None);
        let io = Fake::default();
        *io.version.borrow_mut() = Some(b"0.5.0\n".to_vec());
        *io.newest.borrow_mut() = Some("0.5.1");
        let c = cfg();
        for t in 0..30 {
            w.step(t, 1000 + t, &c, 5, &io);
        }
        assert_eq!(
            io.emitted.borrow().len(),
            1,
            "the first read tells the version"
        );
        assert_eq!(io.health.get(), 0);
        *io.version.borrow_mut() = Some(b"0.5.1\n".to_vec());
        for t in 30..60 {
            w.step(t, 1000 + t, &c, 5, &io);
        }
        assert_eq!(io.health.get(), 1);
        assert_eq!(io.checks.get(), 1, "the start check, once");
        let last = w.state();
        assert_eq!(last.rein.installed.as_deref(), Some("0.5.1"));
        assert_eq!(last.rein.newest.as_deref(), Some("0.5.1"));
        assert_eq!(last.rein.checked_at, Some(1030));
        // A run in progress: no read, no check; focus waits for it.
        io.busy.set(true);
        w.focused();
        *io.version.borrow_mut() = Some(b"0.5.2\n".to_vec());
        for t in 60..3000 {
            w.step(t, 1000 + t, &c, 5, &io);
        }
        assert_eq!((io.health.get(), io.checks.get()), (1, 1));
        io.busy.set(false);
        w.step(3000, 4000, &c, 5, &io);
        assert_eq!((io.health.get(), io.checks.get()), (2, 2));
    }

    #[test]
    fn later_is_kept_in_the_state() {
        let w = Watch::new(
            Asked {
                rein: Some("0.5.1".into()),
                app: None,
            },
            None,
        );
        assert_eq!(w.state().asked.rein.as_deref(), Some("0.5.1"));
        assert!(w.update(|s| s.asked.rein = Some("0.5.1".into())).is_none());
        assert!(w.update(|s| s.asked.rein = Some("0.5.2".into())).is_some());
    }

    fn app_doc_json(version: &str, key_id: &str) -> Vec<u8> {
        serde_json::json!({
            "schema": 1,
            "version": version,
            "key_id": key_id,
            "notes": "Notes.",
            "windows_x64": {
                "url": "https://github.com/Vimurai/rein-app/releases/download/v9/x.exe",
                "size": 10,
                "sha256": "0".repeat(64),
                "sig": "untrusted comment: x\nRUQ=\n",
            }
        })
        .to_string()
        .into_bytes()
    }

    /// E-86: the window's document rides the Rein check's schedule; a newer version for the
    /// baked key is offered in place (on Windows) and kept for `app_update_start`; a key
    /// mismatch is offered by hand and keeps nothing; `[updates] app = false` fetches nothing.
    #[test]
    fn the_window_document_rides_the_check_and_offers_only_a_newer_version() {
        const P: &[&str] = crate::app_update::ASSET_PREFIXES;
        let key = "0123456789ABCDEF";
        let w = Watch::new(Asked::default(), Some("0.0.9".into()));
        assert_eq!(w.state().app.updated_from.as_deref(), Some("0.0.9"));
        let io = Fake::default();
        *io.version.borrow_mut() = Some(b"0.5.0\n".to_vec());
        *io.doc.borrow_mut() = Some(app_doc_json("99.0.0", key));
        *io.key_id.borrow_mut() = Some(key.into());
        for t in 0..31 {
            w.step(t, 1000 + t, &cfg(), 5, &io);
        }
        assert_eq!(io.app_docs.get(), 1, "once, with the start check");
        let s = w.state();
        assert_eq!(s.app.newest.as_deref(), Some("99.0.0"));
        assert_eq!(s.app.notes.as_deref(), Some("Notes."));
        let in_place = crate::app_update::in_place_here();
        assert_eq!(s.app.mode, if in_place { "in_place" } else { "command" });
        assert_eq!(s.app.command.is_some(), !in_place);
        assert_eq!(w.doc.lock().unwrap().is_some(), in_place);
        // Another key: by hand on Windows, nothing kept to install.
        let other = Signed("FEDCBA9876543210".into());
        let changed = w.app_checked(&app_doc_json("99.0.1", key), b"signed", &other, P);
        assert!(changed.is_some());
        assert!(w.doc.lock().unwrap().is_none());
        assert_eq!(
            w.state().app.mode,
            if in_place { "by_hand" } else { "command" }
        );
        // Not newer: nothing offered. Garbage: nothing changes.
        let baked = Signed(key.into());
        w.app_checked(
            &app_doc_json(env!("CARGO_PKG_VERSION"), key),
            b"signed",
            &baked,
            P,
        );
        assert_eq!((w.state().app.newest, w.state().app.mode), (None, "none"));
        assert!(w.app_checked(b"{}", b"signed", &baked, P).is_none());
        // A document whose own signature does not verify is no offer at all.
        assert!(
            w.app_checked(&app_doc_json("99.0.2", key), b"forged", &baked, P)
                .is_none()
        );
        assert_eq!(w.state().app.newest, None);
        // app = false: the Rein check runs, the window's document is never fetched.
        let off = Updates {
            app: false,
            ..cfg()
        };
        let w = Watch::new(Asked::default(), None);
        let io2 = Fake::default();
        *io2.doc.borrow_mut() = Some(app_doc_json("99.0.0", key));
        for t in 0..31 {
            w.step(t, 1000 + t, &off, 5, &io2);
        }
        assert_eq!((io2.checks.get(), io2.app_docs.get()), (1, 0));
    }
}
