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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectsFile {
    pub schema: u32,
    pub recent: Vec<Recent>,
}

impl Default for ProjectsFile {
    fn default() -> Self {
        Self {
            schema: 1,
            recent: Vec::new(),
        }
    }
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
        let live = tempfile::tempdir().unwrap();
        let live_path = live.path().to_str().unwrap().to_string();
        let s = store(t.path());
        s.touch("/no/such/project", None).unwrap();
        s.touch(&live_path, None).unwrap();
        let l = s.list();
        assert_eq!(l[0].exists, Some(true));
        assert_eq!(l[1].exists, Some(false), "kept, shown dimmed");
        s.forget("/no/such/project").unwrap();
        assert_eq!(s.list().len(), 1);
        let unchecked = ProjectStore::open(Some(t.path().to_path_buf()), false);
        assert_eq!(unchecked.list()[0].exists, None, "Windows: not probed");
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
