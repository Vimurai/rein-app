//! `rein-app profiles list|apply` (blueprint §Components 6, §API; D-002, D-005, D-007, D-008).
//!
//! Per profile, `apply`:
//! 1. registers the marketplace with `claude plugin marketplace add
//!    Vimurai/rein-plugins#stable --scope user` and verifies the entry (D-007);
//! 2. installs/disables plugins with `claude plugin install|disable` (D-005).
//!
//! `hooks` are `rein install`'s alone (rein D-155 r3, D-014): the app never reads a `hooks`
//! value to copy it, never writes or removes one, and writes no file inside a project.
//!
//! The app never hand-writes `enabledPlugins`. `extraKnownMarketplaces` is hand-written
//! only as the D-009 fallback when `claude` is absent (Claude Code registers it at its
//! next start; exit 4). `attribution` is not written: no source value is specified.

use std::path::Path;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use rein_profiles::{
    AllowedKey, ApplyError, ApplyOptions, Patch, PluginAction, PluginError, ProcessShell, Profile,
    Shell, check_plugins, ensure_backup, enumerate, marketplace_add_argv, plugin_argv,
    register_marketplace, rein_plugins, set_plugins, undo,
};
use serde_json::Value;

/// Exit codes (blueprint §API).
const EXIT_ERROR: u8 = 1;
const EXIT_CLAUDE_MISSING: u8 = 4;
const EXIT_PARSE: u8 = 5;
const EXIT_CLAUDE_PLUGIN: u8 = 7;

#[derive(Debug, Subcommand)]
pub enum ProfilesCommand {
    /// Show every profile and whether its settings.json exists.
    List,
    /// Show and write the Rein-owned keys in each profile's settings.json.
    Apply(ApplyArgs),
}

#[derive(Debug, Args)]
pub struct ApplyArgs {
    /// Only these profiles (repeatable; default: all).
    #[arg(long = "profile", value_name = "NAME")]
    profiles: Vec<String>,
    /// Install plugins via `claude plugin install`: comma-separated, or `all`.
    #[arg(long, value_name = "LIST", conflicts_with = "undo")]
    enable: Option<String>,
    /// Disable plugins via `claude plugin disable`: comma-separated, or `all`.
    #[arg(long, value_name = "LIST", conflicts_with = "undo")]
    disable: Option<String>,
    /// Print what would change and which commands would run, without doing it.
    #[arg(long)]
    dry_run: bool,
    /// Restore the Rein-owned keys from settings.json.rein-bak.
    #[arg(long)]
    undo: bool,
}

pub fn run(home: &Path, cmd: ProfilesCommand) -> ExitCode {
    let found = enumerate(home);
    for w in &found.warnings {
        eprintln!("warning: {w}");
    }
    match cmd {
        ProfilesCommand::List => {
            println!("{:<16} {:<8} CONFIG DIR", "PROFILE", "SETTINGS");
            for p in &found.profiles {
                let state = if p.exists { "present" } else { "missing" };
                println!("{:<16} {:<8} {}", p.name, state, p.config_dir.display());
            }
            ExitCode::SUCCESS
        }
        ProfilesCommand::Apply(args) => apply_all(&found.profiles, &args, &ProcessShell),
    }
}

/// POSIX single-quoting, so the printed command line can be pasted as is.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn parse_plugins(list: &Option<String>) -> Result<Vec<String>, PluginError> {
    let Some(list) = list else {
        return Ok(Vec::new());
    };
    let names: Vec<String> = if list == "all" {
        rein_plugins::KNOWN_PLUGINS
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    } else {
        list.split(',').map(|s| s.trim().to_owned()).collect()
    };
    check_plugins(&names)?;
    Ok(names)
}

/// Worst outcome across profiles, as an exit code.
#[derive(Default)]
struct Status(u8);

impl Status {
    fn record(&mut self, code: u8) {
        // A parse/mismatch/generic error outranks "claude missing".
        if self.0 == 0 || (self.0 == EXIT_CLAUDE_MISSING && code != EXIT_CLAUDE_MISSING) {
            self.0 = code;
        }
    }
}

fn apply_all(all: &[Profile], args: &ApplyArgs, shell: &dyn Shell) -> ExitCode {
    if let Some(unknown) = args
        .profiles
        .iter()
        .find(|n| !all.iter().any(|p| &p.name == *n))
    {
        eprintln!("rein-app: no profile named '{unknown}' (see `rein-app profiles list`)");
        return ExitCode::from(EXIT_ERROR);
    }
    let selected: Vec<&Profile> = all
        .iter()
        .filter(|p| args.profiles.is_empty() || args.profiles.contains(&p.name))
        .collect();
    let opts = ApplyOptions {
        dry_run: args.dry_run,
    };
    if args.undo {
        return undo_all(&selected);
    }
    let (enable, disable) = match (parse_plugins(&args.enable), parse_plugins(&args.disable)) {
        (Ok(e), Ok(d)) => (e, d),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("rein-app: {e}");
            return ExitCode::from(EXIT_ERROR);
        }
    };

    let mut status = Status::default();
    let mut claude_missing = false;
    for profile in selected {
        // `hooks` belong to `rein install` alone (rein D-155 r3, D-014): never read or copied.
        let patch = Patch::new();
        if let Err(code) = apply_one(
            profile,
            &patch,
            &enable,
            &disable,
            opts,
            shell,
            &mut claude_missing,
        ) {
            status.record(code);
        }
    }
    if claude_missing {
        eprintln!(
            "rein-app: `claude` not found — marketplace entry written for registration at the next Claude Code start; plugins skipped (install Claude Code and re-run to install them)"
        );
        status.record(EXIT_CLAUDE_MISSING);
    }
    ExitCode::from(status.0)
}

