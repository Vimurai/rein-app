//! `Config` (desktop.md §Components 4, §Data Model `ConfigFile`, §Security 4, D-033 ruling 5):
//! the operator's `config.toml` — font, theme, window, keys, and on Windows the WSL distro.
//!
//! Read-only input: at most 256 KiB; an unknown key, a colour that is not `#rrggbb`, or a
//! binding to a name outside the closed `Action` list is an error naming file, line and
//! reason, and the previous good config stays active. The file can name no program, no file
//! and no URL. It is created with commented defaults on first run and never written again;
//! a save is picked up by polling its modification time and size (D-043).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::actions::{self, ActionArg};

pub const MAX_BYTES: u64 = 256 * 1024;
pub const FILE_NAME: &str = "config.toml";
/// How often the file is checked for a save.
pub const POLL: Duration = Duration::from_millis(500);

pub const THEMES: &[&str] = &[
    "gruvbox-dark",
    "tokyonight",
    "catppuccin-mocha",
    "nord",
    "solarized-dark",
    "github-light",
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub font: Font,
    pub theme: Theme,
    pub window: Window,
    pub keys: Keys,
    pub tmux: Tmux,
    pub project: Project,
    pub panels: Panels,
    pub tabs: Tabs,
    pub updates: Updates,
    pub notifications: Notifications,
    pub wsl: Wsl,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Font {
    /// CSS font family list; empty = the platform's default monospace list.
    pub family: String,
    pub size: u32,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: String::new(),
            size: 14,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Theme {
    pub name: String,
    /// Overrides: `fg`, `bg`, `accent`, `color0` … `color15` → `#rrggbb`.
    pub custom: BTreeMap<String, String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // D-049 addendum 1 (E-48): the default theme.
            name: "catppuccin-mocha".into(),
            custom: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Window {
    pub decorations: bool,
    pub opacity: f64,
    /// `dashboard` | `last`.
    pub start: String,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            decorations: true,
            opacity: 1.0,
            start: "dashboard".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Keys {
    /// TERMINAL → NORMAL chord.
    pub escape: Vec<String>,
    pub leader: String,
    /// NORMAL bindings added to or replacing the default keymap: `"g t" = "tab.next"`,
    /// `"<leader> 1" = "tab.goto 1"`.
    pub normal: BTreeMap<String, String>,
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            escape: vec!["C-\\".into(), "C-n".into()],
            leader: "Space".into(),
            normal: BTreeMap::new(),
        }
    }
}

/// The tmux overlay (desktop.md §Look and feel v2 V6, D-050).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Tmux {
    /// The window styles the tmux session it shows while attached.
    pub overlay: bool,
    /// `hidden` | `app` | `keep` (`tmux_overlay::STATUS_MODES`).
    pub status: String,
    /// `app` | `user` (`tmux_server::SERVER_WORDS`, V8): rein-app's own tmux server, or the
    /// user's. Applies to tabs opened after a change.
    pub server: String,
}

impl Default for Tmux {
    fn default() -> Self {
        Self {
            overlay: true,
            status: "hidden".into(),
            server: "app".into(),
        }
    }
}

/// A project's life in the window (desktop.md §Look and feel v2 V9, D-057).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Project {
    /// When every agent of a project has exited, close the project without asking
    /// (`rein start --kill --yes`, then the tab closes); false: ask with the bar (V9 4).
    pub close_when_agents_exit: bool,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            close_when_agents_exit: true,
        }
    }
}

/// The panel ids the app knows (panels.md §Components 7, D-056): the compiled-in descriptors,
/// in rail order. A `[panels] enabled` entry or a `panel.toggle` binding names one of these.
pub const PANEL_IDS: &[&str] = &["dashboard", "communication", "tasks", "attention"];
/// `[panels] rail`.
pub const RAIL_WORDS: &[&str] = &["shown", "hidden"];

/// The side menu (panels.md §Data Model, D-056).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Panels {
    /// `shown` | `hidden`: the rail at the window's left edge. Hidden, panels still open
    /// from the menu and keys.
    pub rail: String,
    /// The panels offered, by id (each in [`PANEL_IDS`], no repeats); rail order is
    /// [`PANEL_IDS`]' order whatever the order here.
    pub enabled: Vec<String>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            rail: "shown".into(),
            enabled: PANEL_IDS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// `[tabs] status`.
