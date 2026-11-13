//! The window's own tmux server (desktop.md §Look and feel v2 V8, D-055, E-57). Sessions opened
//! in the window run on the tmux socket `rein-app`, started with the app's own config, so the
//! user's `~/.tmux.conf` and plugins never apply. rein calls plain `tmux`, so a shim named
//! `tmux` is put first on the tab's `PATH`.
//!
//! Two constant scripts, split by transport (V8 component 3, amended 2026-11-13): the setup
//! script goes on **stdin** (it carries the shim's and the config's text), the start script
//! is an **argv** word with no quote character and no file text — Windows command-line
//! quoting and `wsl.exe` re-parse argv. Nothing but these compiled constants is put in.

use std::time::Duration;

/// The shim, written to `~/.local/state/rein-app/tmux/bin/tmux` (0700).
pub const SHIM: &str = include_str!("tmux_shim.sh");
/// The app's tmux config, written to `~/.local/state/rein-app/tmux/tmux.conf` (0600).
pub const CONF: &str = include_str!("tmux_app.conf");
/// The setup script with the two markers the shim and the config replace.
const SETUP_TEMPLATE: &str = include_str!("tmux_setup.sh");
/// The start script (Linux / macOS): the tmux home checked, the shim first on `PATH`, then
/// `rein start`. One line, no quote character.
const START: &str = include_str!("tmux_start.sh");
/// How long the setup may run (V8 "Execution constraints").
pub const SETUP_LIMIT: Duration = Duration::from_secs(15);

/// Which tmux server a tab's `rein start` reaches (`tmux.server`, V8 component 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
    /// rein-app's own server: the shim, the socket `rein-app`, the app's config.
    App,
    /// The user's default server — the behaviour before E-57, and the rollback.
    User,
}

impl Server {
    /// The setting's words: `app` | `user`; anything else is the default, `app`.
    pub fn from_setting(word: Option<&str>) -> Self {
        match word {
            Some("user") => Self::User,
            _ => Self::App,
        }
    }

    /// The word the V6 overlay script takes for it.
    pub fn word(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::User => "user",
        }
    }
}

/// The setting's allowed words.
pub const SERVER_WORDS: &[&str] = &["app", "user"];

/// The setup script as it is fed on stdin: the template with each marker line replaced by
/// the file's text.
pub fn setup_script() -> String {
    SETUP_TEMPLATE
        .replace("@@SHIM@@\n", SHIM)
        .replace("@@CONF@@\n", CONF)
}

/// The start script for Linux and macOS (the pty layer names the tty).
pub fn start_script() -> &'static str {
    START.trim_end()
}

/// The start script made into `rein start --kill --yes` (V9 component 4, E-62): the same
/// checks and `PATH`, so rein's tmux calls reach the app's server. No tty line is needed.
pub fn kill_script() -> String {
    format!("{} --kill --yes", start_script())
}

/// The start script inside WSL: the same, printing the tty first (V6 "Client tty").
pub fn start_script_wsl() -> String {
    start_script().replacen(
        "; exec ~/.rein/bin/rein start",
        "; tty; exec ~/.rein/bin/rein start",
        1,
    )
}

/// The app's fixed sentence for a setup that did not finish with 0 (V8 3a: own exit code
/// each, mapped as V6's are). `None`: it did not finish within [`SETUP_LIMIT`].
pub fn setup_reason(code: Option<i32>) -> &'static str {
    match code {
        Some(20) => {
            "your home directory's path holds a space, quote, glob or other character the app refuses"
        }
        Some(21) => "a directory under ~/.local/state could not be made",
        Some(22) => {
            "a directory or file under ~/.local/state/rein-app is a symlink or not what it should be"
        }
        Some(23) => {
            "a directory or file on the way to ~/.local/state/rein-app/tmux belongs to another user"
        }
        Some(24) => {
            "~/.local, ~/.local/state or ~/.local/state/rein-app can be written by other users"
        }
        Some(25) => "the app's tmux files could not be written",
        None => "the tmux setup did not finish within 15 seconds",
        Some(_) => "the tmux setup could not run",
    }
}

/// What the window adds to a setup failure: the way out. (A tab whose start script exits 126
/// says the same; the web side, which sees the exit, holds that sentence — `term.ts`.)
pub const USE_YOUR_OWN: &str = "Settings → tmux server → my own uses your own tmux instead";

/// The whole message a tab shows when the setup failed.
pub fn setup_message(why: &str) -> String {
    format!("tmux setup failed: {why}. {USE_YOUR_OWN}.")
}

