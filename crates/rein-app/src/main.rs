//! `rein-app` CLI entry point (blueprint §API / Interface Contracts).
//!
//! Wired so far: `profiles list|apply` (E-4). `install`, `update`, `rollback`, `doctor`
//! and `launch` arrive with E-5/E-6.

mod profiles_cmd;

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
    /// List Claude profiles and edit the Rein-owned settings keys.
    #[command(subcommand)]
    Profiles(profiles_cmd::ProfilesCommand),
}

/// `$HOME` (or `%USERPROFILE%` on Windows) unless `--home` was given.
fn resolve_home(flag: Option<PathBuf>) -> Option<PathBuf> {
    flag.or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(home) = resolve_home(cli.home) else {
        eprintln!("rein-app: cannot determine the home directory; pass --home DIR");
        return ExitCode::from(1);
    };
    match cli.command {
        Command::Profiles(cmd) => profiles_cmd::run(&home, cmd),
    }
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
        assert!(
            Cli::try_parse_from(["rein-app", "profiles", "apply", "--undo", "--enable", "all"])
                .is_err(),
            "--undo conflicts with --enable"
        );
    }
}
