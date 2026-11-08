//! Plugin enable/disable through Claude Code's own CLI (D-005): per profile,
//! `CLAUDE_CONFIG_DIR=<config_dir> claude plugin install|disable <plugin>@rein-plugins --scope user`,
//! then re-read settings.json and verify `enabledPlugins` shows the result. The app never
//! writes an `enabledPlugins` entry itself. Calls go through [`Shell`] with a fixed argv;
//! plugin names come only from the closed [`rein_plugins::KNOWN_PLUGINS`] list.

use std::ffi::OsString;
use std::io;
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

use crate::profile::Profile;
use crate::rein_plugins::{KNOWN_PLUGINS, MARKETPLACE};
use crate::store::{ApplyError, read_settings};

/// Result of one process run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutput {
    pub code: Option<i32>,
    pub stderr: String,
}

/// Runs a program with an exact argv and environment changes; never through a shell.
pub trait Shell {
    fn run(
        &self,
        program: &str,
        args: &[String],
        set_env: &[(String, OsString)],
        remove_env: &[&str],
    ) -> io::Result<ShellOutput>;
}

/// Production [`Shell`]: `std::process::Command`, stdin closed, output captured.
pub struct ProcessShell;

impl Shell for ProcessShell {
    fn run(
        &self,
        program: &str,
        args: &[String],
        set_env: &[(String, OsString)],
        remove_env: &[&str],
    ) -> io::Result<ShellOutput> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for k in remove_env {
            cmd.env_remove(k);
        }
        for (k, v) in set_env {
            cmd.env(k, v);
        }
        let out = cmd.output()?;
        Ok(ShellOutput {
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// Enable (install) or disable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginAction {
    Install,
    Disable,
}

impl PluginAction {
    fn verb(self) -> &'static str {
        match self {
            PluginAction::Install => "install",
            PluginAction::Disable => "disable",
        }
    }
    fn expected(self) -> bool {
        self == PluginAction::Install
    }
}

/// Why a plugin step failed.
#[derive(Debug)]
pub enum PluginError {
    /// Not in the closed plugin list.
    UnknownPlugin(String),
    /// `claude` is not on PATH (CLI exit 4).
    ClaudeMissing,
    /// `claude` exited non-zero.
    Failed {
        plugin: String,
        code: Option<i32>,
        stderr: String,
    },
    /// `claude` exited 0 but settings.json does not show the result.
    NotVerified(String),
    /// The marketplace entry Claude Code wrote is not the D-002 shape (CLI exit 7).
    MarketplaceMismatch(Value),
    /// `claude` changed settings beyond the rein-plugins entries (CLI exit 7).
    UnexpectedChange(Vec<String>),
    Settings(ApplyError),
    Io(io::Error),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::UnknownPlugin(p) => {
                write!(
                    f,
                    "unknown plugin '{p}' (known: {})",
                    KNOWN_PLUGINS.join(", ")
                )
            }
            PluginError::ClaudeMissing => write!(f, "`claude` (Claude Code) is not on PATH"),
            PluginError::Failed {
                plugin,
                code,
                stderr,
            } => {
                let last = stderr
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("");
                write!(
                    f,
                    "claude plugin step for {plugin} failed (exit {code:?}): {last}"
                )
            }
            PluginError::NotVerified(p) => {
                write!(
                    f,
                    "claude reported success for {p} but settings.json does not show it"
                )
            }
            PluginError::MarketplaceMismatch(v) => write!(
                f,
                "claude registered rein-plugins as {v}, expected github {}#{}; not patched",
                crate::rein_plugins::REPO,
                crate::rein_plugins::REF
            ),
            PluginError::UnexpectedChange(keys) => write!(
                f,
                "`claude` changed settings.json beyond the rein-plugins entries ({}); stopped — review settings.json",
                keys.join(", ")
            ),
            PluginError::Settings(e) => write!(f, "{e}"),
            PluginError::Io(e) => write!(f, "i/o error ({e})"),
        }
    }
}

impl std::error::Error for PluginError {}

/// Validate names against the closed list.
pub fn check_plugins<S: AsRef<str>>(names: &[S]) -> Result<(), PluginError> {
    match names.iter().find(|n| !KNOWN_PLUGINS.contains(&n.as_ref())) {
        Some(bad) => Err(PluginError::UnknownPlugin(bad.as_ref().to_owned())),
        None => Ok(()),
    }
}

/// The exact argv after `claude`.
pub fn plugin_argv(action: PluginAction, plugin: &str) -> Vec<String> {
    [
        "plugin",
        action.verb(),
        &format!("{plugin}@{MARKETPLACE}"),
        "--scope",
        "user",
    ]
    .map(String::from)
    .to_vec()
}

