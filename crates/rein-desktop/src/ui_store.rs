//! The app's own `ui.json` (desktop.md §Data Model, D-034; E-40 / E-41): the choices made by
//! pointing and clicking — input style, theme, font size, title bar — and whether the
//! first-run choice was made. `$XDG_STATE_HOME/rein-app/ui.json` (Windows:
//! `%LOCALAPPDATA%\rein-app`), temp file + rename; an unreadable file is ignored and the
//! defaults apply. `config.toml` is the operator's and wins: a field it sets is shown as
//! "set in config.toml" and `ui_set` refuses it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{ConfigView, THEMES};
use crate::ipc::{IpcError, err};

pub const FILE_NAME: &str = "ui.json";
const MAX_BYTES: u64 = 64 * 1024;

/// The most projects "Don't ask for this project" keeps (V9 component 3).
pub const MAX_DONT_ASK: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiFile {
    pub schema: u32,
    /// `simple` (the default) | `modal`.
    pub input_style: String,
    pub theme: Option<String>,
    pub font_size: Option<u32>,
    pub decorations: Option<bool>,
    /// The tmux overlay (V6, D-050): on / off, and the status-line mode.
    pub tmux_overlay: Option<bool>,
    pub tmux_status: Option<String>,
    /// The tmux server tabs open on (V8): `app` | `user`.
    pub tmux_server: Option<String>,
    /// Every agent exited: close the project without asking (V9 component 5).
    pub close_when_agents_exit: Option<bool>,
    /// Projects whose out-of-date bar is not shown ("Don't ask for this project", V9 3).
    pub update_dont_ask: Vec<String>,
    /// V11 (E-66): closing a project or quitting with agents running does not ask.
    pub close_dont_ask: bool,
    pub first_run_done: bool,
}

impl Default for UiFile {
    fn default() -> Self {
        Self {
            schema: 1,
            input_style: "simple".into(),
            theme: None,
            font_size: None,
            decorations: None,
            tmux_overlay: None,
            tmux_status: None,
            tmux_server: None,
            close_when_agents_exit: None,
            update_dont_ask: Vec::new(),
            close_dont_ask: false,
            first_run_done: false,
        }
    }
}

/// One effective setting and where it comes from.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Effective {
    pub value: Value,
    /// `config` (config.toml sets it — read-only here) | `ui` | `default`.
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UiView {
    pub input_style: Effective,
    pub theme: Effective,
    pub font_size: Effective,
    pub decorations: Effective,
    pub tmux_overlay: Effective,
    pub tmux_status: Effective,
    pub tmux_server: Effective,
    pub close_when_agents_exit: Effective,
    pub update_dont_ask: Vec<String>,
    pub close_dont_ask: bool,
    pub first_run_done: bool,
}

pub struct UiStore {
    file: Option<PathBuf>,
    data: Mutex<UiFile>,
}

