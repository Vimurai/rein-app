//! Profile enumeration (blueprint §Components 6, rein D-153 r4).
//!
//! Sources, in order (D-022): the default config dir `~/.claude`; every
//! `accounts.<name>.config_dir` in `$HOME/.rein/accounts.json`; `~/.claude-personal` as
//! `personal` when it exists and no row has that name. Only `config_dir` is read; a row key this build does not know is ignored. rein-app never
//! writes accounts.json: its shape is closed and rein's `rein account` is its only writer
//! (rein D-153 addendum 2).

use std::fs;
use std::path::{Path, PathBuf};

/// One Claude profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub config_dir: PathBuf,
    pub settings_path: PathBuf,
    /// Whether `settings.json` exists right now.
    pub exists: bool,
}

impl Profile {
    fn new(name: &str, config_dir: PathBuf) -> Self {
        let settings_path = config_dir.join("settings.json");
        let exists = settings_path.is_file();
        Self {
            name: name.to_owned(),
            config_dir,
            settings_path,
            exists,
        }
    }
}

/// The result of enumeration: profiles plus non-fatal notes (skipped accounts entries).
#[derive(Debug, Default)]
pub struct Enumeration {
    pub profiles: Vec<Profile>,
    pub warnings: Vec<String>,
}

/// Enumerate the profiles under `home` (D-022): `default` (`~/.claude`), then every
/// accounts.json row with its `config_dir` authoritative for its name, then the built-in
/// `~/.claude-personal` as `personal` only when no row has that name. A directory is listed
/// once. accounts.json problems are warnings, never fatal, and never about a listed row.
pub fn enumerate(home: &Path) -> Enumeration {
    let mut out = Enumeration::default();
    let add = |out: &mut Enumeration, p: Profile| {
        if !out
            .profiles
            .iter()
            .any(|q| same_dir(&q.config_dir, &p.config_dir))
        {
            out.profiles.push(p);
        }
    };

    let claude = home.join(".claude");
    add(&mut out, Profile::new("default", claude.clone()));
    let mut personal_row = false;
    for (name, dir) in account_rows(home, &mut out.warnings) {
        if name == "default" {
            // Reserved: rein never writes it. Silent when it names ~/.claude anyway.
            if !same_dir(&dir, &claude) {
                out.warnings.push(format!(
                    "account \"default\": the name is reserved for {}; skipped",
                    claude.display()
                ));
            }
            continue;
        }
        personal_row |= name == "personal";
        add(&mut out, Profile::new(&name, dir));
    }
    let personal = home.join(".claude-personal");
    if !personal_row && personal.is_dir() {
        add(&mut out, Profile::new("personal", personal));
    }
    out
}

/// The usable rows of `$HOME/.rein/accounts.json` as `(name, config_dir)`. Only
/// `config_dir` is read; an unusable row is reported and left out.
fn account_rows(home: &Path, warnings: &mut Vec<String>) -> Vec<(String, PathBuf)> {
    let path = home.join(".rein").join("accounts.json");
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new(); // absent: an empty registry
    };
    let doc: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!(
                "{}: not valid JSON ({e}); named accounts skipped",
                path.display()
            ));
            return Vec::new();
        }
    };
    let Some(accounts) = doc.get("accounts").and_then(|a| a.as_object()) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for (name, entry) in accounts {
        // Names reach the terminal (E-4 security review P2): terminal safety only; rein's
        // accounts.py owns the grammar (D-022, rein D-153 addendum 2).
        let shown = name.escape_debug();
        if !is_terminal_safe(name) {
            warnings.push(format!(
                "account \"{shown}\": name is empty, longer than 64 bytes or not printable ASCII; skipped"
            ));
            continue;
        }
        match entry.get("config_dir").and_then(|d| d.as_str()) {
            Some(dir) if Path::new(dir).is_absolute() => {
                rows.push((name.clone(), PathBuf::from(dir)))
            }
            Some(dir) => warnings.push(format!(
                "account \"{shown}\": config_dir \"{}\" is not absolute; skipped",
                dir.escape_debug()
            )),
            None => warnings.push(format!("account \"{shown}\": no config_dir; skipped")),
        }
    }
    rows
}

/// Printable ASCII without space (0x21–0x7E), 1 to 64 bytes.
fn is_terminal_safe(name: &str) -> bool {
    (1..=64).contains(&name.len()) && name.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}
