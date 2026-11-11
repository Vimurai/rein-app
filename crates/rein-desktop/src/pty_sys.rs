//! The platform part of `PtyHost`: how the master is written to, and how a detached child is
//! hung up and, if it outlives the grace period, killed. Everything else in `pty.rs` is the
//! same on every platform.

use std::io::Write;

use portable_pty::MasterPty;

use crate::pty::PtyError;

pub use imp::Killer;

/// The writer for a pty master. On Unix it is our own duplicate of the master fd: dropping
/// it only closes it — portable-pty's `take_writer` types a newline and the EOF character on
/// drop, which tmux would pass to the user's pane on every detach (security review of
/// E-35). On Windows the ConPTY input pipe has no such behaviour.
pub fn writer(master: &dyn MasterPty) -> Result<Box<dyn Write + Send>, PtyError> {
    imp::writer(master)
}

#[cfg(unix)]
mod imp {
    use std::io::Write;
    use std::os::fd::{AsRawFd, RawFd};

    use filedescriptor::FileDescriptor;
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;
    use portable_pty::{Child, MasterPty};

    use crate::pty::PtyError;

    /// A borrowed master fd, so [`FileDescriptor::dup`] can duplicate it without `unsafe`.
    struct Raw(RawFd);

    impl AsRawFd for Raw {
        fn as_raw_fd(&self) -> RawFd {
            self.0
        }
    }

    pub fn writer(master: &dyn MasterPty) -> Result<Box<dyn Write + Send>, PtyError> {
        let fd = master
            .as_raw_fd()
            .ok_or_else(|| PtyError::Io("the pty has no file descriptor".into()))?;
        let dup = FileDescriptor::dup(&Raw(fd)).map_err(|e| PtyError::Io(e.to_string()))?;
        Ok(Box::new(dup))
    }

    /// The child's process group: portable-pty makes the child a session leader, so its pid
    /// is the group of everything it starts (`tmux attach` under `rein start`). The tmux
    /// server runs in its own session and is never signalled.
    pub struct Killer(Option<Pid>);

    impl Killer {
        pub fn of(child: &dyn Child) -> Self {
            Self(
                child
                    .process_id()
                    .and_then(|p| i32::try_from(p).ok())
                    .map(Pid::from_raw),
            )
        }

        /// What a closing terminal delivers. The reader holds its own copy of the master, so
        /// closing ours is no hangup by itself.
        pub fn hangup(&self) {
            if let Some(pg) = self.0 {
                let _ = killpg(pg, Signal::SIGHUP);
            }
        }

        /// portable-pty's own killer sends only SIGHUP, which a child may ignore.
        pub fn kill(&self) {
            if let Some(pg) = self.0 {
                let _ = killpg(pg, Signal::SIGKILL); // the waiter thread then reaps it
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::io::Write;
    use std::sync::Mutex;

    use portable_pty::{Child, ChildKiller, MasterPty};

    use crate::pty::PtyError;

    pub fn writer(master: &dyn MasterPty) -> Result<Box<dyn Write + Send>, PtyError> {
        master
            .take_writer()
            .map_err(|e| PtyError::Io(e.to_string()))
    }

    /// `wsl.exe` in the pseudo console. Closing the console ends it (and so hangs up the tmux
    /// client inside WSL); `kill` is `TerminateProcess` for one that does not end.
    pub struct Killer(Mutex<Box<dyn ChildKiller + Send + Sync>>);

    impl Killer {
        pub fn of(child: &dyn Child) -> Self {
            Self(Mutex::new(child.clone_killer()))
        }

        pub fn hangup(&self) {}

        pub fn kill(&self) {
            let _ = self.0.lock().unwrap_or_else(|e| e.into_inner()).kill();
        }
    }
}
