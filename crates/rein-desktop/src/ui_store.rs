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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiFile {
    pub schema: u32,
    /// `simple` (the default) | `modal`.
    pub input_style: String,
    pub theme: Option<String>,
    pub font_size: Option<u32>,
    pub decorations: Option<bool>,
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
            "theme" => {
                refuse_config("theme.name")?;
                match value.as_str() {
                    Some(t) if THEMES.contains(&t) => d.theme = Some(t.into()),
                    _ => return Err(bad()),
                }
            }
            "font_size" => {
                refuse_config("font.size")?;
                match value.as_u64() {
                    Some(n @ 6..=72) => d.font_size = Some(n as u32),
                    _ => return Err(bad()),
                }
            }
            "decorations" => {
                refuse_config("window.decorations")?;
                d.decorations = Some(value.as_bool().ok_or_else(bad)?);
            }
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
        assert_eq!(
            s.view(&c).decorations,
            Effective {
                value: Value::from(false),
                source: "ui"
            }
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
            ("first_run_done", Value::from(1)),
        ] {
            assert_eq!(s.set(&c, f, &v).unwrap_err().kind, "bad_value", "{f}");
        }
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
