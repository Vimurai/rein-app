//! `rein-app` CLI entry point (blueprint §API / Interface Contracts).
//!
//! `install`, `update`, `rollback`, `doctor` (E-5/E-6), `profiles list|apply` (E-4) and,
//! on Windows, `launch` (E-6, WSL2).

mod install_cmd;
#[cfg(test)]
mod layout_equality;
mod profiles_cmd;
mod runner;
mod update_cmd;
mod verdict;
mod window_cmd;
mod wsl;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use rein_release::{
    HostPolicy, REIN_LATEST_API, ReleaseSource, ReqwestHttp, SignatureVerifier, production_verifier,
};

#[derive(Debug, Parser)]
#[command(
    name = "rein-app",
    version,
    about = "Install, verify and update Rein; manage Claude profile settings."
)]
struct Cli {
    /// Home directory to operate on (default: $HOME).
    #[arg(long, global = true, value_name = "DIR")]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Fetch, verify and install a signed rein release into $HOME/.rein, then run
    /// `rein install` and `rein doctor` from it.
    Install {
        /// rein release to install (X.Y.Z); default: the newest release.
        #[arg(long, value_name = "X.Y.Z")]
        version: Option<semver::Version>,
    },
    /// Install the newest rein release if it is newer than the installed one.
    Update {
        /// Install without asking.
        #[arg(long, conflicts_with = "check")]
        yes: bool,
        /// Report the installed and newest version and the notes; install nothing
        /// (exit 0 newer, 10 current, 11 offline).
        #[arg(long)]
        check: bool,
    },
    /// Restore the previous rein version's code from its snapshot (your data is kept),
    /// then run `rein install` and `rein doctor`.
    Rollback,
    /// Run `rein doctor` from the installed tree.
    Doctor {
        /// Pass `--json` to `rein doctor`; its output is printed untouched.
        #[arg(long)]
        json: bool,
    },
    /// Windows: open Windows Terminal into rein inside WSL2 (installing WSL/Ubuntu if absent).
    Launch {
        /// Project directory to start rein in.
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// List Claude profiles and edit the Rein-owned settings keys.
    #[command(subcommand)]
    Profiles(profiles_cmd::ProfilesCommand),
    /// Open the rein-app window (`rein-desktop`), with DIR as the first tab.
    Window {
        /// Project directory to open.
        dir: Option<PathBuf>,
    },
}

/// `$HOME` (or `%USERPROFILE%` on Windows) unless `--home` was given. An empty or relative
/// value is refused: it would place the Rein home under the current directory.
fn resolve_home(flag: Option<PathBuf>) -> Option<PathBuf> {
    let env = |k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    flag.or_else(|| env("HOME"))
        .or_else(|| env("USERPROFILE"))
        .filter(|p| p.is_absolute())
}

/// A usage error: unknown command or flag, bad value (D-016). Never clap's default 2,
/// which on `install` means a release failed verification.
const EXIT_USAGE: u8 = 64;

/// clap's own exit for `--help`/`--version` (0) stays; every usage error becomes 64.
fn usage_exit(err: &clap::Error) -> u8 {
    if err.exit_code() == 0 { 0 } else { EXIT_USAGE }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return ExitCode::from(usage_exit(&err));
        }
    };
    let Some(home) = resolve_home(cli.home) else {
        eprintln!("rein-app: cannot determine an absolute home directory; pass --home /ABS/DIR");
        return ExitCode::from(1);
    };
    let code = match cli.command {
        Command::Install { version } => {
            with_network(|deps| update_cmd::install_cli(&home, version, REIN_LATEST_API, deps))
        }
        Command::Update { check: true, .. } => with_network(|deps| {
            update_cmd::update_check(&home, REIN_LATEST_API, deps, &mut std::io::stdout())
        }),
        Command::Update { yes, .. } => {
            with_network(|deps| update_cmd::update_cli(&home, yes, REIN_LATEST_API, deps))
        }
        Command::Rollback => update_cmd::rollback(
            &home,
            &runner::SystemProcess,
            rein_release::InstallLock::run_uid,
        ),
        Command::Doctor { json } => update_cmd::doctor(&home, &runner::SystemProcess, json),
        Command::Launch { dir } => launch(&dir),
        Command::Profiles(cmd) => return profiles_cmd::run(&home, cmd),
        Command::Window { dir } => window_cmd::run(dir.as_deref()),
    };
    ExitCode::from(code)
}

/// Build the production network dependencies and run `f` with them.
fn with_network(f: impl FnOnce(&install_cmd::Deps<'_>) -> u8) -> u8 {
    let http = match ReqwestHttp::new(HostPolicy::github()) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("rein-app: {e}");
            return install_cmd::EXIT_ERROR;
        }
    };
    let verifier: Box<dyn SignatureVerifier> = production_verifier();
    f(&install_cmd::Deps {
        http: &http,
        source: &ReleaseSource::rein_github(),
        verifier: verifier.as_ref(),
        process: &runner::SystemProcess,
        run_uid: rein_release::InstallLock::run_uid,
    })
}

#[cfg(windows)]
fn launch(dir: &Path) -> u8 {
    wsl::launch(&wsl::SystemShell, &mut std::io::stdin().lock(), dir)
}

#[cfg(not(windows))]
fn launch(_dir: &Path) -> u8 {
    eprintln!("rein-app: `launch` is for Windows; on macOS/Linux run `rein start` directly");
    install_cmd::EXIT_ERROR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profiles_subcommands_and_global_home() {
        let cli = Cli::try_parse_from(["rein-app", "--home", "/h", "profiles", "list"]).unwrap();
        assert_eq!(cli.home, Some(PathBuf::from("/h")));
        let cli = Cli::try_parse_from([
            "rein-app",
            "profiles",
            "apply",
            "--profile",
            "work",
            "--enable",
            "all",
            "--dry-run",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Profiles(_)));
        assert!(Cli::try_parse_from(["rein-app", "bogus"]).is_err());
        assert_eq!(
            resolve_home(Some(PathBuf::from("rel"))),
            None,
            "relative home refused"
        );
        let cli = Cli::try_parse_from(["rein-app", "install", "--version", "0.5.0"]).unwrap();
        assert!(matches!(cli.command, Command::Install { version: Some(_) }));
        let cli = Cli::try_parse_from(["rein-app", "install"]).unwrap();
        assert!(matches!(cli.command, Command::Install { version: None }));
        let cli = Cli::try_parse_from(["rein-app", "update", "--yes"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Update {
                yes: true,
                check: false
            }
        ));
        let cli = Cli::try_parse_from(["rein-app", "update", "--check"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Update {
                yes: false,
                check: true
            }
        ));
        assert!(Cli::try_parse_from(["rein-app", "update", "--check", "--yes"]).is_err());
        let cli = Cli::try_parse_from(["rein-app", "doctor", "--json"]).unwrap();
        assert!(matches!(cli.command, Command::Doctor { json: true }));
        let cli = Cli::try_parse_from(["rein-app", "window", "/p"]).unwrap();
        assert!(matches!(cli.command, Command::Window { dir: Some(_) }));
        for sub in ["rollback", "doctor", "launch", "window"] {
            assert!(Cli::try_parse_from(["rein-app", sub]).is_ok(), "{sub}");
        }
        assert!(Cli::try_parse_from(["rein-app", "install", "--version", "latest"]).is_err());
        assert!(
            Cli::try_parse_from(["rein-app", "profiles", "apply", "--undo", "--enable", "all"])
                .is_err(),
            "--undo conflicts with --enable"
        );
    }
}
