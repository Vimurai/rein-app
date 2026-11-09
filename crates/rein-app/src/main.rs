//! `rein-app` CLI entry point (blueprint §API / Interface Contracts).
//!
//! Wired so far: `install` (E-5), `profiles list|apply` (E-4). `update`, `rollback`,
//! `doctor` and `launch` arrive with E-6.

mod install_cmd;
mod profiles_cmd;
mod runner;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
        /// rein release to install (X.Y.Z). Resolving the newest tag arrives with `update`.
        #[arg(long, value_name = "X.Y.Z")]
        version: semver::Version,
    },
    /// List Claude profiles and edit the Rein-owned settings keys.
    #[command(subcommand)]
    Profiles(profiles_cmd::ProfilesCommand),
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(home) = resolve_home(cli.home) else {
        eprintln!("rein-app: cannot determine an absolute home directory; pass --home /ABS/DIR");
        return ExitCode::from(1);
    };
    match cli.command {
        Command::Install { version } => install(&home, &version),
        Command::Profiles(cmd) => profiles_cmd::run(&home, cmd),
    }
}

fn install(home: &std::path::Path, version: &semver::Version) -> ExitCode {
    use rein_release::{HostPolicy, ReleaseSource, ReqwestHttp, production_verifier};
    let http = match ReqwestHttp::new(HostPolicy::github()) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("rein-app: {e}");
            return ExitCode::from(install_cmd::EXIT_ERROR);
        }
    };
    let verifier = production_verifier();
    let deps = install_cmd::Deps {
        http: &http,
        source: &ReleaseSource::rein_github(),
        verifier: verifier.as_ref(),
        process: &runner::SystemProcess,
    };
    install_cmd::run(home, version, &deps)
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
        assert!(matches!(cli.command, Command::Install { .. }));
        assert!(Cli::try_parse_from(["rein-app", "install", "--version", "latest"]).is_err());
        assert!(
            Cli::try_parse_from(["rein-app", "profiles", "apply", "--undo", "--enable", "all"])
                .is_err(),
            "--undo conflicts with --enable"
        );
    }
}
