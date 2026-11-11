//! The handoff directory of the Windows single instance (desktop.md §Windows window W5):
//! a second start writes its DIR into `handoff\<random>.txt` (temp name + rename, at most
//! 4 KiB) and waits for the window to delete it; the window takes each file, deletes it and
//! opens the text as a tab after the usual validation. No listener, no pipe.
//!
//! Plain files, so the logic runs and is tested on every platform; only Windows uses it.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::project_path;

/// The largest handoff accepted; a longer file is deleted unread.
pub const MAX_HANDOFF: u64 = 4096;
/// How long a second start waits for the window to take its handoff.
pub const WAIT: Duration = Duration::from_secs(5);
/// How often the window looks for handoffs.
pub const POLL: Duration = Duration::from_millis(200);

const SUFFIX: &str = "txt";

/// Second start: hand `text` (a DIR, or empty to only raise the window) to the running window.
/// `Ok(true)` once the window has taken it; `Ok(false)` if nobody took it within `wait` (the
/// file is then removed).
pub fn hand_over(dir: &Path, text: &str, wait: Duration) -> io::Result<bool> {
    std::fs::create_dir_all(dir)?;
    let stem = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    );
    let tmp = dir.join(format!("{stem}.tmp"));
    let file = dir.join(format!("{stem}.{SUFFIX}"));
    std::fs::write(&tmp, text.as_bytes())?;
    std::fs::rename(&tmp, &file)?;
    let deadline = std::time::Instant::now() + wait;
    while std::time::Instant::now() < deadline {
        if !file.exists() {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Not taken in time. If the window takes it just now, removing it fails: that is a yes.
    match std::fs::remove_file(&file) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        _ => Ok(false),
    }
}

/// Window: take every pending handoff, oldest first. Each file is deleted whatever it holds.
/// `Some(None)`: an empty handoff (raise the window); `Some(Some(text))`: a DIR that passed
/// the text rule (the host validates it further when the tab opens). Files older than
/// `stale_after`, larger than [`MAX_HANDOFF`] or failing the text rule are dropped.
pub fn take(dir: &Path, stale_after: Duration) -> Vec<Option<String>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == SUFFIX))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    files.sort();
    let now = SystemTime::now();
    let mut out = Vec::new();
    for (modified, path) in files {
        let fresh = now.duration_since(modified).unwrap_or_default() <= stale_after;
        let small = std::fs::metadata(&path).is_ok_and(|m| m.len() <= MAX_HANDOFF);
        // Bounded read too: the file could grow between the size check and the read.
        let text = (fresh && small).then(|| read_bounded(&path)).flatten();
        let _ = std::fs::remove_file(&path);
        match text.as_deref() {
            Some("") => out.push(None),
            Some(t) if project_path::check_text(t).is_ok() => out.push(Some(t.to_string())),
            _ => {}
        }
    }
    out
}

fn read_bounded(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_HANDOFF + 1)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.len() as u64 > MAX_HANDOFF {
        return None;
    }
    String::from_utf8(buf).ok()
}

/// Window: look for handoffs every [`POLL`] for the rest of the process. Stale files left by
/// an earlier crash are dropped on the first pass.
pub fn watch(dir: PathBuf, on_handoff: impl Fn(Option<String>) + Send + 'static) {
    std::thread::spawn(move || {
        loop {
            for h in take(&dir, WAIT) {
                on_handoff(h);
            }
            std::thread::sleep(POLL);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handoff_is_taken_and_deleted() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("handoff");
        let reader = {
            let dir = dir.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    let got = take(&dir, WAIT);
                    if !got.is_empty() {
                        return got;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Vec::new()
            })
        };
        assert!(hand_over(&dir, "\\\\wsl.localhost\\Ubuntu\\home\\me\\p", WAIT).unwrap());
        assert_eq!(
            reader.join().unwrap(),
            vec![Some("\\\\wsl.localhost\\Ubuntu\\home\\me\\p".to_string())]
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn nobody_taking_it_returns_false_and_cleans_up() {
        let d = tempfile::tempdir().unwrap();
        assert!(!hand_over(d.path(), "/home/me/p", Duration::from_millis(100)).unwrap());
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
    }

    #[test]
    fn empty_raises_and_stale_oversized_or_unsafe_handoffs_are_dropped() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        std::fs::write(p.join("1.txt"), "").unwrap();
        std::fs::write(p.join("2.txt"), "x".repeat(MAX_HANDOFF as usize + 1)).unwrap();
        std::fs::write(p.join("3.txt"), "/a\u{1b}]52;c;x\u{7}").unwrap();
        std::fs::write(p.join("4.tmp"), "/half/written").unwrap();
        assert_eq!(take(p, WAIT), vec![None]);
        let left: Vec<String> = std::fs::read_dir(p)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            left,
            ["4.tmp"],
            "every .txt is deleted; a .tmp is not yet a handoff"
        );

        std::fs::write(p.join("5.txt"), "/home/me/old").unwrap();
        std::fs::File::options()
            .write(true)
            .open(p.join("5.txt"))
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(60))
            .unwrap();
        assert_eq!(take(p, WAIT), Vec::<Option<String>>::new(), "stale");
        assert!(!p.join("5.txt").exists());
    }
}
