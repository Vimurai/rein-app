//! The two project files the panels read (desktop panels.md §Security, §API, D-056; E-59):
//! `.ai/state.json` and `.ai/COMM.md` under a tab's own project — a closed enum, two constant
//! relative paths, nothing else can be named. Both files are written by agents and can ship
//! inside a cloned repository: untrusted. Before any byte is read the target must be a regular
//! file (not a symlink, FIFO, device or directory); `state.json` is read whole only up to
//! 8 MiB, `COMM.md` only from its last 512 KiB. Every refusal is one of the fixed sentences of
//! [`Unavailable`]; no file text ever appears in one.

use std::path::Path;

/// The file a panel source reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectFile {
    StateJson,
    CommMd,
}

impl ProjectFile {
    /// The file's name inside the project's `.ai` directory.
    pub fn name(self) -> &'static str {
        match self {
            Self::StateJson => "state.json",
            Self::CommMd => "COMM.md",
        }
    }
}

/// `state.json` larger than this is refused unread.
pub const STATE_MAX: u64 = 8 * 1024 * 1024;
/// How much of the end of `COMM.md` is read.
pub const COMM_TAIL: u64 = 512 * 1024;

/// Why a source is not available — the app's fixed sentences (panels.md §Components 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    Missing,
    NotRegular,
    TooLarge,
    Unreadable,
    Shape,
    TeamMode,
    TooManyTasks,
}

impl Unavailable {
    pub fn sentence(self) -> &'static str {
        match self {
            Self::Missing => "the project has no such file in .ai yet",
            Self::NotRegular => "the file in .ai is not a plain file (a link, a pipe or a folder)",
            Self::TooLarge => "the file in .ai is larger than this window reads",
            Self::Unreadable => "the file in .ai could not be read",
            Self::Shape => "the file in .ai is not in the form this version reads",
            Self::TeamMode => "this project keeps tasks in a form this version does not read",
            Self::TooManyTasks => "the project lists more tasks than this window reads",
        }
    }
}

/// What identifies one version of a file: a stat (no read). Equal stamps → the cached parse
/// is reused (panels.md §Execution Constraints: read only when it changed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub len: u64,
    pub mtime_ns: i128,
    /// The inode (Linux / macOS) or 0 where the platform gives none through std.
    pub id: u64,
    /// The status-change time (Linux / macOS) or 0.
    pub ctime_ns: i128,
}

/// The bytes read and whether they start at the file's beginning (a `COMM.md` tail may not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBytes {
    pub stamp: FileStamp,
    pub bytes: Vec<u8>,
    pub from_start: bool,
}

/// How much of a file of `len` bytes is read, from where: `(offset, max bytes)`, or too large.
pub fn read_window(which: ProjectFile, len: u64) -> Result<(u64, u64), Unavailable> {
    match which {
        ProjectFile::StateJson if len > STATE_MAX => Err(Unavailable::TooLarge),
        ProjectFile::StateJson => Ok((0, len)),
        ProjectFile::CommMd => Ok((len.saturating_sub(COMM_TAIL), len.min(COMM_TAIL))),
    }
}

