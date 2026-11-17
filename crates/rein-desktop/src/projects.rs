//! `ProjectStore` (desktop.md §Components 3, §Data Model `ProjectsFile`, E-38): the recent
//! projects in the app's own state file `$XDG_STATE_HOME/rein-app/projects.json` (Windows:
//! `%LOCALAPPDATA%\rein-app`). At most 64 entries, newest first; written to a temp file and
//! renamed. A path that no longer exists is kept and shown dimmed, never removed silently.
//! Nothing inside a project's `.ai/` is read, and nothing is written under `$HOME/.rein` or
//! inside a project.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::project_path;

pub const FILE_NAME: &str = "projects.json";
pub const MAX_RECENT: usize = 64;
/// The most open tabs the record keeps (desktop.md V12 §Data model, E-70).
pub const MAX_OPEN: usize = 32;
/// A projects.json larger than this is not ours; it is ignored (and replaced on next write).
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recent {
    /// Canonical absolute path (the Linux path inside WSL on Windows).
    pub path: String,
    /// Seconds since the Unix epoch.
    pub last_opened: u64,
    /// Windows: the WSL distro the path belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
}

/// One open project tab, as the record keeps it (V12 component 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenTab {
    /// Canonical absolute path (the Linux path inside WSL on Windows).
    pub path: String,
    /// Windows: the WSL distro the path belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectsFile {
    pub schema: u32,
    pub recent: Vec<Recent>,
    /// The open project tabs, in tab order (V12 component 4): written on tab open, close,
    /// reorder and switch; a tab leaves it only by Close project, never by quit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open: Vec<OpenTab>,
    /// Which of `open` was the current tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<usize>,
}

impl Default for ProjectsFile {
    fn default() -> Self {
        Self {
            schema: 1,
            recent: Vec::new(),
            open: Vec::new(),
            active: None,
        }
    }
}

/// A distro name the record keeps: what `wsl.exe --list` could print, nothing else.
fn distro_ok(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= 64
        && d.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// The record as read from a file: a malformed entry is dropped, never repaired; at most
/// [`MAX_OPEN`]; an `active` that names no entry is dropped.
fn clean_open(open: Vec<OpenTab>, active: Option<usize>) -> (Vec<OpenTab>, Option<usize>) {
    let mut kept = Vec::new();
    let mut new_active = None;
    for (i, t) in open.into_iter().enumerate() {
        let good = project_path::check(&t.path).is_ok()
            && t.distro.as_deref().is_none_or(distro_ok)
            && !kept.iter().any(|k: &OpenTab| k == &t);
        if good && kept.len() < MAX_OPEN {
            if active == Some(i) {
                new_active = Some(kept.len());
            }
            kept.push(t);
        }
    }
    (kept, new_active)
}

/// One row as the web side shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentView {
    pub path: String,
    pub name: String,
    pub last_opened: u64,
    /// `false`: the directory is gone (shown dimmed). `None`: not checked (Windows — W2: no
    /// `wsl.exe` call per row).
    pub exists: Option<bool>,
}

pub struct ProjectStore {
    file: Option<PathBuf>,
    /// Whether rows are checked for existence (not on Windows, where they live inside WSL).
    check_exists: bool,
    data: Mutex<ProjectsFile>,
}

