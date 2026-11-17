//! Dismissed "Needs you" items (status.md §Components 4 "Dismiss", D-063; E-75):
//! `$XDG_STATE_HOME/rein-app/dismissed.json` (on Windows `%LOCALAPPDATA%\rein-app`) =
//! `{schema: 1, items: [{project, key, text_hash, at}]}`, at most 500, the oldest dropped.
//! `text_hash` is the SHA-256 of what the item said, so no text from a project is stored and
//! an item comes back once its text changes. Written with a temp file and a rename; a file
//! that cannot be read, is too large or is of another schema is ignored.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FILE_NAME: &str = "dismissed.json";
/// The most dismissals kept.
pub const MAX: usize = 500;
/// A larger file is not read.
const MAX_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dismissal {
    pub project: String,
    pub key: String,
    pub text_hash: String,
    pub at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    schema: u32,
    items: Vec<Dismissal>,
}

/// The SHA-256 of an item's words, as 64 hex.
pub fn text_hash(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0u8]);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub struct Dismissed {
    file: Option<PathBuf>,
    items: Mutex<Vec<Dismissal>>,
}

impl Dismissed {
    pub fn open(state_dir: Option<PathBuf>) -> Self {
        let file = state_dir.map(|d| d.join(FILE_NAME));
        let items = file.as_deref().map(load).unwrap_or_default();
        Self {
            file,
            items: Mutex::new(items),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Dismissal>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The item `key` of `project`, saying what `text_hash` stands for, was dismissed.
    pub fn has(&self, project: &str, key: &str, text_hash: &str) -> bool {
        self.lock()
            .iter()
            .any(|d| d.project == project && d.key == key && d.text_hash == text_hash)
    }

    /// Keep a dismissal (an earlier one of the same item is replaced); at most [`MAX`].
    pub fn add(&self, d: Dismissal, now: u64) -> std::io::Result<()> {
        let mut items = self.lock();
        items.retain(|x| !(x.project == d.project && x.key == d.key));
        items.push(Dismissal { at: now, ..d });
        if items.len() > MAX {
            let extra = items.len() - MAX;
            items.drain(..extra);
        }
        self.save(&items)
    }

    fn save(&self, items: &[Dismissal]) -> std::io::Result<()> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension(format!("json.{}.tmp", std::process::id()));
        let body = File {
            schema: 1,
            items: items.to_vec(),
        };
        let text = serde_json::to_string_pretty(&body).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, file)
    }
}

/// The file's dismissals, or none: unreadable, too large, odd or another schema.
fn load(file: &Path) -> Vec<Dismissal> {
    let Ok(meta) = std::fs::metadata(file) else {
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    match serde_json::from_str::<File>(&text) {
        Ok(f) if f.schema == 1 => {
            let mut items: Vec<Dismissal> = f
                .items
                .into_iter()
                .filter(|d| d.text_hash.len() == 64 && d.key.len() <= 64)
                .collect();
            if items.len() > MAX {
                let extra = items.len() - MAX;
                items.drain(..extra);
            }
            items
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(project: &str, key: &str, text: &str) -> Dismissal {
        Dismissal {
            project: project.into(),
            key: key.into(),
            text_hash: text_hash(&[text]),
            at: 0,
        }
    }

    #[test]
    fn a_dismissal_survives_a_restart_and_holds_only_a_hash() {
        let dir = tempfile::tempdir().unwrap();
        let s = Dismissed::open(Some(dir.path().into()));
        s.add(d("/p", "mention:0", "the operator should sign"), 10)
            .unwrap();
        let again = Dismissed::open(Some(dir.path().into()));
        assert!(again.has("/p", "mention:0", &text_hash(&["the operator should sign"])));
        assert!(
            !again.has("/p", "mention:0", &text_hash(&["a new line"])),
            "changed text"
        );
        assert!(!again.has("/q", "mention:0", &text_hash(&["the operator should sign"])));
        let raw = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(!raw.contains("sign"), "no project text is stored: {raw}");
        assert!(raw.contains("\"schema\": 1"));
    }

    #[test]
    fn at_most_500_the_oldest_dropped_and_a_bad_file_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let s = Dismissed::open(Some(dir.path().into()));
        for i in 0..(MAX + 3) {
            s.add(d("/p", &format!("bell:{i}"), "x"), i as u64).unwrap();
        }
        let again = Dismissed::open(Some(dir.path().into()));
        assert!(!again.has("/p", "bell:0", &text_hash(&["x"])));
        assert!(again.has("/p", &format!("bell:{}", MAX + 2), &text_hash(&["x"])));
        assert_eq!(again.lock().len(), MAX);
        // The same item dismissed again replaces the earlier entry.
        again.add(d("/p", "bell:600", "y"), 1).unwrap();
        again.add(d("/p", "bell:600", "z"), 2).unwrap();
        assert_eq!(
            again.lock().iter().filter(|x| x.key == "bell:600").count(),
            1
        );
        for bad in [
            "not json",
            r#"{"schema":2,"items":[]}"#,
            r#"{"schema":1,"items":[{"project":"/p","key":"k","text_hash":"short","at":1}]}"#,
        ] {
            std::fs::write(dir.path().join(FILE_NAME), bad).unwrap();
            assert!(
                Dismissed::open(Some(dir.path().into())).lock().is_empty(),
                "{bad}"
            );
        }
        assert!(
            Dismissed::open(None).add(d("/p", "k", "t"), 1).is_ok(),
            "no state dir: kept in memory"
        );
    }
}