pub const TAB_STATUS_WORDS: &[&str] = &["shown", "hidden"];

/// The tab bar (status.md §Components 3, D-062).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Tabs {
    /// `shown` | `hidden`: each tab's status mark and the status bar's status segment.
    pub status: String,
}

impl Default for Tabs {
    fn default() -> Self {
        Self {
            status: "shown".into(),
        }
    }
}

/// A `[panels] enabled` list: known ids, no repeats.
pub fn panels_enabled_ok(ids: &[String]) -> Result<(), String> {
    for (i, id) in ids.iter().enumerate() {
        if !PANEL_IDS.contains(&id.as_str()) {
            return Err(format!("{id:?} is not a panel — {}", PANEL_IDS.join(", ")));
        }
        if ids[..i].contains(id) {
            return Err(format!("{id:?} is listed twice"));
        }
    }
    Ok(())
}

/// The update watch (updates.md §Data Model, D-065).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Updates {
    /// false: no background check at all (the Update dialog still checks when opened).
    pub check: bool,
    /// Minutes between checks for a newer published Rein, 10–1440.
    pub interval_min: u32,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            check: true,
            interval_min: 30,
        }
    }
}

/// Notifications (status.md §Data Model, §Components 6, D-062; E-76 `stuck_after_min`, E-78
/// the rest). `system` is read and does nothing until the system-notification task (E-79).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Notifications {
    /// false: no notification is raised (the tab marks and "Needs you" stay).
    pub enabled: bool,
    /// The operating system's own notification (E-79).
    pub system: bool,
    /// The events raised, from the closed list `crate::notify::EVENT_NAMES`.
    pub events: Vec<String>,
    /// An agent running, a task in progress and no output for this many minutes: `stuck`.
    pub stuck_after_min: u32,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            system: true,
            events: crate::notify::EVENT_NAMES
                .iter()
                .map(|e| e.to_string())
                .collect(),
            stuck_after_min: 10,
        }
    }
}

/// The events list is from the closed list, each once.
pub fn events_ok(events: &[String]) -> Result<(), String> {
    for (i, e) in events.iter().enumerate() {
        if !crate::notify::EVENT_NAMES.contains(&e.as_str()) {
            return Err(format!(
                "{e:?} is not an event — {}",
                crate::notify::EVENT_NAMES.join(", ")
            ));
        }
        if events[..i].contains(e) {
            return Err(format!("{e:?} is named twice"));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Wsl {
    /// Windows only: the distro the window uses; unset = WSL's default.
    pub distro: Option<String>,
}

/// The commented defaults written on first run. Uncommenting a `# key = value` line (one
/// `#`, one space) gives the built-in default; indented `#   …` lines are examples.
pub const DEFAULTS_FILE: &str = r##"# rein-app window configuration (desktop.md). Edit and save: the window reloads it.
# The app never writes this file again. Each `# key = value` line shows the built-in default;
# remove the `# ` to change it. Indented lines are examples.

# [font]
# family = ""             # empty: the platform's monospace fonts
# size = 14               # 6 – 72
#   family = "JetBrains Mono, monospace"

# [theme]
# name = "catppuccin-mocha" # catppuccin-mocha | gruvbox-dark | tokyonight | nord | solarized-dark | github-light
#   [theme.custom]        # override any colour: fg, bg, accent, color0 … color15
#   accent = "#fabd2f"

# [window]
# decorations = true      # false: no title bar (tiling window managers)
# opacity = 1.0           # 0.6 – 1.0, where the compositor supports it; read at start
# start = "dashboard"     # dashboard | last (reopen the last project)

# [keys]
# escape = ["C-\\", "C-n"]
# leader = "Space"
#   [keys.normal]         # NORMAL-mode bindings, added or replacing: "keys" = "action"
#   "g t" = "tab.next"
#   "<leader> 1" = "tab.goto 1"

# [tmux]                  # the window's look on the tmux session a tab shows, while it is open
# overlay = true          # false: tmux looks as your own ~/.tmux.conf has it (never touched)
# status = "hidden"       # hidden | app (the window list in the window's colours) | keep (yours)

# [project]
# close_when_agents_exit = true # every agent exited: close the project (false: ask)

# [panels]                # the side menu: read-only panels beside the terminal
# rail = "shown"          # shown | hidden (panels still open from the menu and keys)
# enabled = ["dashboard", "communication", "tasks", "attention"]

# [tabs]
# status = "shown"        # shown | hidden: the status mark on each tab and in the status bar

# [updates]               # the window watches for a newer Rein by itself
# check = true            # false: only the Update dialog checks
# interval_min = 30       # 10 – 1440 minutes between checks for a published release

# [notifications]         # a project that needs you, stops, is stuck or ends: a toast, the taskbar
# enabled = true          # false: no notifications (the tab marks and "Needs you" stay)
# system = true           # the system's own notification when the window is not in front
# events = ["needs_you", "stopped", "stuck", "ended", "handoff"]
# stuck_after_min = 10    # 3 – 120: an agent runs, a task is in progress, no output this long

# [wsl]                   # Windows only
#   distro = "Ubuntu"     # unset: WSL's default distro
"##;

/// A refusal: where and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub file: String,
    pub line: Option<usize>,
    pub reason: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(l) => write!(f, "{}:{l}: {}", self.file, self.reason),
            None => write!(f, "{}: {}", self.file, self.reason),
        }
    }
}

