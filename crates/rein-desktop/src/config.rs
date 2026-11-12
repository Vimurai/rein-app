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
            name: "gruvbox-dark".into(),
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
# name = "gruvbox-dark"   # gruvbox-dark | tokyonight | catppuccin-mocha | nord | solarized-dark | github-light
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
    /// `font.size`, `window.decorations`): those win over `ui.json` and are read-only in
    /// Settings (D-034).
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
mod tests {
    use super::*;

    /// The defaults file with every `# key = value` / `# [section]` line uncommented.
    fn uncommented() -> String {
        DEFAULTS_FILE
            .lines()
            .map(|l| match l.strip_prefix("# ") {
                // `#   …` (indented) lines are examples, not defaults.
                Some(rest)
                    if !rest.starts_with(' ') && rest.starts_with('[')
                        || !rest.starts_with(' ')
                            && rest.split('=').next().is_some_and(|k| {
                                let k = k.trim();
                                !k.is_empty()
                                    && k.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                                    && rest.contains('=')
                            }) =>
                {
                    rest.split("   #")
                        .next()
                        .unwrap_or(rest)
                        .trim_end()
                        .to_string()
                }
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_defaults_file_is_the_built_in_defaults() {
        assert_eq!(parse(FILE_NAME, DEFAULTS_FILE).unwrap(), Config::default());
        let live = uncommented();
        assert!(
            live.contains("size = 14") && live.contains("[keys]"),
            "{live}"
        );
        assert_eq!(
            parse(FILE_NAME, &live).unwrap(),
            Config::default(),
            "{live}"
        );
    }

    #[test]
    fn each_refusal_names_file_line_and_reason() {
        let cases = [
            ("[font]\nbogus = 1\n", 2, "unknown field"),
            (
                "[theme]\nname = \"nord\"\n[theme.custom]\naccent = \"red\"\n",
                4,
                "not #rrggbb",
            ),
            (
                "[theme.custom]\ncolor16 = \"#000000\"\n",
                2,
                "unknown colour",
            ),
            (
                "[keys.normal]\n\"g x\" = \"shell.exec\"\n",
                2,
                "is not an action",
            ),
            (
                "[keys.normal]\n\"g o\" = \"project.open /etc\"\n",
                2,
                "does not take that argument",
            ),
            ("[window]\nopacity = 0.2\n", 2, "outside 0.6–1.0"),
            (
                "[font]\nfamily = \"x; } body { background: url(http://e) }\"\n",
                2,
                "font family",
            ),
            ("[theme]\nname = \"solarized-light\"\n", 2, "unknown theme"),
            ("[wsl]\ndistro = \"a b\"\n", 2, "not a WSL distro name"),
            ("[keys]\nescape = []\n", 2, "escape is"),
            ("this is not toml\n", 1, "expected"),
        ];
        for (text, line, reason) in cases {
            let e = parse(FILE_NAME, text).unwrap_err();
            assert_eq!(e.line, Some(line), "{text:?} → {e}");
            assert!(e.reason.contains(reason), "{text:?} → {e}");
            assert!(
                e.to_string().starts_with(&format!("config.toml:{line}: ")),
                "{e}"
            );
        }
        let big = format!("# {}\n", "x".repeat(MAX_BYTES as usize));
        assert!(
            parse(FILE_NAME, &big)
                .unwrap_err()
                .reason
                .contains("256 KiB")
        );
    }

    #[test]
    fn a_valid_file_parses() {
        let text = "[font]\nfamily = \"Iosevka Term, monospace\"\nsize = 16\n[theme]\nname = \"nord\"\n[theme.custom]\nbg = \"#101010\"\ncolor4 = \"#5e81ac\"\n[window]\ndecorations = false\nopacity = 0.9\nstart = \"last\"\n[keys]\nleader = \",\"\n[keys.normal]\n\"<leader> 1\" = \"tab.goto 1\"\n\"g s\" = \"input.style simple\"\n[wsl]\ndistro = \"Ubuntu-24.04\"\n";
        let c = parse(FILE_NAME, text).unwrap();
        assert_eq!(c.font.size, 16);
        assert_eq!(
            c.theme.custom.get("color4").map(String::as_str),
            Some("#5e81ac")
        );
        assert!(!c.window.decorations);
        assert_eq!(c.keys.leader, ",");
        assert_eq!(c.wsl.distro.as_deref(), Some("Ubuntu-24.04"));
    }

    #[test]
    fn first_run_creates_the_file_and_a_save_is_noticed_and_a_refusal_keeps_the_previous() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("rein-app");
        let s = ConfigService::open(Some(dir.clone()));
        let file = dir.join(FILE_NAME);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), DEFAULTS_FILE);
        assert_eq!(s.view().config, Config::default());
        assert!(s.reload_if_changed().is_none(), "unchanged");

        std::fs::write(&file, "[theme]\nname = \"nord\"\n").unwrap();
        let v = s.reload_if_changed().expect("a save is noticed");
        assert_eq!(v.config.theme.name, "nord");
        assert_eq!(v.error, None);

        std::fs::write(&file, "[theme]\nname = \"nord\"\nbogus = 1\n").unwrap();
        let v = s.reload_if_changed().expect("a save is noticed");
        assert_eq!(
            v.config.theme.name, "nord",
            "the previous good config stays"
        );
        assert!(v.error.unwrap().starts_with("config.toml:3:"));

        // Never rewritten: a second open leaves the operator's file alone.
        let _ = ConfigService::open(Some(dir));
        assert!(std::fs::read_to_string(&file).unwrap().contains("bogus"));
    }

    #[test]
    fn the_theme_list_is_the_web_sides() {
        let ts = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/theme.ts"),
        )
        .unwrap();
        let start = ts.find("export const THEME_NAMES").unwrap();
        let list = &ts[start..start + ts[start..].find("];").unwrap()];
        let names: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
        assert_eq!(names, THEMES);
    }

    #[test]
    fn a_config_dir_that_cannot_be_created_is_reported() {
        let t = tempfile::tempdir().unwrap();
        let blocker = t.path().join("file");
        std::fs::write(&blocker, "").unwrap();
        let s = ConfigService::open(Some(blocker.join("rein-app")));
        assert_eq!(s.view().config, Config::default());
        assert!(
            s.view()
                .error
                .unwrap()
                .starts_with("cannot create config.toml")
        );
    }

    #[test]
    fn the_settings_keys_a_file_sets_are_known() {
        assert!(
            settings_keys_set(DEFAULTS_FILE).is_empty(),
            "all commented out"
        );
        assert_eq!(
            settings_keys_set(
                "[theme]\nname = \"nord\"\n[font]\nfamily = \"x\"\n[window]\ndecorations = false\n"
            ),
            ["theme.name", "window.decorations"]
        );
    }

    #[test]
    fn key_names() {
        for ok in [
            "a", ":", "C-\\", "C-n", "C-S-p", "M-1", "Space", "F12", "S-Tab", "<",
        ] {
            assert!(key_ok(ok), "{ok}");
        }
        for bad in ["", "ab", "C-", "F13", "Ctrl+n", "é"] {
            assert!(!key_ok(bad), "{bad}");
        }
    }
}
