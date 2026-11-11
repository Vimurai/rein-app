//! `PtyHost` (desktop.md §Components 2, §Security 2, D-033 ruling 1): one pty per tab whose
//! child is exactly `<home>/.rein/bin/rein start`, cwd = the project directory.
//!
//! Output is read in chunks of at most [`MAX_CHUNK`] and handed to a sink (the Tauri channel).
//! Back-pressure: the web side acknowledges what it has written into xterm.js (`pty_ack`); while
//! more than [`MAX_UNACKED`] bytes are unacknowledged the reader stops reading the pty, so the
//! child blocks on its own writes instead of the window buffering without bound.
//!
//! `detach` closes the pty and sends the child's process group SIGHUP, as a closing terminal
//! does — the tmux client exits, the tmux session keeps running — and the child is always
//! reaped: killed if it outlives [`REAP_GRACE`]. Nothing is written to the pty on detach
//! (portable-pty's own writer types Enter and EOF when dropped, so it is not used).

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use filedescriptor::FileDescriptor;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Largest chunk sent to the web side at once.
pub const MAX_CHUNK: usize = 64 * 1024;
/// The reader pauses while more than this is unacknowledged.
pub const MAX_UNACKED: u64 = 4 * 1024 * 1024;
/// How long a detached child may take to exit on its own before it is killed.
pub const REAP_GRACE: Duration = Duration::from_secs(2);
/// How long the exit report waits for the child's remaining output.
const DRAIN_WAIT: Duration = Duration::from_secs(1);

/// Variables removed from the child's environment. `CLAUDECODE` marks a Claude Code
/// session (desktop.md §Components 2); `TMUX`/`TMUX_PANE` would make `rein start` run
/// `tmux switch-client` on an outer client instead of attaching inside this pty.
pub const ENV_REMOVED: &[&str] = &["CLAUDECODE", "TMUX", "TMUX_PANE"];
/// Variables set in the child's environment.
pub const ENV_SET: &[(&str, &str)] = &[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

/// What runs in a pty. Built only by [`ChildSpec::rein_start`] in production; tests build
/// stub children. No IPC input ever becomes a program or an argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
}

impl ChildSpec {
    /// `<home>/.rein/bin/rein start` in `project` — the only child the window starts here.
    pub fn rein_start(home: &Path, project: &Path) -> Self {
        Self {
            program: rein_bin(home),
            args: vec!["start".into()],
            cwd: project.to_path_buf(),
        }
    }

    fn command(&self) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(&self.program);
        cmd.args(&self.args);
        // The project path is the cwd, never part of a command line (§Security 2).
        cmd.cwd(&self.cwd);
        for k in ENV_REMOVED {
            cmd.env_remove(k);
        }
        for (k, v) in ENV_SET {
            cmd.env(k, v);
        }
        cmd
    }
}

/// The installed `rein` launcher.
pub fn rein_bin(home: &Path) -> PathBuf {
    home.join(".rein/bin/rein")
}

pub type PtyId = u32;

/// Pty size in cells (validated by the caller: cols 2–500, rows 2–200).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl From<Size> for PtySize {
    fn from(s: Size) -> Self {
        PtySize {
            rows: s.rows,
            cols: s.cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PtyError {
    UnknownPty,
    /// `pty_ack` for more bytes than are outstanding (desktop.md §API).
    AckTooLarge,
    Spawn(String),
    Io(String),
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPty => f.write_str("unknown pty"),
            Self::AckTooLarge => f.write_str("acknowledged more output than was sent"),
            Self::Spawn(e) => write!(f, "cannot start the terminal: {e}"),
            Self::Io(e) => write!(f, "terminal i/o: {e}"),
        }
    }
}

/// Counts bytes sent and acknowledged; the reader waits while too much is in flight.
#[derive(Default)]
struct Flow {
    unacked: Mutex<u64>,
    cv: Condvar,
    closed: AtomicBool,
}