/// Linux / macOS: every step through a directory fd, never a path re-walked —
/// the project (O_DIRECTORY), then `.ai` (O_DIRECTORY | O_NOFOLLOW), a stat of the name that
/// does not follow a link (so a device is never opened), then the file (O_NOFOLLOW |
/// O_NONBLOCK, so a FIFO cannot block), and a stat of the open fd. Swapping `.ai` or the file
/// for a link between the steps fails closed (security review of E-59, P2-1).
#[cfg(unix)]
pub fn read_local(
    project: &Path,
    which: ProjectFile,
    read: bool,
) -> Result<FileBytes, Unavailable> {
    use nix::errno::Errno;
    use nix::fcntl::{AtFlags, OFlag, openat};
    use nix::sys::stat::{Mode, SFlag, fstat, fstatat};

    struct Fd(std::os::fd::RawFd);
    impl Drop for Fd {
        fn drop(&mut self) {
            let _ = nix::unistd::close(self.0);
        }
    }
    let gone = |e: Errno| match e {
        Errno::ENOENT => Unavailable::Missing,
        Errno::ELOOP | Errno::ENOTDIR | Errno::EISDIR => Unavailable::NotRegular,
        _ => Unavailable::Unreadable,
    };
    let dir_flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC;
    let proj = Fd(openat(None, project, dir_flags, Mode::empty()).map_err(gone)?);
    let ai = Fd(openat(
        Some(proj.0),
        ".ai",
        dir_flags | OFlag::O_NOFOLLOW,
        Mode::empty(),
    )
    .map_err(gone)?);
    let pre = fstatat(Some(ai.0), which.name(), AtFlags::AT_SYMLINK_NOFOLLOW).map_err(gone)?;
    if SFlag::from_bits_truncate(pre.st_mode) & SFlag::S_IFMT != SFlag::S_IFREG {
        return Err(Unavailable::NotRegular);
    }
    let file_flags = OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
    let f = Fd(openat(Some(ai.0), which.name(), file_flags, Mode::empty()).map_err(gone)?);
    let st = fstat(f.0).map_err(gone)?;
    if SFlag::from_bits_truncate(st.st_mode) & SFlag::S_IFMT != SFlag::S_IFREG {
        return Err(Unavailable::NotRegular);
    }
    let ns = |s: i64, n: i64| i128::from(s) * 1_000_000_000 + i128::from(n);
    let len = u64::try_from(st.st_size).map_err(|_| Unavailable::Unreadable)?;
    let stamp = FileStamp {
        len,
        mtime_ns: ns(st.st_mtime as i64, st.st_mtime_nsec as i64),
        #[allow(clippy::unnecessary_cast)] // st_ino is u32 on some targets
        id: st.st_ino as u64,
        ctime_ns: ns(st.st_ctime as i64, st.st_ctime_nsec as i64),
    };
    let (offset, max) = read_window(which, len)?;
    if !read {
        return Ok(FileBytes {
            stamp,
            bytes: Vec::new(),
            from_start: offset == 0,
        });
    }
    let off = i64::try_from(offset).map_err(|_| Unavailable::Unreadable)?;
    nix::unistd::lseek(f.0, off, nix::unistd::Whence::SeekSet).map_err(gone)?;
    let strict = which == ProjectFile::StateJson;
    let bytes = read_bounded(
        &mut |buf| nix::unistd::read(f.0, buf).map_err(|_| ()),
        max,
        strict,
    )?;
    Ok(FileBytes {
        stamp,
        bytes,
        from_start: offset == 0,
    })
}

/// Read at most `max` bytes with `next`. `strict`: one byte more than the stat promised means
/// the file is not what was checked — refused (`state.json`); otherwise the read stops at
/// `max` (a `COMM.md` an agent is appending to — E-59 review). A file that shrank reads short.
pub fn read_bounded(
    next: &mut dyn FnMut(&mut [u8]) -> Result<usize, ()>,
    max: u64,
    strict: bool,
) -> Result<Vec<u8>, Unavailable> {
    let max = usize::try_from(max).map_err(|_| Unavailable::TooLarge)?;
    let mut out = Vec::with_capacity(max.min(1 << 20));
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let room = if strict { max + 1 } else { max } - out.len();
        if room == 0 {
            return Ok(out);
        }
        let take = room.min(buf.len());
        let n = next(&mut buf[..take]).map_err(|_| Unavailable::Unreadable)?;
        if n == 0 {
            return Ok(out);
        }
        out.extend_from_slice(&buf[..n]);
        if out.len() > max {
            return Err(Unavailable::Unreadable);
        }
    }
}