impl ProjectStore {
    pub fn open(state_dir: Option<PathBuf>, check_exists: bool) -> Self {
        let file = state_dir.map(|d| d.join(FILE_NAME));
        let data = file.as_deref().map(load).unwrap_or_default();
        Self {
            file,
            check_exists,
            data: Mutex::new(data),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ProjectsFile> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Vec<RecentView> {
        self.lock()
            .recent
            .iter()
            .map(|r| RecentView {
                name: name_of(&r.path),
                path: r.path.clone(),
                last_opened: r.last_opened,
                exists: self.check_exists.then(|| Path::new(&r.path).is_dir()),
            })
            .collect()
    }

    /// The project was opened: move it to the front (adding it), keep at most 64, save.
    pub fn touch(&self, path: &str, distro: Option<String>) -> std::io::Result<()> {
        if project_path::check(path).is_err() {
            return Ok(()); // never store a path the window would refuse to show
        }
        let mut d = self.lock();
        d.recent.retain(|r| r.path != path);
        d.recent.insert(
            0,
            Recent {
                path: path.to_string(),
                last_opened: now(),
                distro,
            },
        );
        d.recent.truncate(MAX_RECENT);
        self.save(&d)
    }

    /// The open-tabs record (V12 component 4).
    pub fn open_tabs(&self) -> (Vec<OpenTab>, Option<usize>) {
        let d = self.lock();
        (d.open.clone(), d.active)
    }

    /// Rewrite the open-tabs record: the tabs in tab order and the current one. Cleaned as a
    /// read record is; saved with a temp file and a rename.
    pub fn set_open(&self, open: Vec<OpenTab>, active: Option<usize>) -> std::io::Result<()> {
        let (open, active) = clean_open(open, active);
        let mut d = self.lock();
        if d.open == open && d.active == active {
            return Ok(());
        }
        d.open = open;
        d.active = active;
        self.save(&d)
    }

    /// `projects_forget`: drop one entry (the operator's choice, never automatic).
    pub fn forget(&self, path: &str) -> std::io::Result<()> {
        let mut d = self.lock();
        d.recent.retain(|r| r.path != path);
        self.save(&d)
    }

    fn save(&self, d: &ProjectsFile) -> std::io::Result<()> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension(format!("json.{}.tmp", std::process::id()));
        let text = serde_json::to_string_pretty(d).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, file)
    }
}

/// Read projects.json; anything unreadable, oversized, of another schema or holding a path
/// that fails the text rule is ignored (that row, or the whole file).
fn load(file: &Path) -> ProjectsFile {
    let ok = std::fs::metadata(file).is_ok_and(|m| m.len() <= MAX_BYTES);
    let parsed: Option<ProjectsFile> = ok
        .then(|| std::fs::read_to_string(file).ok())
        .flatten()
        .and_then(|t| serde_json::from_str(&t).ok());
    match parsed {
        Some(mut p) if p.schema == 1 => {
            p.recent.retain(|r| project_path::check(&r.path).is_ok());
            p.recent.truncate(MAX_RECENT);
            let (open, active) = clean_open(std::mem::take(&mut p.open), p.active);
            p.open = open;
            p.active = active;
            p
        }
        _ => ProjectsFile::default(),
    }
}