impl Flow {
    /// Block while more than [`MAX_UNACKED`] bytes are unacknowledged (or until closed).
    fn wait_for_room(&self) {
        let mut n = self.unacked.lock().unwrap_or_else(|e| e.into_inner());
        while *n > MAX_UNACKED && !self.closed.load(Ordering::Acquire) {
            n = self.cv.wait(n).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn sent(&self, bytes: usize) {
        *self.unacked.lock().unwrap_or_else(|e| e.into_inner()) += bytes as u64;
    }

    /// Acknowledge `bytes`; more than is outstanding is refused and changes nothing.
    fn ack(&self, bytes: u64) -> Result<(), PtyError> {
        let mut n = self.unacked.lock().unwrap_or_else(|e| e.into_inner());
        if bytes > *n {
            return Err(PtyError::AckTooLarge);
        }
        *n -= bytes;
        self.cv.notify_all();
        Ok(())
    }

    #[cfg(test)]
    fn unacked(&self) -> u64 {
        *self.unacked.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.cv.notify_all();
    }
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    /// Its own lock: a write that blocks (a child not reading its input) must not hold the
    /// session map, or `pty_ack` / `tab_detach` on every tab would wait behind it.
    writer: Arc<Mutex<FileDescriptor>>,
    /// The child's pid, which is also its process group: portable-pty makes it a session
    /// leader. `None` if the platform did not report one.
    pgid: Option<Pid>,
    exited: Arc<AtomicBool>,
    flow: Arc<Flow>,
}

/// Where a session's output and exit go.
pub trait Events: Send + Sync + 'static {
    /// One chunk of pty output (at most [`MAX_CHUNK`] bytes).
    fn output(&self, id: PtyId, bytes: Vec<u8>);
    /// The child exited (after reaping) with this code.
    fn exit(&self, id: PtyId, code: u32);
}

#[derive(Default)]
pub struct PtyHost {
    next: AtomicU32,
    sessions: Mutex<HashMap<PtyId, Session>>,
}

impl PtyHost {
    /// Open a pty of `size`, start `spec` in it, and stream its output to `events`.
    pub fn open(
        &self,
        spec: &ChildSpec,
        size: Size,
        events: Arc<dyn Events>,
    ) -> Result<PtyId, PtyError> {
        let pair = native_pty_system()
            .openpty(size.into())
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        // Reader and writer first: once the child is spawned nothing may fail before the
        // waiter thread owns it, or the child would be left unreaped.
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Io(e.to_string()))?;
        let writer = master_writer(&*pair.master)?;
        let mut child = pair
            .slave
            .spawn_command(spec.command())
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        drop(pair.slave); // only the child holds the slave side
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let flow = Arc::new(Flow::default());
        let exited = Arc::new(AtomicBool::new(false));
        let pgid = child
            .process_id()
            .and_then(|p| i32::try_from(p).ok())
            .map(Pid::from_raw);

        let drained = spawn_reader(id, reader, Arc::clone(&flow), Arc::clone(&events));
        {
            let exited = Arc::clone(&exited);
            let flow = Arc::clone(&flow);
            std::thread::spawn(move || {
                // Reaps the child; the only place `wait` is called.
                let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
                exited.store(true, Ordering::Release);
                // Report the exit after the child's last output, so it reads last. A
                // grandchild may keep the pty open: wait for the reader only so long.
                let _ = drained.recv_timeout(DRAIN_WAIT);
                flow.close();
                events.exit(id, code);
            });
        }
        self.lock().insert(
            id,
            Session {
                master: pair.master,
                writer: Arc::new(Mutex::new(writer)),
                pgid,
                exited,
                flow,
            },
        );
        Ok(id)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<PtyId, Session>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn write(&self, id: PtyId, bytes: &[u8]) -> Result<(), PtyError> {
        // Take the writer out of the map before writing; the map lock is released here.
        let writer = Arc::clone(&self.lock().get(&id).ok_or(PtyError::UnknownPty)?.writer);
        let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
        w.write_all(bytes)
            .and_then(|()| w.flush())
            .map_err(|e| PtyError::Io(e.to_string()))
    }