impl UiStore {
    pub fn open(state_dir: Option<PathBuf>) -> Self {
        let file = state_dir.map(|d| d.join(FILE_NAME));
        let data = file.as_deref().map(load).unwrap_or_default();
        Self {
            file,
            data: Mutex::new(data),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, UiFile> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Effective = config.toml if it sets the key, else ui.json, else the default.
    pub fn view(&self, cfg: &ConfigView) -> UiView {
        let d = self.lock();
        let set = |k: &str| cfg.set.iter().any(|s| s == k);
        let pick = |key: &str, from_cfg: Value, from_ui: Option<Value>, default: Value| {
            if set(key) {
                Effective {
                    value: from_cfg,
                    source: "config",
                }
            } else if let Some(v) = from_ui {
                Effective {
                    value: v,
                    source: "ui",
                }
            } else {
                Effective {
                    value: default,
                    source: "default",
                }
            }
        };
        let c = &cfg.config;
        UiView {
            input_style: Effective {
                value: Value::from(d.input_style.clone()),
                source: if d.input_style == UiFile::default().input_style && !d.first_run_done {
                    "default"
                } else {
                    "ui"
                },
            },
            theme: pick(
                "theme.name",
                Value::from(c.theme.name.clone()),
                d.theme.clone().map(Value::from),
                Value::from(c.theme.name.clone()),
            ),
            font_size: pick(
                "font.size",
                Value::from(c.font.size),
                d.font_size.map(Value::from),
                Value::from(c.font.size),
            ),
            decorations: pick(
                "window.decorations",
                Value::from(c.window.decorations),
                d.decorations.map(Value::from),
                Value::from(c.window.decorations),
            ),
            tmux_overlay: pick(
                "tmux.overlay",
                Value::from(c.tmux.overlay),
                d.tmux_overlay.map(Value::from),
                Value::from(c.tmux.overlay),
            ),
            tmux_status: pick(
                "tmux.status",
                Value::from(c.tmux.status.clone()),
                d.tmux_status.clone().map(Value::from),
                Value::from(c.tmux.status.clone()),
            ),
            tmux_server: pick(
                "tmux.server",
                Value::from(c.tmux.server.clone()),
                d.tmux_server.clone().map(Value::from),
                Value::from(c.tmux.server.clone()),
            ),
            close_when_agents_exit: pick(
                "project.close_when_agents_exit",
                Value::from(c.project.close_when_agents_exit),
                d.close_when_agents_exit.map(Value::from),
                Value::from(c.project.close_when_agents_exit),
            ),
            update_dont_ask: d.update_dont_ask.clone(),
            close_dont_ask: d.close_dont_ask,
            first_run_done: d.first_run_done,
        }
    }

    /// Set one field (type-checked); a field config.toml sets is refused.
    pub fn set(&self, cfg: &ConfigView, field: &str, value: &Value) -> Result<(), IpcError> {
        let in_config = |k: &str| cfg.set.iter().any(|s| s == k);
        let refuse_config = |k: &str| {
            if in_config(k) {
                Err(err(
                    "set_in_config",
                    format!("{k} is set in config.toml — change it there"),
                ))
            } else {
                Ok(())
            }
        };
        let bad = || err("bad_value", format!("not a valid value for {field}"));
        let mut live = self.lock();
        // Change a copy, save it, and only then adopt it: a failed save changes nothing.
        let mut d = live.clone();
        match field {
            "input_style" => match value.as_str() {
                Some(s @ ("simple" | "modal")) => d.input_style = s.into(),
                _ => return Err(bad()),
            },
            // `null` clears a choice: the default applies again (Reset font size).
            "theme" => {
                refuse_config("theme.name")?;
                d.theme = match value {
                    Value::Null => None,
                    v => match v.as_str() {
                        Some(t) if THEMES.contains(&t) => Some(t.into()),
                        _ => return Err(bad()),
                    },
                };
            }
            "font_size" => {
                refuse_config("font.size")?;
                d.font_size = match value {
                    Value::Null => None,
                    v => match v.as_u64() {
                        Some(n @ 6..=72) => Some(n as u32),
                        _ => return Err(bad()),
                    },
                };
            }
            "decorations" => {
                refuse_config("window.decorations")?;
                d.decorations = match value {
                    Value::Null => None,
                    v => Some(v.as_bool().ok_or_else(bad)?),
                };
            }
            "tmux_overlay" => {
                refuse_config("tmux.overlay")?;
                d.tmux_overlay = match value {
                    Value::Null => None,
                    v => Some(v.as_bool().ok_or_else(bad)?),
                };
            }
            "tmux_status" => {
                refuse_config("tmux.status")?;
                d.tmux_status = match value {
                    Value::Null => None,
                    v => match v.as_str() {
                        Some(m) if crate::tmux_overlay::STATUS_MODES.contains(&m) => Some(m.into()),
                        _ => return Err(bad()),
                    },
                };
            }
            "tmux_server" => {
                refuse_config("tmux.server")?;
                d.tmux_server = match value {
                    Value::Null => None,
                    v => match v.as_str() {
                        Some(m) if crate::tmux_server::SERVER_WORDS.contains(&m) => Some(m.into()),
                        _ => return Err(bad()),
                    },
                };
            }
            "close_when_agents_exit" => {
                refuse_config("project.close_when_agents_exit")?;
                d.close_when_agents_exit = match value {
                    Value::Null => None,
                    v => Some(v.as_bool().ok_or_else(bad)?),
                };
            }
            // The whole list: project paths, each passing the path text rule, at most
            // MAX_DONT_ASK, duplicates dropped.
            "update_dont_ask" => {
                let list = value.as_array().ok_or_else(bad)?;
                if list.len() > MAX_DONT_ASK {
                    return Err(bad());
                }
                let mut out: Vec<String> = Vec::new();
                for v in list {
                    let p = v.as_str().ok_or_else(bad)?;
                    crate::project_path::check(p).map_err(|_| bad())?;
                    if !out.iter().any(|o| o == p) {
                        out.push(p.into());
                    }
                }
                d.update_dont_ask = out;
            }
            "close_dont_ask" => d.close_dont_ask = value.as_bool().ok_or_else(bad)?,
            "first_run_done" => d.first_run_done = value.as_bool().ok_or_else(bad)?,
            _ => return Err(err("bad_field", "not a setting")),
        }
        self.save(&d)
            .map_err(|e| err("io", format!("cannot save ui.json: {e}")))?;
        *live = d;
        Ok(())
    }

    fn save(&self, d: &UiFile) -> std::io::Result<()> {
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

/// Anything unreadable, oversized, of another schema or holding an unknown value → defaults.
fn load(file: &Path) -> UiFile {
    let ok = std::fs::metadata(file).is_ok_and(|m| m.len() <= MAX_BYTES);
    let parsed: Option<UiFile> = ok
        .then(|| std::fs::read_to_string(file).ok())
        .flatten()
        .and_then(|t| serde_json::from_str(&t).ok());
    match parsed {
        Some(u)
            if u.schema == 1
                && matches!(u.input_style.as_str(), "simple" | "modal")
                && u.theme.as_deref().is_none_or(|t| THEMES.contains(&t))
                && u.font_size.is_none_or(|n| (6..=72).contains(&n)) =>
        {
            u
        }
        _ => UiFile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cfg(set: &[&str]) -> ConfigView {
        ConfigView {
            config: Config::default(),
            path: None,
            error: None,
            set: set.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn defaults_then_ui_choices_persist() {
        let t = tempfile::tempdir().unwrap();
        let s = UiStore::open(Some(t.path().to_path_buf()));
        let v = s.view(&cfg(&[]));
        assert_eq!(v.input_style.value, "simple");
        assert_eq!(v.theme.source, "default");
        assert!(!v.first_run_done);
        s.set(&cfg(&[]), "input_style", &Value::from("modal"))
            .unwrap();
        s.set(&cfg(&[]), "theme", &Value::from("nord")).unwrap();
        s.set(&cfg(&[]), "first_run_done", &Value::from(true))
            .unwrap();
        let again = UiStore::open(Some(t.path().to_path_buf())).view(&cfg(&[]));
        assert_eq!(again.input_style.value, "modal");
        assert_eq!(
            again.theme,
            Effective {
                value: Value::from("nord"),
                source: "ui"
            }
        );
        assert!(again.first_run_done);
    }

    #[test]
    fn config_toml_wins_and_its_fields_are_refused() {
        let t = tempfile::tempdir().unwrap();
        let s = UiStore::open(Some(t.path().to_path_buf()));
        let c = cfg(&["theme.name", "font.size"]);
        assert_eq!(
            s.set(&c, "theme", &Value::from("nord")).unwrap_err().kind,
            "set_in_config"
        );
        assert_eq!(
            s.set(&c, "font_size", &Value::from(18)).unwrap_err().kind,
            "set_in_config"
        );
        assert_eq!(s.view(&c).theme.source, "config");
        s.set(&c, "decorations", &Value::from(false)).unwrap();
        s.set(&cfg(&[]), "font_size", &Value::from(20)).unwrap();
        assert_eq!(s.view(&cfg(&[])).font_size.value, 20);
        s.set(&cfg(&[]), "font_size", &Value::Null).unwrap();
        assert_eq!(
            s.view(&cfg(&[])).font_size.source,
            "default",
            "null clears the choice"
        );
        assert_eq!(
            s.view(&c).decorations,
            Effective {
                value: Value::from(false),
                source: "ui"
            }
        );
        // The tmux overlay settings (V6): defaults, a choice, config.toml winning.
        assert_eq!(
            s.view(&c).tmux_status,
            Effective {
                value: Value::from("hidden"),
                source: "default"
            }
        );
        s.set(&c, "tmux_status", &Value::from("app")).unwrap();
        s.set(&c, "tmux_overlay", &Value::from(false)).unwrap();
        assert_eq!(s.view(&c).tmux_status.value, "app");
        assert_eq!(s.view(&c).tmux_overlay.value, false);
        let t = cfg(&["tmux.status"]);
        assert_eq!(
            s.set(&t, "tmux_status", &Value::from("keep"))
                .unwrap_err()
                .kind,
            "set_in_config"
        );
        assert_eq!(s.view(&t).tmux_status.source, "config");
        // E-57 (V8): the tmux server — rein-app's own by default, the user's by choice, and
        // config.toml winning.
        assert_eq!(
            s.view(&c).tmux_server,
            Effective {
                value: Value::from("app"),
                source: "default"
            }
        );
        s.set(&c, "tmux_server", &Value::from("user")).unwrap();
        assert_eq!(
            s.view(&c).tmux_server,
            Effective {
                value: Value::from("user"),
                source: "ui"
            }
        );
        let t = cfg(&["tmux.server"]);
        assert_eq!(
            s.set(&t, "tmux_server", &Value::from("app"))
                .unwrap_err()
                .kind,
            "set_in_config"
        ); // E-62 (V9 component 5): close when the agents exit — on by default, off by choice,
        // config.toml winning; only a boolean.
        assert_eq!(
            s.view(&c).close_when_agents_exit,
            Effective {
                value: Value::from(true),
                source: "default"
            }
        );
        s.set(&c, "close_when_agents_exit", &Value::from(false))
            .unwrap();
        assert_eq!(s.view(&c).close_when_agents_exit.value, false);
        assert_eq!(
            s.set(&c, "close_when_agents_exit", &Value::from("no"))
                .unwrap_err()
                .kind,
            "bad_value"
        );
        let t = cfg(&["project.close_when_agents_exit"]);
        assert_eq!(
            s.set(&t, "close_when_agents_exit", &Value::from(true))
                .unwrap_err()
                .kind,
            "set_in_config"
        );
    }

    #[test]
    fn only_listed_fields_with_valid_values() {
        let t = tempfile::tempdir().unwrap();
        let s = UiStore::open(Some(t.path().to_path_buf()));
        let c = cfg(&[]);
        for (f, v) in [
            ("input_style", Value::from("vim")),
            ("theme", Value::from("solarized-light")),
            ("font_size", Value::from(200)),
            ("decorations", Value::from("no")),
            ("tmux_overlay", Value::from("on")),
            ("tmux_status", Value::from("off")),
            ("tmux_server", Value::from("mine")),
            ("first_run_done", Value::from(1)),
            // E-63: the don't-ask list holds project paths only.
            ("update_dont_ask", Value::from("/home/me/p")),
            ("update_dont_ask", serde_json::json!(["relative/p"])),
            ("update_dont_ask", serde_json::json!(["/a\u{202e}b"])),
            ("update_dont_ask", serde_json::json!([7])),
        ] {
            assert_eq!(s.set(&c, f, &v).unwrap_err().kind, "bad_value", "{f}");
        }
        let many: Vec<String> = (0..=MAX_DONT_ASK).map(|i| format!("/p/{i}")).collect();
        assert_eq!(
            s.set(&c, "update_dont_ask", &serde_json::json!(many))
                .unwrap_err()
                .kind,
            "bad_value"
        );
        s.set(
            &c,
            "update_dont_ask",
            &serde_json::json!(["/home/me/p", "/home/me/p", "/home/me/q"]),
        )
        .unwrap();
        assert_eq!(s.view(&c).update_dont_ask, ["/home/me/p", "/home/me/q"]);
        assert_eq!(
            s.set(&c, "keys", &Value::from("x")).unwrap_err().kind,
            "bad_field"
        );
    }

    #[test]
    fn a_failed_save_changes_nothing() {
        let t = tempfile::tempdir().unwrap();
        let blocker = t.path().join("file");
        std::fs::write(&blocker, "").unwrap();
        let s = UiStore::open(Some(blocker.join("rein-app")));
        assert_eq!(
            s.set(&cfg(&[]), "input_style", &Value::from("modal"))
                .unwrap_err()
                .kind,
            "io"
        );
        assert_eq!(
            s.view(&cfg(&[])).input_style.value,
            "simple",
            "the live state is unchanged"
        );
    }

    #[test]
    fn an_unreadable_file_gives_the_defaults() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join(FILE_NAME), "not json").unwrap();
        assert_eq!(
            UiStore::open(Some(t.path().to_path_buf()))
                .view(&cfg(&[]))
                .input_style
                .value,
            "simple"
        );
        std::fs::write(
            t.path().join(FILE_NAME),
            r#"{"schema":1,"input_style":"vim"}"#,
        )
        .unwrap();
        assert_eq!(
            UiStore::open(Some(t.path().to_path_buf()))
                .view(&cfg(&[]))
                .input_style
                .value,
            "simple"
        );
    }
}