/// Windows: through the distro's UNC path, no process. `.ai` must be a directory and not a
/// reparse point; the file is opened without following a reparse point and its HANDLE must
/// be neither a reparse point (a WSL symlink, FIFO, socket or device all show as one through
/// the share — measured, E-59) nor a directory. The workspace forbids `unsafe`, so the
/// handle's final path is not compared (security review P2-1, Windows half): a process in
/// WSL that swaps `.ai` for a link between the two steps is the user already.
#[cfg(windows)]
pub fn read_unc(ai_dir: &Path, which: ProjectFile, read: bool) -> Result<FileBytes, Unavailable> {
    use std::io::{Read, Seek, SeekFrom};
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    const REPARSE: u32 = 0x400;
    const DIRECTORY: u32 = 0x10;
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let gone = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::NotFound => Unavailable::Missing,
        _ => Unavailable::Unreadable,
    };
    let dir = std::fs::symlink_metadata(ai_dir).map_err(gone)?;
    if dir.file_attributes() & REPARSE != 0 || dir.file_attributes() & DIRECTORY == 0 {
        return Err(Unavailable::NotRegular);
    }
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x7) // read | write | delete: never in an agent's way
        .custom_flags(OPEN_REPARSE_POINT)
        .open(ai_dir.join(which.name()))
        .map_err(gone)?;
    let m = f.metadata().map_err(gone)?;
    if m.file_attributes() & (REPARSE | DIRECTORY) != 0 {
        return Err(Unavailable::NotRegular);
    }
    let len = m.len();
    let mtime_ns = i128::from(m.last_write_time()) * 100;
    let stamp = FileStamp {
        len,
        mtime_ns,
        id: 0,
        ctime_ns: 0,
    };
    let (offset, max) = read_window(which, len)?;
    if !read {
        return Ok(FileBytes {
            stamp,
            bytes: Vec::new(),
            from_start: offset == 0,
        });
    }
    f.seek(SeekFrom::Start(offset)).map_err(gone)?;
    let strict = which == ProjectFile::StateJson;
    let bytes = read_bounded(&mut |buf| f.read(buf).map_err(|_| ()), max, strict)?;
    Ok(FileBytes {
        stamp,
        bytes,
        from_start: offset == 0,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn project() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join(".ai")).unwrap();
        t
    }

    #[test]
    fn a_plain_file_is_read_and_stamped() {
        let p = project();
        std::fs::write(p.path().join(".ai/state.json"), b"{\"tasks\":[]}").unwrap();
        let r = read_local(p.path(), ProjectFile::StateJson, true).unwrap();
        assert_eq!(r.bytes, b"{\"tasks\":[]}");
        assert!(r.from_start);
        assert_eq!(r.stamp.len, 12);
        let s = read_local(p.path(), ProjectFile::StateJson, false).unwrap();
        assert_eq!(
            s.stamp, r.stamp,
            "a stat gives the same stamp, reads nothing"
        );
        assert!(s.bytes.is_empty());
    }

    /// panels.md §Tests "Read scope": a symlink, FIFO or directory in place of each file, and
    /// a symlinked `.ai`, are refused; a missing file is "missing".
    #[test]
    fn only_a_regular_file_is_read() {
        for which in [ProjectFile::StateJson, ProjectFile::CommMd] {
            let p = project();
            let target = p.path().join(".ai").join(which.name());
            assert_eq!(
                read_local(p.path(), which, true).unwrap_err(),
                Unavailable::Missing
            );
            let secret = p.path().join("secret");
            std::fs::write(&secret, b"secret").unwrap();
            symlink(&secret, &target).unwrap();
            assert_eq!(
                read_local(p.path(), which, true).unwrap_err(),
                Unavailable::NotRegular,
                "symlink"
            );
            std::fs::remove_file(&target).unwrap();
            nix::unistd::mkfifo(&target, nix::sys::stat::Mode::S_IRWXU).unwrap();
            assert_eq!(
                read_local(p.path(), which, true).unwrap_err(),
                Unavailable::NotRegular,
                "fifo"
            );
            std::fs::remove_file(&target).unwrap();
            std::fs::create_dir(&target).unwrap();
            assert_eq!(
                read_local(p.path(), which, true).unwrap_err(),
                Unavailable::NotRegular,
                "directory"
            );
        }
        let p = project();
        let elsewhere = tempfile::tempdir().unwrap();
        std::fs::write(elsewhere.path().join("state.json"), b"{}").unwrap();
        std::fs::remove_dir(p.path().join(".ai")).unwrap();
        symlink(elsewhere.path(), p.path().join(".ai")).unwrap();
        assert_eq!(
            read_local(p.path(), ProjectFile::StateJson, true).unwrap_err(),
            Unavailable::NotRegular,
            "a symlinked .ai"
        );
    }

    /// A 20 MiB state.json is refused from its stat, without a read; a 20 MiB COMM.md is read
    /// from its last 512 KiB only.
    #[test]
    fn big_files_are_refused_or_tailed_without_a_full_read() {
        let p = project();
        let big = vec![b'x'; 20 * 1024 * 1024];
        std::fs::write(p.path().join(".ai/state.json"), &big).unwrap();
        assert_eq!(
            read_local(p.path(), ProjectFile::StateJson, true).unwrap_err(),
            Unavailable::TooLarge
        );
        std::fs::write(p.path().join(".ai/COMM.md"), &big).unwrap();
        let r = read_local(p.path(), ProjectFile::CommMd, true).unwrap();
        assert_eq!(r.bytes.len() as u64, COMM_TAIL);
        assert!(!r.from_start);
        assert_eq!(read_window(ProjectFile::CommMd, 100).unwrap(), (0, 100));
    }

    #[test]
    fn a_read_longer_than_the_stat_is_refused() {
        let mut chunks = vec![vec![1u8; 10], vec![2u8; 10]].into_iter();
        let mut next = |buf: &mut [u8]| -> Result<usize, ()> {
            match chunks.next() {
                Some(c) => {
                    let n = c.len().min(buf.len());
                    buf[..n].copy_from_slice(&c[..n]);
                    Ok(n)
                }
                None => Ok(0),
            }
        };
        assert_eq!(
            read_bounded(&mut next, 15, true).unwrap_err(),
            Unavailable::Unreadable
        );
        // Not strict (COMM.md being appended to): exactly the window, no refusal.
        let mut endless = |buf: &mut [u8]| -> Result<usize, ()> {
            buf.fill(7);
            Ok(buf.len())
        };
        assert_eq!(
            read_bounded(&mut endless, 15, false).unwrap(),
            vec![7u8; 15]
        );
    }
}
