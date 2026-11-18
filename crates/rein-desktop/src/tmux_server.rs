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
/// The attach path (V12 component 3, E-69): after the start script's checks and `PATH`, the
/// tag (`$1`) is re-checked as 64 hex, the session carrying it is looked up again on the app's
/// server, its id checked as `$` plus digits, and `tmux attach` is exec'd on that id. A session
/// gone since the lookup exits [`ATTACH_GONE`]; it never falls back to `rein start`. With a
/// second word (an adoption, V12 9) that session id is used — checked the same way — instead
/// of a lookup by the tag. One line,
/// no quote character: `#` only inside a word (an interactive shell's comment starts a word).
fn attach_tail() -> String {
    format!(
        "echo $1 | grep -qxE [0-9a-f]{{64}} || exit {ATTACH_REFUSED}; s=; if [ $# -ge 2 ]; then s=x$2; tmux has-session -t $2 2>/dev/null || exit {ATTACH_GONE}; else for l in $(tmux list-sessions -F x#{{session_id}}:#{{@rein_app_project}} 2>/dev/null); do case $l in *:$1) s=${{l%%:*}};; esac; done; fi; n=${{s#x?}}; [ ${{#n}} -gt 0 ] || exit {ATTACH_GONE}; echo $n | grep -qxE [0-9]+ || exit {ATTACH_GONE}; [ ${{s%$n}} = x$ ] || exit {ATTACH_GONE}; exec tmux attach -t ${{s#x}}"
    )
}
/// The attach script's word was not a tag (the app never sends one that is not).
pub const ATTACH_REFUSED: i32 = 75;
/// The attach script found no session with the tag: it ended between the lookup and the attach.
pub const ATTACH_GONE: i32 = 76;
/// The most output the setup's lookup may print (a row per session of the app's server).
pub const SETUP_CAP: u64 = 256 * 1024;

/// How long the setup may run (V8 "Execution constraints").
pub const SETUP_LIMIT: Duration = Duration::from_secs(15);

/// Which tmux server a tab's `rein start` reaches (`tmux.server`, V8 component 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
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

/// The attach script for Linux and macOS (V12 component 3): the start script's checks and
/// `PATH`, then the attach on the tagged session in place of `rein start`. The tag is the one
/// positional word after it (`sh -c <script> sh <tag>`). Windows attaches through WSL only.
#[cfg(any(unix, test))]
pub fn attach_script() -> String {
    start_script().replacen("exec ~/.rein/bin/rein start", &attach_tail(), 1)
}

/// The attach script inside WSL: the same, printing the tty first (V6 "Client tty").
pub fn attach_script_wsl() -> String {
    start_script().replacen(
        "exec ~/.rein/bin/rein start",
        &format!("tty; {}", attach_tail()),
        1,
    )
}

/// One session of the app's server as the setup's lookup lists it (V12 component 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    /// tmux's id: `$` and digits.
    pub id: String,
    /// `@rein_app_project`: 64 hex, or empty when the session was never tagged.
    pub tag: String,
    /// The session's start directory.
    pub path: String,
}

/// What a tab opening on the app's server does (V12 components 2, 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    /// A session carries the tab's tag: attach to it.
    Live,
    /// No session carries the tag, but exactly one untagged session started in the tab's
    /// folder: attach to that id (the attach step then tags it).
    Adopt(String),
    /// Two or more untagged sessions started in the tab's folder: neither is guessed at.
    Two,
    /// No session for the folder (or no server): `rein start`.
    None,
}

/// The decision, in Rust (V12 component 9): `canon` makes a session's path canonical as the
/// tab's `project` already is (on Windows the path inside WSL is compared as written).
pub fn decide(
    rows: &[SessionRow],
    tag: &str,
    project: &str,
    canon: &dyn Fn(&str) -> String,
) -> Lookup {
    if rows.iter().any(|r| r.tag == tag) {
        return Lookup::Live;
    }
    let mine: Vec<&SessionRow> = rows
        .iter()
        .filter(|r| r.tag.is_empty() && canon(&r.path) == project)
        .collect();
    match mine.as_slice() {
        [] => Lookup::None,
        [one] => Lookup::Adopt(one.id.clone()),
        _ => Lookup::Two,
    }
}

