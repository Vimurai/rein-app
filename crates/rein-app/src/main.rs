//! `rein-app` CLI entry point.
//!
//! Subcommands (`install`, `update`, `rollback`, `doctor`, `profiles`, `launch`) are wired
//! by E-3..E-6 per `.ai/blueprints/distribution.md` §API. Until then only `--version`
//! and `--help` exist.

use std::process::ExitCode;

const USAGE: &str = "usage: rein-app [--version | --help]";

/// What the CLI does for a given argument list (excluding argv[0]).
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Version,
    Help,
    Unknown(String),
}

fn parse(args: &[String]) -> Action {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => Action::Help,
        Some("-V" | "--version") => Action::Version,
        Some(other) => Action::Unknown(other.to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Action::Version => {
            println!("rein-app {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Action::Help => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Action::Unknown(arg) => {
            eprintln!("rein-app: unknown argument '{arg}'\n{USAGE}");
            ExitCode::from(64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parse_maps_flags_and_rejects_unknown_arguments() {
        assert_eq!(parse(&args(&[])), Action::Help);
        assert_eq!(parse(&args(&["--help"])), Action::Help);
        assert_eq!(parse(&args(&["-V"])), Action::Version);
        assert_eq!(parse(&args(&["--version"])), Action::Version);
        assert_eq!(
            parse(&args(&["install"])),
            Action::Unknown("install".into())
        );
    }
}
