//! One window per user session on Windows (desktop.md §Windows window W5, D-038). No named
//! pipe: a lock file `%LOCALAPPDATA%\rein-app\instance.lock` held open without sharing for
//! the window's life; a second start that cannot open it hands its DIR over through the
//! handoff directory (`handoff_dir`) and exits 0, or exits 1 if nobody takes it within 5 s.
//! The directory is private to the user by profile ACL; nothing listens.
//!
//! Same calls as the Unix module: `instance_dir`, `acquire`, `serve`.

use std::fs::File;
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::handoff_dir;

const LOCK_NAME: &str = "instance.lock";
const HANDOFF_DIR: &str = "handoff";
/// `ERROR_SHARING_VIOLATION`: another process holds the lock file open.
const SHARING_VIOLATION: i32 = 32;

/// The held lock, kept for the window's life, and where handoffs arrive.
pub struct Guard {
    _lock: File,
    handoff: PathBuf,
}

pub enum Instance {
    Primary(Guard),
    HandedOff,
}

/// `%LOCALAPPDATA%\rein-app` (W6).
pub fn instance_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    crate::dirs::state_dir(env)
}

/// Become the window, or hand `dir` to the one that is running.
pub fn acquire(dir: &Path, handoff: Option<&str>) -> io::Result<Instance> {
    std::fs::create_dir_all(dir.join(HANDOFF_DIR))?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .share_mode(0)
        .open(dir.join(LOCK_NAME));
    match lock {
        Ok(f) => Ok(Instance::Primary(Guard {
            _lock: f,
            handoff: dir.join(HANDOFF_DIR),
        })),
        Err(e) if e.raw_os_error() == Some(SHARING_VIOLATION) => {
            let text = handoff.unwrap_or("");
            if handoff_dir::hand_over(&dir.join(HANDOFF_DIR), text, handoff_dir::WAIT)? {
                Ok(Instance::HandedOff)
            } else {
                Err(io::Error::other(
                    "a rein-app window holds the lock but did not answer within 5 seconds",
                ))
            }
        }
        Err(e) => Err(e),
    }
}

/// Serve handoffs for the rest of the process; `on_handoff` runs once per handoff with the
/// DIR text (`None`: only raise the window). The lock stays held while the process lives.
pub fn serve(guard: Guard, on_handoff: impl Fn(Option<String>) + Send + 'static) {
    let Guard { _lock, handoff } = guard;
    // The watcher runs for the process's life and owns the lock, so the lock is held as long.
    handoff_dir::watch(handoff, move |h| {
        let _held = &_lock;
        on_handoff(h);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn second_start_hands_over_and_a_free_lock_is_taken() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("rein-app");
        let Instance::Primary(g) = acquire(&d, None).unwrap() else {
            panic!("first start must be primary")
        };
        let (tx, rx) = mpsc::channel();
        serve(g, move |h| tx.send(h).unwrap());
        assert!(matches!(
            acquire(&d, Some("/home/me/p")).unwrap(),
            Instance::HandedOff
        ));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Some("/home/me/p".to_string())
        );

        let t2 = tempfile::tempdir().unwrap();
        let free = t2.path().join("rein-app");
        assert!(matches!(
            acquire(&free, None).unwrap(),
            Instance::Primary(_)
        ));
    }
}