/// `$` and digits: a tmux session id, the only form passed to the attach script.
pub fn session_id_ok(id: &str) -> bool {
    id.len() > 1 && id.starts_with('$') && id[1..].bytes().all(|b| b.is_ascii_digit())
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
        Some(26) => "the app's project tag was refused",
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

/// A finished setup run as the host reports it, with its lookup's rows: the lines between
/// `sessions` and `end` (a login shell's start-up text may come first). A row that is not
/// `s|$N|<64 hex or empty>|<path>` is left out; no `end` is no answer.
pub fn setup_outcome(
    run: std::io::Result<Option<rein_platform::process::Output>>,
) -> Result<Vec<SessionRow>, &'static str> {
    match run {
        Ok(Some((Some(0), out))) => {
            let text = String::from_utf8_lossy(&out);
            let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
            let start = lines.iter().rposition(|l| *l == "sessions");
            let end = lines.iter().rposition(|l| *l == "end");
            let (Some(a), Some(b)) = (start, end) else {
                return Err(setup_reason(Some(-1)));
            };
            if b < a {
                return Err(setup_reason(Some(-1)));
            }
            Ok(lines[a + 1..b]
                .iter()
                .filter_map(|l| {
                    let mut p = l.splitn(4, '|');
                    let (Some("s"), Some(id), Some(tag), Some(path)) =
                        (p.next(), p.next(), p.next(), p.next())
                    else {
                        return None;
                    };
                    let tag_ok = tag.is_empty() || crate::session_tag::is_tag(tag);
                    (session_id_ok(id) && tag_ok && path.starts_with('/')).then(|| SessionRow {
                        id: id.into(),
                        tag: tag.into(),
                        path: path.into(),
                    })
                })
                .collect())
        }
        Ok(Some((code, _))) => Err(setup_reason(code)),
        Ok(None) => Err(setup_reason(None)),
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

    /// V12 component 3: the attach scripts keep the argv rule, re-check the tag before any
    /// tmux call, check the session id, and never start rein.
    #[test]
    fn the_attach_scripts_are_argv_safe_and_never_start_rein() {
        for s in [attach_script(), attach_script_wsl()] {
            assert!(!s.contains(['\'', '"', '\n', '`', '\\']), "{s}");
            // An interactive bash may expand `!` history: only `! ` (test's negation) occurs.
            assert!(
                s.match_indices('!')
                    .all(|(i, _)| s[i + 1..].starts_with(' ')),
                "{s}"
            );
            assert!(s.starts_with("set -o noglob; H=$HOME/.local/state/rein-app/tmux; "));
            assert!(s.contains("unset TMUX TMUX_PANE; PATH=$H/bin:$PATH;"));
            assert!(!s.contains("rein start") && !s.contains(".rein/bin"), "{s}");
            let check = s
                .find("grep -qxE [0-9a-f]{64} || exit 75")
                .expect("tag check");
            assert!(
                check < s.find("tmux list-sessions").unwrap(),
                "the tag is checked first"
            );
            assert!(s.contains("grep -qxE [0-9]+ || exit 76; [ ${s%$n} = x$ ] || exit 76;"));
            assert!(s.ends_with("exec tmux attach -t ${s#x}"));
            // An interactive shell starts a comment at a word's `#`: none does here.
            assert!(!s.split_whitespace().any(|w| w.starts_with('#')), "{s}");
        }
        // The window names the "gone" exit by the same number (term.ts).
        assert!(
            include_str!("../ui/src/term.ts")
                .contains(&format!("export const ATTACH_GONE = {ATTACH_GONE};"))
        );
        assert_ne!(ATTACH_GONE, ATTACH_REFUSED);
        assert!(![0, 126, 127, crate::project_setup::INIT_FAILED].contains(&ATTACH_GONE));
        assert!(!attach_script().contains("tty;"));
        assert!(attach_script_wsl().contains("PATH=$H/bin:$PATH; tty; echo $1"));
    }

    /// V12 component 9 (E-90): the decision, as a table.
    #[test]
    fn a_tagged_session_attaches_one_untagged_in_the_folder_is_adopted_two_are_not_guessed() {
        let t = "b".repeat(64);
        let other = "c".repeat(64);
        let row = |id: &str, tag: &str, path: &str| SessionRow {
            id: id.into(),
            tag: tag.into(),
            path: path.into(),
        };
        let same = |p: &str| p.trim_end_matches('/').to_string();
        let d = |rows: &[SessionRow]| decide(rows, &t, "/home/me/p", &same);
        assert_eq!(d(&[]), Lookup::None);
        assert_eq!(
            d(&[row("$1", &t, "/elsewhere")]),
            Lookup::Live,
            "the tag wins wherever it started"
        );
        assert_eq!(
            d(&[row("$2", "", "/home/me/p/")]),
            Lookup::Adopt("$2".into()),
            "made canonical"
        );
        assert_eq!(
            d(&[row("$2", "", "/home/me/p"), row("$5", "", "/home/me/p")]),
            Lookup::Two
        );
        assert_eq!(
            d(&[row("$2", "", "/home/me/other")]),
            Lookup::None,
            "another folder is not adopted"
        );
        assert_eq!(
            d(&[row("$2", &other, "/home/me/p")]),
            Lookup::None,
            "tagged for another project"
        );
        assert_eq!(
            d(&[row("$2", &t, "/home/me/p"), row("$5", "", "/home/me/p")]),
            Lookup::Live
        );
        assert!(
            session_id_ok("$12")
                && !session_id_ok("$")
                && !session_id_ok("12")
                && !session_id_ok("$1;x")
        );
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
            Some(26),
            Some(9),
            None,
        ] {
            assert!(!setup_reason(code).is_empty());
        }
        let tag = "a".repeat(64);
        let out = format!(
            "motd from a login shell\nsessions\ns|$3|{tag}|/home/me/p\ns|$1||/home/me/q|with bar\ns|x9||/odd\ns|$4|short|/home/me/r\nnot a row\nend\n"
        );
        assert_eq!(
            setup_outcome(Ok(Some((Some(0), out.into_bytes())))),
            Ok(vec![
                SessionRow {
                    id: "$3".into(),
                    tag: tag.clone(),
                    path: "/home/me/p".into()
                },
                SessionRow {
                    id: "$1".into(),
                    tag: String::new(),
                    path: "/home/me/q|with bar".into()
                },
            ]),
            "a row of another shape is left out; a path may hold `|`"
        );
        assert_eq!(
            setup_outcome(Ok(Some((Some(0), b"sessions\nend\n".to_vec())))),
            Ok(vec![])
        );
        // No end line (cut off), or no lookup at all: not an answer the app acts on.
        for out in [&b""[..], b"sessions\ns|$1||/p\n", b"live\n"] {
            assert!(setup_outcome(Ok(Some((Some(0), out.to_vec())))).is_err());
        }
        assert_eq!(
            setup_outcome(Ok(Some((Some(26), vec![])))),
            Err("the app's project tag was refused")
        );
        assert_eq!(
            setup_outcome(Ok(None)),
            Err("the tmux setup did not finish within 15 seconds")
        );
        assert_eq!(
            setup_outcome(Err(std::io::Error::other("os words"))),
            Err("the tmux setup could not run")
        );
        assert_eq!(Server::from_setting(Some("user")), Server::User);
        assert_eq!(Server::from_setting(Some("nonsense")), Server::App);
        assert_eq!(Server::from_setting(None), Server::App);
    }
}