/// Parse and validate `text` (the contents of `file`).
pub fn parse(file: &str, text: &str) -> Result<Config, ConfigError> {
    let fail = |line: Option<usize>, reason: String| ConfigError {
        file: file.to_string(),
        line,
        reason,
    };
    if text.len() as u64 > MAX_BYTES {
        return Err(fail(None, "larger than 256 KiB".into()));
    }
    let config: Config = toml::from_str(text).map_err(|e| {
        let line = e.span().map(|s| line_at(text, s.start));
        fail(line, e.message().trim().to_string())
    })?;
    validate(&config)
        .map_err(|(section, key, reason)| fail(line_of(text, section, &key), reason))?;
    Ok(config)
}

/// 1-based line of byte offset `at`.
fn line_at(text: &str, at: usize) -> usize {
    text[..at.min(text.len())].matches('\n').count() + 1
}

/// The line that sets `key` inside `[section]` (best effort, for semantic errors).
fn line_of(text: &str, section: &str, key: &str) -> Option<usize> {
    let mut current = String::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(h) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = h.trim().to_string();
            continue;
        }
        let name = line
            .split('=')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"');
        if current == section && !line.starts_with('#') && name == key {
            return Some(i + 1);
        }
    }
    None
}

type Refusal = (&'static str, String, String);

fn validate(c: &Config) -> Result<(), Refusal> {
    let r =
        |section: &'static str, key: &str, reason: String| Err((section, key.to_string(), reason));
    if !(6..=72).contains(&c.font.size) {
        return r(
            "font",
            "size",
            format!("font size {} is outside 6–72", c.font.size),
        );
    }
    if !font_family_ok(&c.font.family) {
        return r(
            "font",
            "family",
            "font family may hold letters, digits, spaces and , . ' - _ only (at most 128)".into(),
        );
    }
    if !THEMES.contains(&c.theme.name.as_str()) {
        return r(
            "theme",
            "name",
            format!(
                "unknown theme {:?} — one of {}",
                c.theme.name,
                THEMES.join(", ")
            ),
        );
    }
    for (k, v) in &c.theme.custom {
        if !custom_key_ok(k) {
            return r(
                "theme.custom",
                k,
                format!("unknown colour {k:?} — fg, bg, accent or color0 … color15"),
            );
        }
        if !is_hex_colour(v) {
            return r("theme.custom", k, format!("colour {v:?} is not #rrggbb"));
        }
    }
    if !(0.6..=1.0).contains(&c.window.opacity) {
        return r(
            "window",
            "opacity",
            format!("opacity {} is outside 0.6–1.0", c.window.opacity),
        );
    }
    if !matches!(c.window.start.as_str(), "dashboard" | "last") {
        return r(
            "window",
            "start",
            format!("start {:?} — dashboard or last", c.window.start),
        );
    }
    if c.keys.escape.is_empty()
        || c.keys.escape.len() > 4
        || !c.keys.escape.iter().all(|k| key_ok(k))
    {
        return r(
            "keys",
            "escape",
            "escape is 1–4 key names, such as [\"C-\\\\\", \"C-n\"]".into(),
        );
    }
    if !key_ok(&c.keys.leader) {
        return r(
            "keys",
            "leader",
            format!("leader {:?} is not a key name", c.keys.leader),
        );
    }
    for (keys, action) in &c.keys.normal {
        let tokens: Vec<&str> = keys.split(' ').collect();
        if tokens.len() > 4 || !tokens.iter().all(|t| *t == "<leader>" || key_ok(t)) {
            return r(
                "keys.normal",
                keys,
                format!("{keys:?} is not a key sequence"),
            );
        }
        if let Err(why) = binding_ok(action) {
            return r("keys.normal", keys, why);
        }
    }
    if !crate::tmux_overlay::STATUS_MODES.contains(&c.tmux.status.as_str()) {
        return r(
            "tmux",
            "status",
            format!("status {:?} — hidden, app or keep", c.tmux.status),
        );
    }
    if !crate::tmux_server::SERVER_WORDS.contains(&c.tmux.server.as_str()) {
        return r(
            "tmux",
            "server",
            format!("server {:?} — app or user", c.tmux.server),
        );
    }
    if !RAIL_WORDS.contains(&c.panels.rail.as_str()) {
        return r(
            "panels",
            "rail",
            format!("rail {:?} — shown or hidden", c.panels.rail),
        );
    }
    if let Err(why) = panels_enabled_ok(&c.panels.enabled) {
        return r("panels", "enabled", why);
    }
    if !TAB_STATUS_WORDS.contains(&c.tabs.status.as_str()) {
        return r(
            "tabs",
            "status",
            format!("status {:?} — shown or hidden", c.tabs.status),
        );
    }
    if !(10..=1440).contains(&c.updates.interval_min) {
        return r(
            "updates",
            "interval_min",
            format!("interval_min {} is outside 10–1440", c.updates.interval_min),
        );
    }
    if !(3..=120).contains(&c.notifications.stuck_after_min) {
        return r(
            "notifications",
            "stuck_after_min",
            format!(
                "stuck_after_min {} is outside 3–120",
                c.notifications.stuck_after_min
            ),
        );
    }
    if let Err(why) = events_ok(&c.notifications.events) {
        return r("notifications", "events", why);
    }
    if let Some(d) = &c.wsl.distro
        && crate::wsl_host::check_name(d).is_err()
    {
        return r("wsl", "distro", format!("{d:?} is not a WSL distro name"));
    }
    Ok(())
}

fn font_family_ok(s: &str) -> bool {
    s.len() <= 128
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | ',' | '.' | '\'' | '-' | '_'))
}