fn apply_one(
    profile: &Profile,
    patch: &Patch,
    enable: &[String],
    disable: &[String],
    opts: ApplyOptions,
    shell: &dyn Shell,
    claude_missing: &mut bool,
) -> Result<(), u8> {
    let fail = |e: &dyn std::fmt::Display, code: u8| {
        eprintln!("{}: {e}", profile.name);
        code
    };
    // D-024: rein-app writes wherever accounts.json points; say so when it is not a Claude dir.
    if !profile
        .config_dir
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with(".claude"))
    {
        println!(
            "{}: note: config dir {} is not a .claude* directory",
            profile.name,
            profile.config_dir.display()
        );
    }
    let verb = if opts.dry_run {
        "would change"
    } else {
        "changed"
    };
    match rein_profiles::apply(profile, patch, opts) {
        Ok(out) if out.changes.is_empty() => {}
        Ok(out) => {
            println!(
                "{} ({}): {verb}",
                profile.name,
                profile.settings_path.display()
            );
            for c in &out.changes {
                let before = c
                    .before
                    .as_ref()
                    .map_or("(absent)".to_owned(), Value::to_string);
                println!("  {}: {before} -> {}", c.key.json_key(), c.after);
            }
        }
        Err(e @ (ApplyError::Parse(_) | ApplyError::Plan(_))) => return Err(fail(&e, EXIT_PARSE)),
        Err(e) => return Err(fail(&e, EXIT_ERROR)),
    }

    if opts.dry_run {
        let mut cmds = vec![marketplace_add_argv()];
        cmds.extend(enable.iter().map(|p| plugin_argv(PluginAction::Install, p)));
        cmds.extend(
            disable
                .iter()
                .map(|p| plugin_argv(PluginAction::Disable, p)),
        );
        for argv in cmds {
            println!(
                "{}: would run CLAUDE_CONFIG_DIR={} claude {}",
                profile.name,
                shell_quote(&profile.config_dir.to_string_lossy()),
                argv.join(" ")
            );
        }
        return Ok(());
    }
    if *claude_missing {
        return hand_write_marketplace(profile, opts);
    }
    if !profile.config_dir.is_dir() {
        println!(
            "{}: config dir missing; marketplace and plugins skipped",
            profile.name
        );
        return Ok(());
    }
    ensure_backup(profile).map_err(|e| fail(&e, EXIT_ERROR))?;
    let steps = register_marketplace(shell, profile)
        .and_then(|()| set_plugins(shell, profile, PluginAction::Install, &as_strs(enable)))
        .and_then(|()| set_plugins(shell, profile, PluginAction::Disable, &as_strs(disable)));
    match steps {
        Ok(()) => {
            println!(
                "{}: marketplace rein-plugins registered{}",
                profile.name,
                plugin_note(enable, disable)
            );
            Ok(())
        }
        Err(PluginError::ClaudeMissing) => {
            *claude_missing = true;
            hand_write_marketplace(profile, opts)
        }
        // Any `claude plugin` failure, a wrong marketplace entry, or an unexpected change
        // by `claude` (blueprint §API exit 7).
        Err(
            e @ (PluginError::MarketplaceMismatch(_)
            | PluginError::UnexpectedChange(_)
            | PluginError::Failed { .. }
            | PluginError::NotVerified(_)),
        ) => Err(fail(&e, EXIT_CLAUDE_PLUGIN)),
        Err(e) => Err(fail(&e, EXIT_ERROR)),
    }
}

/// D-009 fallback when `claude` is absent: hand-write the D-002 marketplace entry (with
/// ref "stable"); Claude Code registers it at its next session start. `enabledPlugins` is
/// never hand-written.
fn hand_write_marketplace(profile: &Profile, opts: ApplyOptions) -> Result<(), u8> {
    if !profile.config_dir.is_dir() {
        return Ok(());
    }
    let patch = Patch::new().set(
        AllowedKey::ExtraKnownMarketplaces,
        rein_plugins::marketplace_entry(),
    );
    match rein_profiles::apply(profile, &patch, opts) {
        Ok(_) => {
            println!(
                "{}: marketplace entry written; registration completes at the next Claude Code start",
                profile.name
            );
            Ok(())
        }
        Err(e @ (ApplyError::Parse(_) | ApplyError::Plan(_))) => {
            eprintln!("{}: {e}", profile.name);
            Err(EXIT_PARSE)
        }
        Err(e) => {
            eprintln!("{}: {e}", profile.name);
            Err(EXIT_ERROR)
        }
    }
}

fn as_strs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

fn plugin_note(enable: &[String], disable: &[String]) -> String {
    let mut s = String::new();
    if !enable.is_empty() {
        s.push_str(&format!("; installed {}", enable.join(", ")));
    }
    if !disable.is_empty() {
        s.push_str(&format!("; disabled {}", disable.join(", ")));
    }
    s
}

fn undo_all(selected: &[&Profile]) -> ExitCode {
    let mut status = Status::default();
    for p in selected {
        match undo(p) {
            Ok(()) => println!(
                "{}: Rein-owned keys restored from settings.json.rein-bak",
                p.name
            ),
            Err(ApplyError::NoBackup) => println!("{}: no backup, unchanged", p.name),
            Err(e @ (ApplyError::Parse(_) | ApplyError::Plan(_))) => {
                eprintln!("{}: {e}", p.name);
                status.record(EXIT_PARSE);
            }
            Err(e) => {
                eprintln!("{}: {e}", p.name);
                status.record(EXIT_ERROR);
            }
        }
    }
    ExitCode::from(status.0)
}
