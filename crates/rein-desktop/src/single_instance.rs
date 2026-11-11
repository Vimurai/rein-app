//! One window per user session (desktop.md §Execution Constraints). The first start binds a
//! Unix socket in `$XDG_RUNTIME_DIR/rein-app` (else the state directory); a later start
//! connects, hands over its DIR on one line and exits 0. No plugin, no D-Bus (D-035).
//!
//! The socket's directory is created `0700`: only the user's own processes can hand the
//! window a path, and the window still validates every path it receives.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use nix::fcntl::{Flock, FlockArg};

use crate::project_path;

const SOCKET_NAME: &str = "desktop.sock";
const LOCK_NAME: &str = "desktop.lock";
/// A handoff line: one path plus the newline. Anything longer is dropped.
const MAX_LINE: u64 = project_path::MAX_PATH_BYTES as u64 + 1;
/// A peer that connects and sends nothing cannot hold the accept loop.
const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// The directory holding the socket: `$XDG_RUNTIME_DIR/rein-app` when that variable is an
/// absolute path, else the app's state directory.
pub fn instance_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let abs = |k: &str| env(k).map(PathBuf::from).filter(|p| p.is_absolute());
    abs("XDG_RUNTIME_DIR")
        .map(|d| d.join("rein-app"))
        .or_else(|| crate::dirs::state_dir(env))
}

/// The result of trying to become the running window.
pub enum Instance {
    /// This process is the window; serve handoffs from this listener.
    Primary(UnixListener),
    /// A window was already running and took the DIR (if any).
    HandedOff,
}

/// Become the primary instance, or hand `dir` to the one that is running.
pub fn acquire(dir: &Path, handoff: Option<&str>) -> std::io::Result<Instance> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    // An existing directory keeps its mode; refuse one other users can reach.
    let mode = std::fs::metadata(dir)?.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(std::io::Error::other(format!(
            "{} is reachable by other users (mode {:o})",
            dir.display(),
            mode & 0o777
        )));
    }
    let sock = dir.join(SOCKET_NAME);
    // Serialise starts: without it two starts that both find a stale socket can each remove
    // the other's fresh one and both become the window. Released when this returns.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK_NAME))?;
    let _held =
        Flock::lock(lock, FlockArg::LockExclusive).map_err(|(_, e)| std::io::Error::from(e))?;
    let mut attempt = 1;
    loop {
        match try_acquire(&sock, handoff) {
            // The window was closing: it accepted nothing or exited before replying. Try
            // again; the next attempt finds the socket refused and takes over.
            Err(e) if vanished(&e) && attempt < ATTEMPTS => {
                attempt += 1;
                std::thread::sleep(RETRY_PAUSE);
            }
            r => return r,
        }
    }
}

/// How often a start retries when the running window goes away mid-handoff.
const ATTEMPTS: u32 = 5;
const RETRY_PAUSE: Duration = Duration::from_millis(100);

/// The peer closed the connection without replying: the window exited, it did not hang.
fn vanished(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(
        e.kind(),
        ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof
    )
}

fn try_acquire(sock: &Path, handoff: Option<&str>) -> std::io::Result<Instance> {
    // Bind first: two simultaneous starts cannot both win a bind.
    match UnixListener::bind(sock) {
        Ok(l) => return Ok(Instance::Primary(l)),
        Err(e) if e.kind() != std::io::ErrorKind::AddrInUse => return Err(e),
        Err(_) => {}
    }
    match UnixStream::connect(sock) {
        Ok(mut s) => {
            s.write_all(format!("{}\n", handoff.unwrap_or("")).as_bytes())?;
            // Wait for the window's one-byte reply before closing. A client that closes
            // before the window reaches `accept` can lose its connection (ECONNABORTED on
            // macOS), and the reply means "handed off" is true when this returns.
            s.set_read_timeout(Some(READ_TIMEOUT))?;
            let mut ack = [0u8; 1];
            s.read_exact(&mut ack)?;
            Ok(Instance::HandedOff)
        }
        // A socket file with nobody listening: left by a window that crashed or closed.
        // Removing it can race another start doing the same; a missing file is fine.
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            match std::fs::remove_file(sock) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
            Ok(Instance::Primary(UnixListener::bind(sock)?))
        }
        Err(e) => Err(e),
    }
}