    pub fn resize(&self, id: PtyId, size: Size) -> Result<(), PtyError> {
        let s = self.lock();
        let s = s.get(&id).ok_or(PtyError::UnknownPty)?;
        s.master
            .resize(size.into())
            .map_err(|e| PtyError::Io(e.to_string()))
    }

    /// The web side has written `bytes` of this pty's output into the terminal.
    pub fn ack(&self, id: PtyId, bytes: u32) -> Result<(), PtyError> {
        let flow = Arc::clone(&self.lock().get(&id).ok_or(PtyError::UnknownPty)?.flow);
        flow.ack(u64::from(bytes))
    }

    /// Bytes sent to the web side and not yet acknowledged.
    #[cfg(test)]
    pub fn unacked(&self, id: PtyId) -> Result<u64, PtyError> {
        let s = self.lock();
        Ok(s.get(&id).ok_or(PtyError::UnknownPty)?.flow.unacked())
    }

    /// Close the pty master (the tmux client detaches; its session keeps running) and make
    /// sure the child is reaped: a child still alive after [`REAP_GRACE`] is killed.
    pub fn detach(&self, id: PtyId) -> Result<(), PtyError> {
        let reaper = close(self.lock().remove(&id).ok_or(PtyError::UnknownPty)?);
        std::thread::spawn(move || reap(&[reaper], REAP_GRACE));
        Ok(())
    }

    /// Detach every pty and wait (at most [`REAP_GRACE`]) for the children, killing what is
    /// left — for the window closing, when no background thread would outlive the process.
    pub fn detach_all(&self) {
        let reapers: Vec<Reaper> = self.lock().drain().map(|(_, s)| close(s)).collect();
        reap(&reapers, REAP_GRACE);
    }