/// A finished setup run as the host reports it.
pub fn setup_outcome(run: std::io::Result<Option<i32>>) -> Result<(), &'static str> {
    match run {
        Ok(Some(0)) => Ok(()),
        Ok(code) => Err(setup_reason(code)),
        Err(_) => Err(setup_reason(Some(-1))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setup_carries_the_two_files_exactly() {
        let s = setup_script();
        assert!(
            !s.contains("@@SHIM@@") && !s.contains("@@CONF@@"),
            "both markers replaced"
        );
        assert!(s.contains(&format!(
            "<<'REIN_APP_SHIM_END' || exit 25\n{SHIM}REIN_APP_SHIM_END\n"
        )));
        assert!(s.contains(&format!(
            "<<'REIN_APP_CONF_END' || exit 25\n{CONF}REIN_APP_CONF_END\n"
        )));
        // A here-document ends at the first line equal to its delimiter: neither text has one,
        // and each ends with a newline so the delimiter starts its own line.
        for text in [SHIM, CONF] {
            assert!(text.ends_with('\n'));
            assert!(
                !text
                    .lines()
                    .any(|l| l == "REIN_APP_SHIM_END" || l == "REIN_APP_CONF_END")
            );
            assert!(!text.contains("@@SHIM@@") && !text.contains("@@CONF@@"));
        }
        assert!(
            !s.contains("XDG_STATE_HOME\n") && !s.contains("$XDG_STATE_HOME"),
            "never read"
        );
    }

    /// V8 component 4: a closed list with no command-running line — read from the file itself.
    #[test]
    fn the_app_config_runs_no_command() {
        let lines: Vec<&str> = CONF
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        for l in &lines {
            let lower = l.to_ascii_lowercase();
            for bad in [
                "run-shell",
                "run ",
                "source",
                "if-shell",
                "if ",
                "hook",
                "@plugin",
                "#(",
                "`",
                "$(",
            ] {
                assert!(!lower.contains(bad), "{l:?} contains {bad:?}");
            }
            assert!(l.starts_with("set "), "{l:?}: only `set` lines");
        }
        assert_eq!(
            lines,
            [
                "set -g default-terminal \"tmux-256color\"",
                "set -as terminal-features \",*:RGB\"",
                "set -g mouse on",
                "set -sg escape-time 10",
                "set -g focus-events on",
                "set -g history-limit 50000",
                "set -g set-clipboard external",
                "set -g status off",
                "set -g pane-border-status top",
                "set -g pane-border-format \" #{?#{@rein_role},#{@rein_role} · #{@rein_account},#{pane_index}} \"",
            ],
            "exactly V8's list (E-58: the pane role labels)"
        );
        // E-58: no shell substitution anywhere in the file, comments included.
        assert!(!CONF.contains("#("), "the config holds `#(`");
    }

    /// V8 3b and its rule for every argv script: one line, no quote character, no file text.
    #[test]
    fn the_start_scripts_are_argv_safe() {
        for s in [start_script().to_string(), start_script_wsl()] {
            assert!(!s.contains(['\'', '"', '\n', '`', '\\']), "{s}");
            // set -o noglob, not set -f: in zsh -f is NO_RCS (security_engineer, 2026-11-13).
            assert!(s.starts_with("set -o noglob; H=$HOME/.local/state/rein-app/tmux; "));
            assert!(s.ends_with("exec ~/.rein/bin/rein start"));
            assert!(
                s.contains("|| exit 126;")
                    && s.contains("unset TMUX TMUX_PANE; PATH=$H/bin:$PATH;")
            );
            assert!(!s.contains("XDG_STATE_HOME"));
        }
        assert!(!start_script().contains("tty;"));
        assert!(start_script_wsl().contains("; tty; exec ~/.rein/bin/rein start"));
    }

    #[test]
    fn every_setup_exit_has_a_fixed_sentence() {
        for code in [
            Some(20),
            Some(21),
            Some(22),
            Some(23),
            Some(24),
            Some(25),
            Some(9),
            None,
        ] {
            assert!(!setup_reason(code).is_empty());
        }
        assert_eq!(setup_outcome(Ok(Some(0))), Ok(()));
        assert_eq!(
            setup_outcome(Err(std::io::Error::other("os words"))),
            Err("the tmux setup could not run")
        );
        assert_eq!(Server::from_setting(Some("user")), Server::User);
        assert_eq!(Server::from_setting(Some("nonsense")), Server::App);
        assert_eq!(Server::from_setting(None), Server::App);
    }
}