/// The exact argv after `claude` that registers the marketplace (D-007).
pub fn marketplace_add_argv() -> Vec<String> {
    let source = format!("{}#{}", crate::rein_plugins::REPO, crate::rein_plugins::REF);
    ["plugin", "marketplace", "add", &source, "--scope", "user"]
        .map(String::from)
        .to_vec()
}

/// Everything in settings.json except the entries `claude plugin` is allowed to write for
/// rein-plugins (`extraKnownMarketplaces.rein-plugins`, `enabledPlugins.*@rein-plugins`).
/// Compared before/after every `claude` call: `claude` is found on PATH and is not
/// trusted to leave other keys alone (E-4 security re-review P1).
fn foreign_view(profile: &Profile) -> Result<Map<String, Value>, PluginError> {
    let doc = read_settings(&profile.settings_path)
        .map_err(PluginError::Settings)?
        .map(|(_, v)| v);
    let mut map = match doc {
        None => Map::new(),
        Some(Value::Object(m)) => m,
        Some(_) => {
            return Err(PluginError::Settings(crate::store::ApplyError::Plan(
                crate::patch::PlanError::NotAnObject,
            )));
        }
    };
    let suffix = format!("@{MARKETPLACE}");
    for (key, keep) in [
        (
            "extraKnownMarketplaces",
            &(|k: &str| k != MARKETPLACE) as &dyn Fn(&str) -> bool,
        ),
        ("enabledPlugins", &|k: &str| !k.ends_with(&suffix)),
    ] {
        if let Some(Value::Object(inner)) = map.get_mut(key) {
            inner.retain(|k, _| keep(k));
            if inner.is_empty() {
                map.remove(key);
            }
        }
    }
    Ok(map)
}

/// Names of top-level keys whose foreign view differs.
fn changed_keys(before: &Map<String, Value>, after: &Map<String, Value>) -> Vec<String> {
    let mut keys: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|k| before.get(*k) != after.get(*k))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

fn run_claude(
    shell: &dyn Shell,
    profile: &Profile,
    argv: &[String],
    what: &str,
) -> Result<(), PluginError> {
    let before = foreign_view(profile)?;
    run_claude_unguarded(shell, profile, argv, what)?;
    let changed = changed_keys(&before, &foreign_view(profile)?);
    if changed.is_empty() {
        Ok(())
    } else {
        Err(PluginError::UnexpectedChange(changed))
    }
}

fn run_claude_unguarded(
    shell: &dyn Shell,
    profile: &Profile,
    argv: &[String],
    what: &str,
) -> Result<(), PluginError> {
    let env = [(
        "CLAUDE_CONFIG_DIR".to_owned(),
        profile.config_dir.clone().into_os_string(),
    )];
    let out = shell
        .run(
            "claude",
            argv,
            &env,
            &["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"],
        )
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => PluginError::ClaudeMissing,
            _ => PluginError::Io(e),
        })?;
    if out.code == Some(0) {
        Ok(())
    } else {
        Err(PluginError::Failed {
            plugin: what.to_owned(),
            code: out.code,
            stderr: out.stderr,
        })
    }
}

/// Register the rein-plugins marketplace with `claude plugin marketplace add
/// Vimurai/rein-plugins#stable --scope user`, then verify settings.json holds exactly the
/// D-002 entry (github, repo, ref "stable"). A different entry is reported, never patched.
pub fn register_marketplace(shell: &dyn Shell, profile: &Profile) -> Result<(), PluginError> {
    run_claude(shell, profile, &marketplace_add_argv(), MARKETPLACE)?;
    let settings = read_settings(&profile.settings_path).map_err(PluginError::Settings)?;
    let written = settings
        .as_ref()
        .and_then(|(_, v)| v.get("extraKnownMarketplaces"))
        .and_then(|m| m.get(MARKETPLACE))
        .cloned();
    let expected = crate::rein_plugins::marketplace_entry()[MARKETPLACE].clone();
    if written.as_ref() == Some(&expected) {
        Ok(())
    } else {
        Err(PluginError::MarketplaceMismatch(
            written.unwrap_or(Value::Null),
        ))
    }
}

/// Run `action` for each plugin in `profile`, verifying each result in settings.json.
pub fn set_plugins(
    shell: &dyn Shell,
    profile: &Profile,
    action: PluginAction,
    plugins: &[&str],
) -> Result<(), PluginError> {
    check_plugins(plugins)?;
    for plugin in plugins {
        run_claude(shell, profile, &plugin_argv(action, plugin), plugin)?;
        let settings = read_settings(&profile.settings_path).map_err(PluginError::Settings)?;
        let key = format!("{plugin}@{MARKETPLACE}");
        let shown = settings
            .as_ref()
            .and_then(|(_, v)| v.get("enabledPlugins"))
            .and_then(|p| p.get(&key))
            .and_then(Value::as_bool);
        if shown != Some(action.expected()) {
            return Err(PluginError::NotVerified(key));
        }
    }
    Ok(())
}
