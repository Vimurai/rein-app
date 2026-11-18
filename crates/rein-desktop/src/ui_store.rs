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

use crate::config::{ConfigView, PANEL_IDS, RAIL_WORDS, TAB_STATUS_WORDS, THEMES};
use crate::ipc::{IpcError, err};

pub const FILE_NAME: &str = "ui.json";
const MAX_BYTES: u64 = 64 * 1024;

/// The most projects "Don't ask for this project" keeps (V9 component 3).
pub const MAX_DONT_ASK: usize = 500;
/// The panel's width range in px, and its width until the user drags it (panels.md 2; widened
/// by status.md §Components 7, E-74 — the window also keeps it under half its own width).
pub const PANEL_WIDTH: std::ops::RangeInclusive<u32> = 320..=720;
pub const PANEL_WIDTH_DEFAULT: u32 = 400;

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
    /// V11 (E-66): closing a project with agents running does not ask (tab close only since
    /// V12 component 8 — quit never asks).
    pub close_dont_ask: bool,
    /// The side menu (panels.md §Data Model, E-60): the rail, `shown` | `hidden`, and the
    /// panels the user switched **off** (D-056 addendum 2 ruling 4, E-73) — a panel the file
    /// does not name is on, so a panel added later arrives on. `[panels]` in config.toml wins.
    pub panels_rail: Option<String>,
    pub panels_disabled: Option<Vec<String>>,
    /// The old key (E-60: the panels switched on). Read once by [`load`] into
    /// `panels_disabled` and never written again.
    #[serde(skip_serializing)]
    pub panels_enabled: Option<Vec<String>>,
    /// The panel's width once dragged, and the panel that was open last (or none).
    pub panel_width: Option<u32>,
    pub panel_open: Option<String>,
    /// The status mark on each tab and in the status bar (status.md §3, E-77): `shown` |
    /// `hidden`. `[tabs] status` in config.toml wins.
    pub tabs_status: Option<String>,
    pub first_run_done: bool,
    /// The versions answered "Later" on the update card (updates.md §Data Model): that version
    /// does not ask again.
    pub updates_asked: crate::updates::Asked,
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
            panels_rail: None,
            panels_disabled: None,
            panels_enabled: None,
            panel_width: None,
            panel_open: None,
            tabs_status: None,
            first_run_done: false,
            updates_asked: Default::default(),
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
    pub panels_rail: Effective,
    pub panels_enabled: Effective,
    pub panel_width: u32,
    pub panel_open: Option<String>,
    pub tabs_status: Effective,
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

    /// The versions answered "Later" (updates.md component 4).
    pub fn asked(&self) -> crate::updates::Asked {
        self.lock().updates_asked.clone()
    }

    /// Record `asked` (already checked by the caller) and save.
    pub fn set_asked(&self, asked: crate::updates::Asked) -> Result<(), IpcError> {
        let mut live = self.lock();
        let mut d = live.clone();
        d.updates_asked = asked;
        self.save(&d)
            .map_err(|e| err("io", format!("cannot save ui.json: {e}")))?;
        *live = d;
        Ok(())
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
            panels_rail: pick(
                "panels.rail",
                Value::from(c.panels.rail.clone()),
                d.panels_rail.clone().map(Value::from),
                Value::from(c.panels.rail.clone()),
            ),
            panels_enabled: pick(
                "panels.enabled",
                Value::from(c.panels.enabled.clone()),
                d.panels_disabled.as_ref().map(|off| {
                    Value::from(
                        PANEL_IDS
                            .iter()
                            .filter(|id| !off.iter().any(|o| o == *id))
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>(),
                    )
                }),
                Value::from(c.panels.enabled.clone()),
            ),
            tabs_status: pick(
                "tabs.status",
                Value::from(c.tabs.status.clone()),
                d.tabs_status.clone().map(Value::from),
                Value::from(c.tabs.status.clone()),
            ),
            // A hand-edited ui.json out of range falls back rather than reaching the page.
            panel_width: d
                .panel_width
                .filter(|w| PANEL_WIDTH.contains(w))
                .unwrap_or(PANEL_WIDTH_DEFAULT),
            panel_open: d
                .panel_open
                .clone()
                .filter(|p| PANEL_IDS.contains(&p.as_str())),
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
            "panels_rail" => {
                refuse_config("panels.rail")?;
                d.panels_rail = match value {
                    Value::Null => None,
                    v => match v.as_str() {
                        Some(w) if RAIL_WORDS.contains(&w) => Some(w.into()),
                        _ => return Err(bad()),
                    },
                };
            }
            "tabs_status" => {
                refuse_config("tabs.status")?;
                d.tabs_status = match value {
                    Value::Null => None,
                    v => match v.as_str() {
                        Some(w) if TAB_STATUS_WORDS.contains(&w) => Some(w.into()),
                        _ => return Err(bad()),
                    },
                };
            }
            // Settings sends the panels that are on (its switches are unchanged); the file
            // keeps the ones that are off.
            "panels_enabled" => {
                refuse_config("panels.enabled")?;
                d.panels_disabled = match value {
                    Value::Null => None,
                    v => {
                        let ids = v
                            .as_array()
                            .ok_or_else(bad)?
                            .iter()
                            .map(|x| x.as_str().map(String::from))
                            .collect::<Option<Vec<String>>>()
                            .ok_or_else(bad)?;
                        crate::config::panels_enabled_ok(&ids).map_err(|_| bad())?;
                        Some(
                            PANEL_IDS
                                .iter()
                                .filter(|id| !ids.iter().any(|i| i == *id))
                                .map(|id| id.to_string())
                                .collect(),
                        )
                    }
                };
            }
            "panel_width" => {
                d.panel_width = match value.as_u64().and_then(|w| u32::try_from(w).ok()) {
                    Some(w) if PANEL_WIDTH.contains(&w) => Some(w),
                    _ => return Err(bad()),
                };
            }
            "panel_open" => {
                d.panel_open = match value {
                    Value::Null => None,
                    v => match v.as_str() {
                        Some(id) if PANEL_IDS.contains(&id) => Some(id.into()),
                        _ => return Err(bad()),
                    },
                };
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

/// The panels that existed when ui.json stored the ones switched on (E-60's `panels_enabled`).
pub const LEGACY_PANEL_IDS: &[&str] = &["dashboard", "communication", "tasks"];

/// `panels_disabled` as read: an unknown id (a panel this version does not have) is dropped;
/// a repeated id refuses the file. A stored `panels_enabled` (no `panels_disabled` beside it)
/// is read once: of the legacy ids, those it lacks are off; any later panel is on.
fn panels_read(u: &mut UiFile) -> Result<(), ()> {
    let legacy = u.panels_enabled.take();
    if u.panels_disabled.is_none()
        && let Some(on) = legacy
    {
        u.panels_disabled = Some(
            LEGACY_PANEL_IDS
                .iter()
                .filter(|id| !on.iter().any(|o| o == *id))
                .map(|id| id.to_string())
                .collect(),
        );
    }
    if let Some(off) = &mut u.panels_disabled {
        if off.iter().enumerate().any(|(i, a)| off[..i].contains(a)) {
            return Err(());
        }
        off.retain(|id| PANEL_IDS.contains(&id.as_str()));
    }
    Ok(())
}

/// Anything unreadable, oversized, of another schema or holding an unknown value → defaults.
fn load(file: &Path) -> UiFile {
    let ok = std::fs::metadata(file).is_ok_and(|m| m.len() <= MAX_BYTES);
    let parsed: Option<UiFile> = ok
        .then(|| std::fs::read_to_string(file).ok())
        .flatten()
        .and_then(|t| serde_json::from_str::<UiFile>(&t).ok())
        .and_then(|mut u| panels_read(&mut u).ok().map(|()| u));
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

    /// D-056 addendum 2 ruling 4 (E-73): the file keeps the panels switched off, so a panel
    /// added later is on. Extends `the_panel_choices_are_closed_and_config_wins`.
    #[test]
    fn the_file_keeps_the_panels_switched_off() {
        let t = tempfile::tempdir().unwrap();
        let c = cfg(&[]);
        let s = UiStore::open(Some(t.path().to_path_buf()));
        s.set(
            &c,
            "panels_enabled",
            &serde_json::json!(["tasks", "attention"]),
        )
        .unwrap();
        let text = std::fs::read_to_string(t.path().join(FILE_NAME)).unwrap();
        let file: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            file["panels_disabled"],
            serde_json::json!(["dashboard", "communication"])
        );
        assert!(file.get("panels_enabled").is_none());
        // Round trip.
        let again = UiStore::open(Some(t.path().to_path_buf())).view(&c);
        assert_eq!(
            again.panels_enabled.value,
            serde_json::json!(["tasks", "attention"])
        );
        assert_eq!(again.panels_enabled.source, "ui");
        // An id this version does not know is dropped, not a refusal of the file.
        std::fs::write(
            t.path().join(FILE_NAME),
            r#"{"schema":1,"input_style":"modal","panels_disabled":["tasks","weather"]}"#,
        )
        .unwrap();
        let v = UiStore::open(Some(t.path().to_path_buf())).view(&c);
        assert_eq!(v.input_style.value, "modal");
        assert_eq!(
            v.panels_enabled.value,
            serde_json::json!(["dashboard", "communication", "attention"])
        );
        // A repeated id or a value that is not a list refuses the file (the defaults apply).
        for bad in [
            r#"{"schema":1,"input_style":"modal","panels_disabled":["tasks","tasks"]}"#,
            r#"{"schema":1,"input_style":"modal","panels_disabled":"tasks"}"#,
        ] {
            std::fs::write(t.path().join(FILE_NAME), bad).unwrap();
            let v = UiStore::open(Some(t.path().to_path_buf())).view(&c);
            assert_eq!(v.input_style.value, "simple", "{bad}");
            assert_eq!(
                v.panels_enabled.value,
                serde_json::json!(PANEL_IDS),
                "{bad}"
            );
        }
        // config.toml still wins.
        let mut from_cfg = cfg(&["panels.enabled"]);
        from_cfg.config.panels.enabled = vec!["dashboard".into()];
        std::fs::write(
            t.path().join(FILE_NAME),
            r#"{"schema":1,"panels_disabled":["dashboard"]}"#,
        )
        .unwrap();
        let s = UiStore::open(Some(t.path().to_path_buf()));
        let v = s.view(&from_cfg);
        assert_eq!(
            (v.panels_enabled.value, v.panels_enabled.source),
            (serde_json::json!(["dashboard"]), "config")
        );
        assert_eq!(
            s.set(&from_cfg, "panels_enabled", &serde_json::json!(["tasks"]))
                .unwrap_err()
                .kind,
            "set_in_config"
        );
    }

    /// A ui.json written before E-73 holds the panels switched on: read once, then gone.
    #[test]
    fn a_stored_panels_enabled_is_read_once() {
        let t = tempfile::tempdir().unwrap();
        let c = cfg(&[]);
        let file = t.path().join(FILE_NAME);
        let open = |text: &str| {
            std::fs::write(&file, text).unwrap();
            UiStore::open(Some(t.path().to_path_buf()))
        };
        // All three that existed then: all four on.
        let s = open(r#"{"schema":1,"panels_enabled":["dashboard","communication","tasks"]}"#);
        assert_eq!(
            s.view(&c).panels_enabled.value,
            serde_json::json!(PANEL_IDS)
        );
        // Only tasks: dashboard and communication off, attention on.
        let s = open(r#"{"schema":1,"panels_enabled":["tasks"]}"#);
        assert_eq!(
            s.view(&c).panels_enabled.value,
            serde_json::json!(["tasks", "attention"])
        );
        // The old key is gone after the next save, whatever is saved.
        s.set(&c, "panel_width", &Value::from(400)).unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(saved.get("panels_enabled").is_none());
        assert_eq!(
            saved["panels_disabled"],
            serde_json::json!(["dashboard", "communication"])
        );
        // Both keys: the new one is the answer.
        let s = open(r#"{"schema":1,"panels_enabled":["tasks"],"panels_disabled":[]}"#);
        assert_eq!(
            s.view(&c).panels_enabled.value,
            serde_json::json!(PANEL_IDS)
        );
    }

    /// E-77 (status.md §3): the tab status switch — kept, closed, and config.toml wins.
    #[test]
    fn the_tab_status_switch_is_kept_closed_and_config_wins() {
        let t = tempfile::tempdir().unwrap();
        let s = UiStore::open(Some(t.path().to_path_buf()));
        let c = cfg(&[]);
        assert_eq!(s.view(&c).tabs_status.value, Value::from("shown"));
        s.set(&c, "tabs_status", &Value::from("hidden")).unwrap();
        let again = UiStore::open(Some(t.path().to_path_buf())).view(&c);
        assert_eq!(
            (again.tabs_status.value, again.tabs_status.source),
            (Value::from("hidden"), "ui")
        );
        for value in [Value::from("off"), Value::from(false), Value::from(1)] {
            assert_eq!(
                s.set(&c, "tabs_status", &value).unwrap_err().kind,
                "bad_value",
                "{value}"
            );
        }
        let from_cfg = cfg(&["tabs.status"]);
        assert_eq!(
            s.set(&from_cfg, "tabs_status", &Value::from("shown"))
                .unwrap_err()
                .kind,
            "set_in_config"
        );
        assert_eq!(s.view(&from_cfg).tabs_status.source, "config");
        s.set(&c, "tabs_status", &Value::Null).unwrap();
        assert_eq!(s.view(&c).tabs_status.source, "default");
    }

    /// E-60 (panels.md §Data Model): the side menu's choices and the panel's own state.
    #[test]
    fn the_panel_choices_are_closed_and_config_wins() {
        let t = tempfile::tempdir().unwrap();
        let s = UiStore::open(Some(t.path().to_path_buf()));
        let c = cfg(&[]);
        let v = s.view(&c);
        assert_eq!(v.panels_rail.value, Value::from("shown"));
        assert_eq!(v.panels_enabled.value, serde_json::json!(PANEL_IDS));
        assert_eq!((v.panel_width, v.panel_open), (PANEL_WIDTH_DEFAULT, None));
        s.set(&c, "panels_rail", &Value::from("hidden")).unwrap();
        s.set(&c, "panels_enabled", &serde_json::json!(["tasks"]))
            .unwrap();
        s.set(&c, "panel_width", &Value::from(480)).unwrap();
        s.set(&c, "panel_open", &Value::from("communication"))
            .unwrap();
        let again = UiStore::open(Some(t.path().to_path_buf())).view(&c);
        assert_eq!(again.panels_rail.value, Value::from("hidden"));
        assert_eq!(again.panels_enabled.value, serde_json::json!(["tasks"]));
        assert_eq!(again.panel_width, 480);
        assert_eq!(again.panel_open.as_deref(), Some("communication"));
        s.set(&c, "panel_open", &Value::Null).unwrap();
        assert_eq!(s.view(&c).panel_open, None);
        for (field, value) in [
            ("panels_rail", Value::from("left")),
            ("panels_enabled", serde_json::json!(["shell"])),
            ("panels_enabled", serde_json::json!(["tasks", "tasks"])),
            ("panels_enabled", Value::from("tasks")),
            ("panel_width", Value::from(319)),
            ("panel_width", Value::from(721)),
            ("panel_width", Value::from(4_294_967_656_u64)),
            ("panel_open", Value::from("shell")),
        ] {
            assert_eq!(
                s.set(&c, field, &value).unwrap_err().kind,
                "bad_value",
                "{field} {value}"
            );
        }
        let t2 = cfg(&["panels.rail", "panels.enabled"]);
        assert_eq!(
            s.set(&t2, "panels_rail", &Value::from("shown"))
                .unwrap_err()
                .kind,
            "set_in_config"
        );
        assert_eq!(s.view(&t2).panels_enabled.source, "config");
        // A hand-edited ui.json out of range never reaches the page.
        std::fs::write(
            t.path().join(FILE_NAME),
            r#"{"schema":1,"panel_width":9000,"panel_open":"shell"}"#,
        )
        .unwrap();
        let odd = UiStore::open(Some(t.path().to_path_buf())).view(&c);
        assert_eq!(
            (odd.panel_width, odd.panel_open),
            (PANEL_WIDTH_DEFAULT, None)
        );
    }
}
