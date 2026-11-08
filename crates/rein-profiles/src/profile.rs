//! Profile enumeration (blueprint §Components 6, rein D-153 r4).
//!
//! Sources, in order: the default config dir `~/.claude`; `~/.claude-personal` if it
//! exists; every `accounts.<name>.config_dir` in `$HOME/.rein/accounts.json`. Only
//! `config_dir` is read from accounts.json — the rest of that file is opaque.

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

/// Enumerate the profiles under `home`. A profile whose config dir is already listed is
/// not repeated. accounts.json problems are warnings, never fatal.
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

    add(&mut out, Profile::new("default", home.join(".claude")));
    let personal = home.join(".claude-personal");
    if personal.is_dir() {
        add(&mut out, Profile::new("personal", personal));
    }

    let accounts_path = home.join(".rein").join("accounts.json");
    let Ok(text) = fs::read_to_string(&accounts_path) else {
        return out; // absent: no named accounts
    };
    let doc: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            out.warnings.push(format!(
                "{}: not valid JSON ({e}); named accounts skipped",
                accounts_path.display()
            ));
            return out;
        }
    };
    let Some(accounts) = doc.get("accounts").and_then(|a| a.as_object()) else {
        return out;
    };
    for (name, entry) in accounts {
        // E-4 security review P2: no duplicate built-in names, no terminal escapes.
        if matches!(name.as_str(), "default" | "personal") || !is_valid_profile_name(name) {
            out.warnings.push(format!(
                "account {:?}: name is reserved or not [A-Za-z0-9._-]; skipped",
                name
            ));
            continue;
        }
        match entry.get("config_dir").and_then(|d| d.as_str()) {
            Some(dir) if Path::new(dir).is_absolute() => {
                add(&mut out, Profile::new(name, PathBuf::from(dir)));
            }
            Some(dir) => out.warnings.push(format!(
                "account '{name}': config_dir '{dir}' is not absolute; skipped"
            )),
            None => out
                .warnings
                .push(format!("account '{name}': no config_dir; skipped")),
        }
    }
    out
}

/// `[A-Za-z0-9._-]{1,64}`
fn is_valid_profile_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}