    /// Whether `id` is an open pty.
    #[cfg(test)]
    pub fn contains(&self, id: PtyId) -> bool {
        self.lock().contains_key(&id)
    }
}

/// A borrowed master fd, so [`FileDescriptor::dup`] can duplicate it without `unsafe`.
struct Raw(RawFd);

impl AsRawFd for Raw {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

/// Our own duplicate of the master fd for writing. Dropping it only closes it — unlike
/// portable-pty's `take_writer`, whose drop writes a newline and the EOF character, which
/// tmux would pass to the user's pane on every detach.
fn master_writer(master: &dyn MasterPty) -> Result<FileDescriptor, PtyError> {
    let fd = master
        .as_raw_fd()
        .ok_or_else(|| PtyError::Io("the pty has no file descriptor".into()))?;
    FileDescriptor::dup(&Raw(fd)).map_err(|e| PtyError::Io(e.to_string()))
}

/// A host that goes away takes its children with it: every pty is detached and every child
/// reaped. The window calls `detach_all` on exit already (this is then a no-op); it matters
/// when a host is dropped early, such as a test that fails before its own cleanup.
impl Drop for PtyHost {
    fn drop(&mut self) {
        self.detach_all();
    }
}

/// What is left of a session after its pty is closed: enough to make sure the child ends.
struct Reaper {
    pgid: Option<Pid>,
    exited: Arc<AtomicBool>,
}

/// Close a session's pty: the master and the writer go (a write blocked on it fails once the
/// child is gone), the reader stops waiting for acks, and the child's group gets SIGHUP.
/// The reader's own copy of the master keeps the pty open until the child exits, so closing
/// the master alone is no hangup: the signal is what a closing terminal would deliver.
fn close(s: Session) -> Reaper {
    let Session {
        master,
        writer,
        pgid,
        exited,
        flow,
    } = s;
    drop(writer);
    drop(master);
    flow.close();
    if !exited.load(Ordering::Acquire)
        && let Some(pg) = pgid
    {
        let _ = killpg(pg, Signal::SIGHUP);
    }
    Reaper { pgid, exited }
}

/// Wait up to `grace` for the children to exit on the hangup, then SIGKILL the rest.
///
/// Residual risk (security review of E-35, P3): between the waiter reaping a child and
/// setting `exited` there is a window of a few instructions in which a reused pid could be
/// signalled. Closing it needs `waitid(WNOWAIT)`, which nix does not offer on every target.
fn reap(reapers: &[Reaper], grace: Duration) {
    let alive = || reapers.iter().any(|r| !r.exited.load(Ordering::Acquire));
    let step = Duration::from_millis(50);
    let mut waited = Duration::ZERO;
    while alive() && waited < grace {
        std::thread::sleep(step);
        waited += step;
    }
    for r in reapers {
        // portable-pty's own killer sends only SIGHUP, which a child may ignore. SIGKILL to
        // the group also ends the `tmux attach` client under `rein start`; the tmux server
        // runs in its own session and is untouched.
        if !r.exited.load(Ordering::Acquire)
            && let Some(pg) = r.pgid
        {
            let _ = killpg(pg, Signal::SIGKILL); // the waiter thread then reaps it
        }
    }
}

/// Read the pty until EOF; the returned receiver gets one message when reading ends.
fn spawn_reader(
    id: PtyId,
    mut reader: Box<dyn Read + Send>,
    flow: Arc<Flow>,
    events: Arc<dyn Events>,
) -> std::sync::mpsc::Receiver<()> {
    let (done, drained) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; MAX_CHUNK];
        loop {
            flow.wait_for_room();
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break, // EOF / EIO: the child side closed
                Ok(n) => {
                    flow.sent(n);
                    events.output(id, buf[..n].to_vec());
                }
            }
        }
        let _ = done.send(());
    });
    drained
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;

    #[derive(Default)]
    struct Capture {
        out: Mutex<Vec<u8>>,
        chunks: Mutex<Vec<usize>>,
        exit: Mutex<Option<mpsc::Sender<u32>>>,
    }

    impl Events for Capture {
        fn output(&self, _: PtyId, bytes: Vec<u8>) {
            self.chunks.lock().unwrap().push(bytes.len());
            self.out.lock().unwrap().extend(bytes);
        }
        fn exit(&self, _: PtyId, code: u32) {
            if let Some(tx) = self.exit.lock().unwrap().take() {
                let _ = tx.send(code);
            }
        }
    }

    fn capture() -> (Arc<Capture>, mpsc::Receiver<u32>) {
        let (tx, rx) = mpsc::channel();
        let c = Arc::new(Capture::default());
        *c.exit.lock().unwrap() = Some(tx);
        (c, rx)
    }

    fn sh(script: &str) -> ChildSpec {
        ChildSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            cwd: std::env::temp_dir(),
        }
    }

    const SIZE: Size = Size { cols: 80, rows: 24 };

    fn wait_for(c: &Capture, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if String::from_utf8_lossy(&c.out.lock().unwrap()).contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn the_production_child_is_exactly_rein_start_in_the_project() {
        let spec = ChildSpec::rein_start(Path::new("/home/u"), Path::new("/home/u/Dev/p"));
        assert_eq!(spec.program, PathBuf::from("/home/u/.rein/bin/rein"));
        assert_eq!(spec.args, vec![OsString::from("start")]);
        assert_eq!(spec.cwd, PathBuf::from("/home/u/Dev/p"));
        let argv = spec.command().get_argv().clone();
        assert_eq!(
            argv,
            vec![OsString::from("/home/u/.rein/bin/rein"), "start".into()]
        );
        let cmd = spec.command();
        assert_eq!(
            cmd.get_cwd().map(PathBuf::from),
            Some(PathBuf::from("/home/u/Dev/p"))
        );
        for k in ENV_REMOVED {
            assert_eq!(cmd.get_env(k), None, "{k} must be unset");
        }
        assert_eq!(
            cmd.get_env("TERM"),
            Some(std::ffi::OsStr::new("xterm-256color"))
        );
    }

    #[test]
    fn bytes_round_trip_and_the_env_is_clean() {
        let (c, _rx) = capture();
        let host = PtyHost::default();
        let spec = sh("echo \"term=$TERM cc=${CLAUDECODE-unset} tmux=${TMUX-unset}\"; cat");
        let id = host.open(&spec, SIZE, c.clone()).unwrap();
        assert!(wait_for(&c, "term=xterm-256color cc=unset tmux=unset"));
        host.write(id, b"round-trip\n").unwrap();
        assert!(wait_for(&c, "round-trip"));
        host.detach(id).unwrap();
    }

    #[test]
    fn the_cwd_is_the_given_directory() {
        let dir = tempfile::tempdir().unwrap();
        let canon = dir.path().canonicalize().unwrap();
        let (c, rx) = capture();
        let host = PtyHost::default();
        let spec = ChildSpec {
            cwd: canon.clone(),
            ..sh("pwd -P")
        };
        host.open(&spec, SIZE, c.clone()).unwrap();
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(wait_for(&c, &canon.to_string_lossy()));
    }

    #[test]
    fn resize_reaches_the_child() {
        let (c, _rx) = capture();
        let host = PtyHost::default();
        let id = host
            .open(&sh("read x; stty size; cat"), SIZE, c.clone())
            .unwrap();
        host.resize(
            id,
            Size {
                cols: 123,
                rows: 45,
            },
        )
        .unwrap();
        host.write(id, b"\n").unwrap();
        assert!(wait_for(&c, "45 123"));
        host.detach(id).unwrap();
    }

    #[test]
    fn the_exit_code_is_reported_after_the_last_output() {
        let (c, rx) = capture();
        PtyHost::default()
            .open(&sh("printf 'last words'; exit 3"), SIZE, c.clone())
            .unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), 3);
        // Captured at the moment of the exit event: the output is already complete.
        assert!(String::from_utf8_lossy(&c.out.lock().unwrap()).contains("last words"));
    }

    #[test]
    fn detach_reaps_the_child_even_if_it_ignores_the_hangup() {
        let (c, rx) = capture();
        let host = PtyHost::default();
        let id = host
            .open(
                &sh("trap '' HUP; echo ready; while :; do sleep 1; done"),
                SIZE,
                c.clone(),
            )
            .unwrap();
        assert!(wait_for(&c, "ready"));
        let t = Instant::now();
        host.detach(id).unwrap();
        rx.recv_timeout(REAP_GRACE + Duration::from_secs(5))
            .unwrap();
        assert!(t.elapsed() < REAP_GRACE + Duration::from_secs(5));
        assert!(!host.contains(id));
        assert_eq!(host.write(id, b"x"), Err(PtyError::UnknownPty));
    }

    #[test]
    fn detach_types_nothing_into_the_pty() {
        // A raw-mode reader, as `tmux attach` is: every byte that reaches it is recorded.
        // Security review of E-35 (P1): portable-pty's writer typed "\n" + EOF on drop.
        let dir = tempfile::tempdir().unwrap();
        let got = dir.path().join("got");
        let (c, rx) = capture();
        let host = PtyHost::default();
        let script = format!(
            "stty raw -echo; trap '' HUP; echo ready; exec dd bs=1 of='{}' 2>/dev/null",
            got.display()
        );
        let id = host.open(&sh(&script), SIZE, c.clone()).unwrap();
        assert!(wait_for(&c, "ready"));
        host.detach(id).unwrap();
        rx.recv_timeout(REAP_GRACE + Duration::from_secs(5))
            .unwrap();
        let bytes = std::fs::read(&got).unwrap_or_default();
        assert!(bytes.is_empty(), "detach typed {bytes:?} into the pty");
    }

    #[test]
    fn detach_hangs_up_a_child_that_does_not_ignore_it() {
        let (c, rx) = capture();
        let host = PtyHost::default();
        let id = host
            .open(
                &sh("echo ready; while :; do sleep 1; done"),
                SIZE,
                c.clone(),
            )
            .unwrap();
        assert!(wait_for(&c, "ready"));
        let t = Instant::now();
        host.detach(id).unwrap();
        rx.recv_timeout(REAP_GRACE + Duration::from_secs(5))
            .unwrap();
        assert!(
            t.elapsed() < REAP_GRACE,
            "the child left on the hangup, not on the kill: {:?}",
            t.elapsed()
        );
    }

    #[test]
    fn a_write_blocked_on_a_child_that_does_not_read_holds_no_other_command() {
        let (c, rx) = capture();
        let host = Arc::new(PtyHost::default());
        // The child never reads its input, so the pty input buffer fills and a write blocks.
        let id = host
            .open(
                &sh("echo ready; while :; do sleep 1; done"),
                SIZE,
                c.clone(),
            )
            .unwrap();
        assert!(wait_for(&c, "ready"));
        let h = Arc::clone(&host);
        std::thread::spawn(move || {
            let _ = h.write(id, &vec![b'x'; 64 * 1024]);
        });
        std::thread::sleep(Duration::from_millis(300));
        let t = Instant::now();
        let outstanding = host.unacked(id).unwrap();
        host.ack(id, u32::try_from(outstanding).unwrap()).unwrap();
        host.resize(id, Size { cols: 90, rows: 30 }).unwrap();
        host.detach(id).unwrap();
        assert!(
            t.elapsed() < Duration::from_secs(1),
            "nothing waited on the blocked write"
        );
        rx.recv_timeout(REAP_GRACE + Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    fn detach_all_reaps_every_child_before_returning() {
        let host = PtyHost::default();
        let mut rxs = Vec::new();
        for _ in 0..3 {
            let (c, rx) = capture();
            host.open(&sh("trap '' HUP; while :; do sleep 1; done"), SIZE, c)
                .unwrap();
            rxs.push(rx);
        }
        host.detach_all();
        for rx in rxs {
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
    }

    #[test]
    fn dropping_the_host_reaps_its_children() {
        let (c, rx) = capture();
        let host = PtyHost::default();
        host.open(&sh("trap '' HUP; while :; do sleep 1; done"), SIZE, c)
            .unwrap();
        drop(host);
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    #[test]
    fn unknown_ptys_are_refused() {
        let host = PtyHost::default();
        assert_eq!(host.write(9, b"x"), Err(PtyError::UnknownPty));
        assert_eq!(host.resize(9, SIZE), Err(PtyError::UnknownPty));
        assert_eq!(host.ack(9, 1), Err(PtyError::UnknownPty));
        assert_eq!(host.detach(9), Err(PtyError::UnknownPty));
    }

    #[test]
    fn back_pressure_pauses_reading_until_acknowledged() {
        let (c, _rx) = capture();
        let host = PtyHost::default();
        // Endless output; nothing is acknowledged at first.
        let id = host
            .open(&sh("while :; do printf '%01024d' 0; done"), SIZE, c.clone())
            .unwrap();
        let total = || c.out.lock().unwrap().len() as u64;
        // Let it fill: the total settles just past the limit and stays there.
        let mut last = 0;
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(50));
            let now = total();
            if now > MAX_UNACKED && now == last {
                break;
            }
            last = now;
        }
        let stalled = total();
        assert!(stalled > MAX_UNACKED, "read past the limit: {stalled}");
        assert!(
            stalled <= MAX_UNACKED + MAX_CHUNK as u64,
            "stopped at the limit: {stalled}"
        );
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(total(), stalled, "no reading while the web side is behind");
        assert!(c.chunks.lock().unwrap().iter().all(|&n| n <= MAX_CHUNK));
        // An ack larger than what is outstanding is refused and changes nothing.
        let outstanding = host.unacked(id).unwrap();
        assert_eq!(host.ack(id, u32::MAX), Err(PtyError::AckTooLarge));
        assert_eq!(host.unacked(id).unwrap(), outstanding);
        host.ack(id, u32::try_from(outstanding).unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while total() == stalled && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(total() > stalled, "reading resumes after the ack");
        host.detach(id).unwrap();
    }
}
