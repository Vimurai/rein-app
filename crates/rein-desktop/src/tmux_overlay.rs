//! The tmux overlay (desktop.md §Look and feel v2 V6, D-050): while a tab's tmux client is
//! attached, the session it shows takes the window's look; tmux undoes it on detach.
//!
//! The work is one constant POSIX `sh` script ([`SCRIPT`]) fed on stdin to
//! `sh -s -- <client tty> <status mode> <colour>…`. Every positional word is validated here
//! before any process starts (D-033 ruling 7): nothing from the web view, the config file or
//! the terminal becomes a command, a format or an option name. The script checks the words
//! again, and the session and window ids it uses come from tmux and are checked too.
//!
//! The host runs it (`LocalHost`: `/bin/sh -s`; `WslHost`: `wsl.exe … -e sh -ls`) with a
//! 15-second limit; any failure leaves tmux as the user has it and is shown as one line in
//! the health view.

use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// The overlay script: `sh -s -- <tty> <hidden|app|keep|off> <bg> <fg> <muted> <border> <accent>`.
pub const SCRIPT: &str = include_str!("tmux_overlay.sh");
/// The whole run, the script's own 10-second wait for the client included.
/// How long the script waits for the tab's tmux client (its own loop: 300 x 0.2 s). A fresh
/// `rein start` attaches seconds after the tab opens (E-56); the run ends earlier when the
/// tab's terminal ends (`run`'s `alive`).
pub const CLIENT_WAIT: Duration = Duration::from_secs(60);
/// The whole run: the client wait plus the time to style the session.
pub const LIMIT: Duration = Duration::from_secs(75);
/// The settings' `status` words; `off` (remove the overlay) is the app's own, used when the
/// `overlay` setting is switched off while a tab is attached.
pub const STATUS_MODES: &[&str] = &["hidden", "app", "keep"];
/// Only so much of a Windows tab's first output is searched for the `tty` line.
pub const TTY_LINE_WINDOW: usize = 4096;

/// The theme colours the overlay uses, as the web side's theme table holds them.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Colours {
    pub bg: String,
    pub fg: String,
    pub muted: String,
    pub border: String,
    pub accent: String,
}

/// A client tty as tmux names it: `/dev/pts/N` (Linux, WSL) or `/dev/ttysN` (macOS).
pub fn tty_ok(s: &str) -> bool {
    let digits =
        |d: &str, max: usize| (1..=max).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit());
    if let Some(d) = s.strip_prefix("/dev/pts/") {
        digits(d, 5)
    } else if let Some(d) = s.strip_prefix("/dev/ttys") {
        digits(d, 4)
    } else {
        false
    }
}

/// `#rrggbb`, lower case — the only colour form that reaches the script.
pub fn colour_ok(s: &str) -> bool {
    s.len() == 7
        && s.starts_with('#')
        && s[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The script's positional words, every one validated. `mode` is a settings word or `off`.
pub fn args(tty: &str, mode: &str, c: &Colours) -> Result<Vec<String>, String> {
    if !tty_ok(tty) {
        return Err("the terminal's tty is not one tmux would name".into());
    }
    if !(STATUS_MODES.contains(&mode) || mode == "off") {
        return Err(format!("{mode:?} is not a status mode"));
    }
    let colours = [&c.bg, &c.fg, &c.muted, &c.border, &c.accent];
    if let Some(bad) = colours.iter().find(|c| !colour_ok(c)) {
        return Err(format!(
            "colour {:?} is not #rrggbb",
            bad.chars().take(16).collect::<String>()
        ));
    }
    let mut v = vec![tty.to_string(), mode.to_string()];
    v.extend(colours.into_iter().cloned());
    Ok(v)
}

/// Plain words for the script's exit codes (see its header).
pub fn reason(code: Option<i32>) -> String {
    match code {
        Some(0) => "applied".into(),
        Some(2) => "the overlay was given a value it does not accept".into(),
        Some(3) => "tmux was not found".into(),
        Some(4) => "tmux is older than 3.2".into(),
        Some(5) => format!(
            "the tab's tmux client was not attached within {} seconds",
            CLIENT_WAIT.as_secs()
        ),
        Some(6) => "tmux refused an option".into(),
        Some(c) => format!("the overlay exited with code {c}"),
        None => format!(
            "the overlay did not finish within {} seconds",
            LIMIT.as_secs()
        ),
    }
}

/// Run `cmd` with [`SCRIPT`] on stdin, its output discarded, for at most `limit`, and only while
/// `alive()` (the tab's terminal is still there — E-56: the script may wait a minute for a
/// client that a failed `rein start` never attaches). `Ok(None)`: it was killed, at the limit
/// or because `alive()` turned false.
pub fn run(
    cmd: &mut Command,
    limit: Duration,
    alive: &dyn Fn() -> bool,
) -> io::Result<Option<i32>> {
    let deadline = Instant::now() + limit;
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        // On its own thread: the script is larger than a Windows pipe's buffer, and a child
        // that does not read (a WSL VM still booting) must not hold this past `limit`. A
        // child killed at the deadline closes the pipe and ends the write.
        std::thread::spawn(move || {
            let _ = stdin.write_all(SCRIPT.as_bytes());
        });
    }
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status.code().unwrap_or(-1)));
        }
        if Instant::now() >= deadline || !alive() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A finished run as the host reports it: `Ok` when the script exited 0, else why not.
