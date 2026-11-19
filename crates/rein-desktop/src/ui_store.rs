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
/// `[notifications] stuck_after_min`'s range (status.md §Data Model).
pub const STUCK_AFTER_MIN: std::ops::RangeInclusive<u32> = 3..=120;

/// The notification settings in effect (config.toml, then ui.json, then the defaults), as
/// the notifier and the `stuck` rule read them (E-78).
pub fn notify_settings(v: &UiView) -> (crate::notify::Settings, u32) {
    let enabled = v.notifications_enabled.value.as_bool().unwrap_or(true);
    let events = v
        .notifications_events
        .value
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().and_then(crate::notify::Event::from_name))
                .collect()
        })
        .unwrap_or_else(|| crate::notify::Event::ALL.to_vec());
    let stuck_min = v
        .notifications_stuck_after_min
        .value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|m| STUCK_AFTER_MIN.contains(m))
        .unwrap_or(10);
    (
        crate::notify::Settings { enabled, events },
        stuck_min.saturating_mul(60),
    )
}

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
    /// `[notifications]` (status.md §Data Model, E-78): config.toml wins on each.
    pub notifications_enabled: Option<bool>,
    pub notifications_system: Option<bool>,
    pub notifications_events: Option<Vec<String>>,
    pub notifications_stuck_after_min: Option<u32>,
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
            notifications_enabled: None,
            notifications_system: None,
            notifications_events: None,
            notifications_stuck_after_min: None,
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
    pub notifications_enabled: Effective,
    pub notifications_system: Effective,
    pub notifications_events: Effective,
    pub notifications_stuck_after_min: Effective,
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
            notifications_enabled: pick(
                "notifications.enabled",
                Value::from(c.notifications.enabled),
                d.notifications_enabled.map(Value::from),
                Value::from(c.notifications.enabled),
            ),
            notifications_system: pick(
                "notifications.system",
                Value::from(c.notifications.system),
                d.notifications_system.map(Value::from),
                Value::from(c.notifications.system),
            ),
            notifications_events: pick(
                "notifications.events",
                Value::from(c.notifications.events.clone()),
                d.notifications_events
                    .clone()
                    .filter(|e| crate::config::events_ok(e).is_ok())
                    .map(Value::from),
                Value::from(c.notifications.events.clone()),
            ),
            notifications_stuck_after_min: pick(
                "notifications.stuck_after_min",
                Value::from(c.notifications.stuck_after_min),
                d.notifications_stuck_after_min
                    .filter(|m| STUCK_AFTER_MIN.contains(m))
                    .map(Value::from),
                Value::from(c.notifications.stuck_after_min),
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
            "notifications_enabled" => {
                refuse_config("notifications.enabled")?;
                d.notifications_enabled = match value {
                    Value::Null => None,
                    v => Some(v.as_bool().ok_or_else(bad)?),
                };
            }
            "notifications_system" => {
                refuse_config("notifications.system")?;
                d.notifications_system = match value {
                    Value::Null => None,
                    v => Some(v.as_bool().ok_or_else(bad)?),
                };
            }
            "notifications_events" => {
                refuse_config("notifications.events")?;
                d.notifications_events = match value {
                    Value::Null => None,
                    v => {
                        let names = v
                            .as_array()
                            .ok_or_else(bad)?
                            .iter()
                            .map(|x| x.as_str().map(String::from))
                            .collect::<Option<Vec<String>>>()
                            .ok_or_else(bad)?;
                        crate::config::events_ok(&names).map_err(|_| bad())?;
                        Some(names)
                    }
                };
            }
            "notifications_stuck_after_min" => {
                refuse_config("notifications.stuck_after_min")?;
                d.notifications_stuck_after_min = match value {
                    Value::Null => None,
                    v => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
                        Some(n) if STUCK_AFTER_MIN.contains(&n) => Some(n),
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
#[path = "ui_store_tests.rs"]
mod tests;