/// Read one handoff from `stream`: `Some(Some(path))` for a valid path, `Some(None)` for an
/// empty line (a bare second start: just raise the window), `None` for anything else.
/// A complete line is acknowledged with one byte, valid or not; the sender waits for it.
pub fn read_handoff(mut stream: UnixStream) -> Option<Option<PathBuf>> {
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(READ_TIMEOUT)).ok()?;
    let mut line = String::new();
    BufReader::new((&stream).take(MAX_LINE))
        .read_line(&mut line)
        .ok()?;
    let _ = stream.write_all(b"\n");
    let line = line.strip_suffix('\n')?;
    if line.is_empty() {
        return Some(None);
    }
    project_path::check(line).ok().map(Some)
}

/// Serve handoffs until the listener fails; `on_handoff` runs once per valid request with the
/// DIR (`None`: only raise the window) — the same call as on Windows.
pub fn serve(listener: UnixListener, on_handoff: impl Fn(Option<String>) + Send + 'static) {
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if let Some(req) = read_handoff(stream) {
                on_handoff(req.map(|p| p.to_string_lossy().into_owned()));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn dir() -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("rein-app");
        (t, d)
    }

    #[test]
    fn second_start_hands_its_dir_to_the_first() {
        let (_t, d) = dir();
        let Instance::Primary(l) = acquire(&d, None).unwrap() else {
            panic!("first start must be primary")
        };
        let (tx, rx) = mpsc::channel();
        serve(l, move |p| tx.send(p).unwrap());
        assert!(matches!(
            acquire(&d, Some("/home/u/proj")).unwrap(),
            Instance::HandedOff
        ));
        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got, Some("/home/u/proj".to_string()));
        assert!(matches!(acquire(&d, None).unwrap(), Instance::HandedOff));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), None);
    }

    #[test]
    fn a_stale_socket_is_replaced() {
        let (_t, d) = dir();
        let first = acquire(&d, None).unwrap();
        drop(first); // the listener closes; the socket file stays, as after a crash
        assert!(d.join(SOCKET_NAME).exists());
        assert!(matches!(acquire(&d, None).unwrap(), Instance::Primary(_)));
    }

    #[test]
    fn a_window_that_exits_mid_handoff_is_taken_over() {
        // The window is closing: its listener is still bound, so connect succeeds, but it
        // accepts and drops the connection without replying, then goes away.
        let (_t, d) = dir();
        let Instance::Primary(l) = acquire(&d, None).unwrap() else {
            panic!("first start must be primary")
        };
        let closing = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            drop(s);
            drop(l);
        });
        assert!(matches!(
            acquire(&d, Some("/home/u/proj")).unwrap(),
            Instance::Primary(_)
        ));
        closing.join().unwrap();
    }

    #[test]
    fn the_instance_dir_is_private_and_a_shared_one_is_refused() {
        let (_t, d) = dir();
        acquire(&d, None).unwrap();
        assert_eq!(
            std::fs::metadata(&d).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let (_t2, open) = dir();
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(acquire(&open, None).is_err());
    }

    #[test]
    fn an_unsafe_or_oversized_handoff_is_dropped() {
        for payload in [
            "/a\u{1b}]52;c;x\u{7}\n".to_string(),
            "relative\n".to_string(),
            "/no-newline".to_string(),
            format!("/{}\n", "a".repeat(project_path::MAX_PATH_BYTES + 10)),
        ] {
            let (mut a, b) = UnixStream::pair().unwrap();
            a.write_all(payload.as_bytes()).unwrap();
            drop(a);
            assert_eq!(read_handoff(b), None, "{payload:.40?}");
        }
    }

    #[test]
    fn instance_dir_prefers_an_absolute_runtime_dir() {
        let env = |k: &str| match k {
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".into()),
            "HOME" => Some("/home/u".into()),
            _ => None,
        };
        assert_eq!(
            instance_dir(&env),
            Some(PathBuf::from("/run/user/1000/rein-app"))
        );
        let rel = |k: &str| match k {
            "XDG_RUNTIME_DIR" => Some("run".into()),
            "HOME" => Some("/home/u".into()),
            _ => None,
        };
        assert_eq!(instance_dir(&rel), crate::dirs::state_dir(&rel));
    }
}