fn custom_key_ok(k: &str) -> bool {
    matches!(k, "fg" | "bg" | "accent")
        || k.strip_prefix("color")
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| n < 16 && k == format!("color{n}"))
}

pub fn is_hex_colour(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// A key token as the window names keys: `a`, `:`, `C-n`, `C-S-p`, `M-1`, `Space`, `F5` ….
pub fn key_ok(token: &str) -> bool {
    let mut rest = token;
    while let Some(r) = ["C-", "M-", "D-", "S-"]
        .iter()
        .find_map(|m| rest.strip_prefix(m))
    {
        rest = r;
    }
    const NAMED: &[&str] = &[
        "Space", "Enter", "Esc", "Tab", "BS", "Del", "Up", "Down", "Left", "Right", "Home", "End",
        "PageUp", "PageDown", "Insert",
    ];
    let single = rest.chars().count() == 1 && rest.chars().all(|c| c.is_ascii_graphic());
    let function = rest
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=12).contains(&n));
    single || function || NAMED.contains(&rest)
}

/// `"action"` or `"action arg"` with the argument the action takes.
fn binding_ok(binding: &str) -> Result<(), String> {
    let mut parts = binding.splitn(2, ' ');
    let name = parts.next().unwrap_or("");
    let arg = parts.next();
    let Some(action) = actions::find(name) else {
        return Err(format!("{name:?} is not an action"));
    };
    let ok = match (action.arg, arg) {
        (_, None) => true,
        (ActionArg::TabNumber, Some(n)) => {
            matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        }
        (ActionArg::ThemeName, Some(t)) => THEMES.contains(&t),
        (ActionArg::InputStyle, Some(s)) => matches!(s, "simple" | "modal"),
        (ActionArg::PanelId, Some(p)) => PANEL_IDS.contains(&p),
        // A binding cannot name a file or a directory (§Security 4).
        (ActionArg::Dir, Some(_)) | (ActionArg::None, Some(_)) => false,
    };
    if ok {
        Ok(())
    } else {
        Err(format!("{binding:?}: {name} does not take that argument"))
    }
}