pub fn outcome(run: io::Result<Option<i32>>) -> Result<(), String> {
    match run {
        Ok(Some(0)) => Ok(()),
        Ok(code) => Err(reason(code)),
        Err(e) => Err(format!("the overlay could not start: {e}")),
    }
}

/// The last overlay failure, for the health view; cleared by the next success.
#[derive(Default)]
pub struct OverlayHealth(Mutex<Option<String>>);

impl OverlayHealth {
    pub fn record(&self, outcome: &Result<(), String>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = outcome.clone().err();
    }

    pub fn failure(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Windows (V6 "Client tty"): the start script prints `tty` before `exec rein start`. In one
/// chunk of the first [`TTY_LINE_WINDOW`] bytes of output, a `/dev/pts/N` that starts a line
/// (or follows an escape sequence, as ConPTY draws it) and ends one is taken out: the tty and
/// the chunk without it. Anything else → `None` and the chunk is shown untouched.
pub fn take_tty_line(chunk: &[u8]) -> Option<(String, Vec<u8>)> {
    const P: &[u8] = b"/dev/pts/";
    let start = chunk.windows(P.len()).position(|w| w == P)?;
    if !line_start(&chunk[..start]) {
        return None;
    }
    let digits = chunk[start + P.len()..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    let end = start + P.len() + digits;
    let tty = std::str::from_utf8(&chunk[start..end]).ok()?;
    if !tty_ok(tty) {
        return None;
    }
    // The line ends: an erase-to-end-of-line ConPTY may add, then CR LF, LF, or more drawing.
    let mut rest = &chunk[end..];
    if let Some(r) = rest.strip_prefix(b"\x1b[K") {
        rest = r;
    }
    let cut = if let Some(r) = rest.strip_prefix(b"\r\n") {
        r
    } else if let Some(r) = rest.strip_prefix(b"\n") {
        r
    } else if rest.is_empty() || rest.starts_with(b"\x1b") || rest.starts_with(b"\r") {
        rest
    } else {
        return None;
    };
    let mut out = chunk[..start].to_vec();
    out.extend_from_slice(cut);
    Some((tty.to_string(), out))
}

/// tmux has started drawing (it switches to the alternate screen when it attaches): the
/// `tty` line comes before that, so nothing later is searched (security review of E-51, F6).
pub fn drawing_started(chunk: &[u8]) -> bool {
    chunk.windows(8).any(|w| w == b"\x1b[?1049h")
}

/// Whether `before` ends where a line starts: nothing, a line break, or a complete CSI
/// sequence (`ESC [` parameters, a final byte).
fn line_start(before: &[u8]) -> bool {
    match before.last() {
        None | Some(b'\n' | b'\r') => true,
        Some(&f) if (0x40..=0x7e).contains(&f) => {
            let params = before[..before.len() - 1]
                .iter()
                .rev()
                .take_while(|b| (0x20..=0x3f).contains(*b))
                .count();
            let at = before.len() - 1 - params;
            at >= 2 && &before[at - 2..at] == b"\x1b["
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    /// E-56: a fresh `rein start` attaches seconds after the tab opens; the script waits for
    /// the client at least 30 s (dc0ad9f: 10 s), as long as `CLIENT_WAIT` says, and the run's
    /// limit leaves time to style the session after that.
    #[test]
    fn the_client_wait_outlasts_a_fresh_start() {
        let line = SCRIPT
            .lines()
            .find(|l| l.starts_with("while [ $i -lt "))
            .expect("the client wait loop");
        let n: u64 = line["while [ $i -lt ".len()..]
            .split(' ')
            .next()
            .and_then(|x| x.parse().ok())
            .expect("its count");
        let after = &SCRIPT[SCRIPT.find(line).unwrap()..];
        assert!(
            after.lines().take(5).any(|l| l.trim() == "sleep 0.2"),
            "polls every 0.2 s"
        );
        let wait = Duration::from_millis(n * 200);
        assert!(wait >= Duration::from_secs(30), "the script waits {wait:?}");
        assert_eq!(wait, CLIENT_WAIT);
        assert!(LIMIT >= CLIENT_WAIT + Duration::from_secs(10));
        assert!(reason(Some(5)).contains("60 seconds"));
    }

    /// E-56: the run stops as soon as the tab's terminal is gone, not at the limit.
    #[cfg(unix)]
    #[test]
    fn a_run_stops_when_the_tab_is_gone() {
        let t0 = Instant::now();
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "sleep 30"]);
        let gone_at = t0 + Duration::from_millis(200);
        let out = run(&mut cmd, LIMIT, &|| Instant::now() < gone_at).unwrap();
        assert_eq!(out, None, "killed");
        assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    }

    use super::*;

    fn mocha() -> Colours {
        Colours {
            bg: "#1e1e2e".into(),
            fg: "#cdd6f4".into(),
            muted: "#a6adc8".into(),
            border: "#6c7086".into(),
            accent: "#cba6f7".into(),
        }
    }

    #[test]
    fn only_validated_words_reach_the_script() {
        assert_eq!(
            args("/dev/pts/3", "hidden", &mocha()).unwrap(),
            [
                "/dev/pts/3",
                "hidden",
                "#1e1e2e",
                "#cdd6f4",
                "#a6adc8",
                "#6c7086",
                "#cba6f7"
            ]
        );
        assert!(args("/dev/ttys004", "app", &mocha()).is_ok());
        assert!(args("/dev/pts/3", "off", &mocha()).is_ok());
        for tty in [
            "",
            "/dev/pts/",
            "/dev/pts/3 ",
            "/dev/pts/123456",
            "/dev/pts/3;x",
            "/dev/tty1",
            "/dev/ttys",
            "/dev/ttys12345",
            "/dev/pts/../sda",
            "/tmp/pts/3",
        ] {
            assert!(args(tty, "hidden", &mocha()).is_err(), "{tty:?}");
        }
        for mode in ["", "Hidden", "on", "keep;", "app "] {
            assert!(args("/dev/pts/3", mode, &mocha()).is_err(), "{mode:?}");
        }
        for bad in [
            "#1E1E2E", "#1e1e2", "1e1e2e1", "#1e1e2g", "red", "#1e1e2e;", "",
        ] {
            let mut c = mocha();
            c.accent = bad.into();
            assert!(args("/dev/pts/3", "app", &c).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_script_is_the_constant_file_and_its_hook_text_is_safe_for_tmux_and_sh() {
        assert!(SCRIPT.contains("client-detached[4271]"));
        assert!(SCRIPT.contains("[ $# -eq 7 ] || exit 2"));
        // The restore text sits inside tmux single quotes in the hook: none may appear in it.
        let line = SCRIPT
            .lines()
            .find(|l| l.starts_with("RESTORE=$(printf '"))
            .unwrap();
        let body = &line["RESTORE=$(printf '".len()..line.len() - " \\".len() - 1];
        assert!(!body.contains('\''), "{body}");
        // The client name never reaches the shell: tmux compares it (security review of
        // E-51, F1 — `#{q:}` leaves a newline or tab through).
        assert!(SCRIPT.contains("[ #{==:#{hook_client},$tty} = 1 ]"));
        assert!(!SCRIPT.contains("#{q:hook_client}"));
        assert!(!SCRIPT.contains("\"#{hook_client}\""));
    }

    #[cfg(unix)]
    #[test]
    fn a_child_that_never_reads_its_input_is_still_stopped_at_the_limit() {
        // `sleep` reads nothing: the script stays in the pipe (security review of E-51).
        let start = Instant::now();
        let mut cmd = Command::new("/bin/sleep");
        cmd.arg("30");
        assert_eq!(
            run(&mut cmd, Duration::from_millis(300), &|| true).unwrap(),
            None
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "exit 4"]);
        assert_eq!(
            run(&mut cmd, Duration::from_secs(5), &|| true).unwrap(),
            Some(4)
        );
    }

    #[test]
    fn exit_codes_have_plain_reasons() {
        assert_eq!(reason(Some(4)), "tmux is older than 3.2");
        assert_eq!(
            reason(Some(5)),
            "the tab's tmux client was not attached within 60 seconds"
        );
        assert!(reason(None).contains("75 seconds"));
    }

    #[test]
    fn the_windows_tty_line_is_taken_out_and_anything_else_passes_untouched() {
        /// Output in, and the tty taken out with what is left shown (`None`: untouched).
        type Case = (&'static [u8], Option<(&'static str, &'static [u8])>);
        let cases: &[Case] = &[
            (b"/dev/pts/3\r\nnext", Some(("/dev/pts/3", b"next"))),
            (b"/dev/pts/12\n", Some(("/dev/pts/12", b""))),
            (
                b"\x1b[?25l\x1b[2J\x1b[m\x1b[H/dev/pts/7\x1b[K\r\n\x1b[?25h",
                Some(("/dev/pts/7", b"\x1b[?25l\x1b[2J\x1b[m\x1b[H\x1b[?25h")),
            ),
            (b"motd\r\n/dev/pts/4\r\n", Some(("/dev/pts/4", b"motd\r\n"))),
            (b"/dev/pts/4\x1b[2;1H", Some(("/dev/pts/4", b"\x1b[2;1H"))),
            // Not a line of its own, not a tty, or no tty at all: shown as it came.
            (b"echo /dev/pts/3\r\n", None),
            (b"/dev/pts/3x\r\n", None),
            (b"/dev/pts/\r\n", None),
            (b"/dev/pts/1234567\r\n", None),
            (b"not a tty\r\n", None),
            (b"x\x1b[/dev/pts/3\r\n", None),
            (b"", None),
        ];
        for (input, want) in cases {
            let got = take_tty_line(input);
            let want = want.map(|(t, rest)| (t.to_string(), rest.to_vec()));
            assert_eq!(got, want, "{:?}", String::from_utf8_lossy(input));
        }
    }

    #[test]
    fn the_search_stops_once_tmux_draws() {
        assert!(drawing_started(b"\x1b[?1049h\x1b[H"));
        assert!(drawing_started(b"x\x1b[?1049h"));
        assert!(!drawing_started(b"\x1b[?1049l"));
        assert!(!drawing_started(b"/dev/pts/3\r\n"));
    }

    #[test]
    fn health_keeps_the_last_failure_until_a_success() {
        let h = OverlayHealth::default();
        assert_eq!(h.failure(), None);
        h.record(&Err("tmux is older than 3.2".into()));
        assert_eq!(h.failure().as_deref(), Some("tmux is older than 3.2"));
        h.record(&Ok(()));
        assert_eq!(h.failure(), None);
    }
}