fn name_of(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("/")
        .to_string()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &Path) -> ProjectStore {
        ProjectStore::open(Some(dir.to_path_buf()), true)
    }

    /// V12 component 4 (E-70): the open-tabs record round trip, a malformed entry dropped,
    /// the cap, and a path from the file held to the same rule as a picked one.
    #[test]
    fn the_open_tabs_record_round_trips_and_drops_what_is_odd() {
        let t = tempfile::tempdir().unwrap();
        let s = store(t.path());
        let tab = |p: &str, d: Option<&str>| OpenTab {
            path: p.into(),
            distro: d.map(Into::into),
        };
        s.set_open(
            vec![tab("/p/a", None), tab("/p/b", Some("Ubuntu"))],
            Some(1),
        )
        .unwrap();
        assert_eq!(
            store(t.path()).open_tabs(),
            s.open_tabs(),
            "read back the same"
        );
        assert_eq!(s.open_tabs().1, Some(1));
        // The cap, and an active index past it dropped.
        let many: Vec<OpenTab> = (0..40).map(|i| tab(&format!("/p/{i}"), None)).collect();
        s.set_open(many, Some(35)).unwrap();
        assert_eq!(s.open_tabs().0.len(), MAX_OPEN);
        assert_eq!(s.open_tabs().1, None);
        // A file written by hand: each odd entry dropped, the rest kept in order.
        std::fs::write(
            t.path().join(FILE_NAME),
            r#"{"schema":1,"recent":[],"open":[
                {"path":"/p/ok"},
                {"path":"relative/x"},
                {"path":"/p/bidi\u202ename"},
                {"path":"/p/two","distro":"Ub untu"},
                {"path":"/p/ok"},
                {"path":"/p/last","distro":"Debian-12"}
              ],"active":5}"#,
        )
        .unwrap();
        let (open, active) = store(t.path()).open_tabs();
        assert_eq!(
            open,
            [tab("/p/ok", None), tab("/p/last", Some("Debian-12"))]
        );
        assert_eq!(active, Some(1), "active follows its entry");
        // An entry of another shape drops the record, not the recent list.
        std::fs::write(
            t.path().join(FILE_NAME),
            r#"{"schema":1,"recent":[{"path":"/p/r","last_opened":1}],"open":"nope"}"#,
        )
        .unwrap();
        let again = store(t.path());
        assert_eq!(again.open_tabs(), (vec![], None));
        // A file without the fields: as before (the start screen).
        std::fs::write(t.path().join(FILE_NAME), r#"{"schema":1,"recent":[]}"#).unwrap();
        assert_eq!(store(t.path()).open_tabs(), (vec![], None));
    }

    #[test]
    fn newest_first_at_most_64_and_saved_atomically() {
        let t = tempfile::tempdir().unwrap();
        let s = store(t.path());
        for i in 0..70 {
            s.touch(&format!("/p/{i}"), None).unwrap();
        }
        let l = s.list();
        assert_eq!(l.len(), MAX_RECENT);
        assert_eq!(l[0].path, "/p/69");
        assert_eq!(l[0].name, "69");
        s.touch("/p/10", None).unwrap();
        assert_eq!(s.list()[0].path, "/p/10", "reopening moves it to the front");
        assert_eq!(s.list().len(), MAX_RECENT, "no duplicate");
        // Reopened from disk: the same list; no temp file left behind.
        assert_eq!(store(t.path()).list(), s.list());
        let names: Vec<String> = std::fs::read_dir(t.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [FILE_NAME]);
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(t.path().join(FILE_NAME)).unwrap())
                .unwrap();
        assert_eq!(doc["schema"], 1);
    }

    #[test]
    fn a_missing_path_is_kept_and_marked_and_forget_removes_it() {
        let t = tempfile::tempdir().unwrap();
        let s = store(t.path());
        s.touch("/no/such/project", None).unwrap();
        s.touch("/no/other/project", None).unwrap();
        assert_eq!(s.list()[1].exists, Some(false), "kept, shown dimmed");
        s.forget("/no/such/project").unwrap();
        assert_eq!(s.list().len(), 1);
        let unchecked = ProjectStore::open(Some(t.path().to_path_buf()), false);
        assert_eq!(unchecked.list()[0].exists, None, "Windows: not probed");
    }

    /// Only a POSIX path is stored (on Windows they are WSL paths and never probed), so a
    /// live directory on this machine exists only where the temp dir is a POSIX path.
    #[cfg(unix)]
    #[test]
    fn a_live_path_is_marked_present() {
        let t = tempfile::tempdir().unwrap();
        let live = tempfile::tempdir().unwrap();
        let s = store(t.path());
        s.touch(live.path().to_str().unwrap(), None).unwrap();
        assert_eq!(s.list()[0].exists, Some(true));
    }

    #[test]
    fn unsafe_or_foreign_contents_are_refused() {
        let t = tempfile::tempdir().unwrap();
        let s = store(t.path());
        s.touch("/a\u{1b}[2Jb", None).unwrap();
        s.touch("relative", None).unwrap();
        assert!(
            s.list().is_empty(),
            "a path with a control character is never stored"
        );
        std::fs::write(
            t.path().join(FILE_NAME),
            r#"{"schema":1,"recent":[{"path":"/ok","last_opened":1},{"path":"/a\u001bb","last_opened":2}]}"#,
        )
        .unwrap();
        let l = store(t.path()).list();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].path, "/ok");
        std::fs::write(t.path().join(FILE_NAME), r#"{"schema":2,"recent":[]}"#).unwrap();
        assert!(store(t.path()).list().is_empty());
        std::fs::write(t.path().join(FILE_NAME), "not json").unwrap();
        assert!(store(t.path()).list().is_empty());
    }

    #[test]
    fn last_is_the_newest() {
        let t = tempfile::tempdir().unwrap();
        let s = store(t.path());
        // `window.start = "last"` opens the first row.
        assert!(s.list().is_empty());
        s.touch("/a", None).unwrap();
        s.touch("/b", Some("Ubuntu".into())).unwrap();
        assert_eq!(s.list()[0].path, "/b");
    }
}