/// What the web side gets (`config_get`, `config_changed`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigView {
    pub config: Config,
    /// Where the file is, shown and copied by `:config`.
    pub path: Option<String>,
    /// The last refusal (the previous good config is the one in `config`).
    pub error: Option<String>,
    /// The keys config.toml itself sets among those Settings can also set (`theme.name`,
    /// `font.size`, `window.decorations`, `tmux.overlay`, `tmux.status`, `tmux.server`): those win over
    /// `ui.json` and are read-only in Settings (D-034).
    pub set: Vec<String>,
}

/// Which of the Settings-visible keys `text` sets (it has already parsed).
pub fn settings_keys_set(text: &str) -> Vec<String> {
    let Ok(t) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    [
        ("theme", "name"),
        ("font", "size"),
        ("window", "decorations"),
        ("tmux", "overlay"),
        ("tmux", "status"),
        ("tmux", "server"),
        ("project", "close_when_agents_exit"),
        ("panels", "rail"),
        ("panels", "enabled"),
        ("tabs", "status"),
        ("notifications", "enabled"),
        ("notifications", "system"),
        ("notifications", "events"),
        ("notifications", "stuck_after_min"),
    ]
    .iter()
    .filter(|(table, key)| t.get(*table).and_then(|v| v.get(*key)).is_some())
    .map(|(table, key)| format!("{table}.{key}"))
    .collect()
}

/// The loaded config and how to notice a save.
pub struct ConfigService {
    path: Option<PathBuf>,
    state: Mutex<(ConfigView, Option<(SystemTime, u64)>)>,
}

impl ConfigService {
    /// Load `dir/config.toml`, creating it with the commented defaults if it is missing.
    pub fn open(dir: Option<PathBuf>) -> Self {
        let path = dir.map(|d| d.join(FILE_NAME));
        // First run: the commented defaults, once. A failure is shown, not swallowed — the
        // window still runs on the built-in defaults.
        let mut created: std::io::Result<()> = Ok(());
        if let Some(p) = &path
            && !p.exists()
        {
            created = p
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| write_new(p, DEFAULTS_FILE));
        }
        let view = ConfigView {
            config: Config::default(),
            path: path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            error: None,
            set: Vec::new(),
        };
        let s = Self {
            path,
            state: Mutex::new((view, None)),
        };
        s.reload();
        if let Err(e) = created {
            let mut st = s.state.lock().unwrap_or_else(|e| e.into_inner());
            st.0.error = Some(format!(
                "cannot create {FILE_NAME}: {e} — using the built-in defaults"
            ));
        }
        s
    }

    pub fn view(&self) -> ConfigView {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .0
            .clone()
    }

    /// Re-read the file; a refusal keeps the previous config and records the error.
    pub fn reload(&self) -> ConfigView {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = &self.path {
            st.1 = stamp(p);
            match read_bounded(p) {
                Ok(text) => match parse(FILE_NAME, &text) {
                    Ok(c) => {
                        st.0.config = c;
                        st.0.error = None;
                        st.0.set = settings_keys_set(&text);
                    }
                    Err(e) => st.0.error = Some(e.to_string()),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    st.0.config = Config::default();
                    st.0.error = None;
                    st.0.set = Vec::new();
                }
                Err(e) => st.0.error = Some(format!("{FILE_NAME}: {e}")),
            }
        }
        st.0.clone()
    }

    /// `Some(view)` when the file changed since the last read (and was re-read).
    pub fn reload_if_changed(&self) -> Option<ConfigView> {
        let p = self.path.as_ref()?;
        let now = stamp(p);
        let before = self.state.lock().unwrap_or_else(|e| e.into_inner()).1;
        (now != before).then(|| self.reload())
    }
}

fn stamp(p: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

fn read_bounded(p: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut buf = String::new();
    std::fs::File::open(p)?
        .take(MAX_BYTES + 1)
        .read_to_string(&mut buf)?;
    Ok(buf)
}

/// Create `p` with `text`; never overwrites (the file is the operator's).
fn write_new(p: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(p)?
        .write_all(text.as_bytes())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
