//! `WslLauncher` (blueprint §Components 8; Windows target only at the CLI, testable on
//! every platform): `wsl --status`; no WSL → offer `wsl --install` (declined → exit 20);
//! WSL without a distro → `wsl --install -d Ubuntu`; ready → `rein-app update --yes`
//! inside the distro, then Windows Terminal into `rein start` in the project directory.
//! Every `wsl.exe`/`wt.exe` call goes through [`Shell`]; argv only, never a command line.

// Only `launch` on Windows reaches this module; elsewhere it is exercised by its tests.
#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Exit when WSL is absent and the operator declines to install it (blueprint §API).
pub const EXIT_WSL_DECLINED: u8 = 20;
const EXIT_ERROR: u8 = 1;
/// Linux rein-app missing inside the distro (runtime missing, as `install`).
const EXIT_RUNTIME: u8 = 4;

/// One finished process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOut {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
}

impl ShellOut {
    fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs Windows programs. `Err` means the program could not be started (absent).
pub trait Shell {
    fn run(&self, program: &str, args: &[OsString]) -> io::Result<ShellOut>;
}

/// The real [`Shell`]: stdin/stderr inherited, stdout captured.
pub struct SystemShell;

impl Shell for SystemShell {
    fn run(&self, program: &str, args: &[OsString]) -> io::Result<ShellOut> {
        let out = std::process::Command::new(program)
            .args(args)
            .stderr(std::process::Stdio::inherit())
            .output()?;
        Ok(ShellOut {
            code: out.status.code(),
            stdout: out.stdout,
        })
    }
}

/// What `wsl.exe` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WslState {
    Absent,
    NoDistro,
    Ready,
}

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

/// `wsl.exe --list --quiet` prints UTF-16LE; decode either that or UTF-8.
fn decode(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).all(|&b| b == 0) {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

pub fn detect(shell: &dyn Shell) -> WslState {
    match shell.run("wsl.exe", &args(&["--status"])) {
        Ok(o) if o.ok() => {}
        _ => return WslState::Absent,
    }
    match shell.run("wsl.exe", &args(&["--list", "--quiet"])) {
        Ok(o) if o.ok() => {
            let names = decode(&o.stdout).replace(['\u{feff}', '\0'], "");
            if names.trim().is_empty() {
                WslState::NoDistro
            } else {
                WslState::Ready
            }
        }
        _ => WslState::NoDistro,
    }
}

fn ask(input: &mut dyn BufRead, question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    input.read_line(&mut answer).is_ok() && matches!(answer.trim(), "y" | "Y" | "yes" | "Yes")
}

pub fn launch(shell: &dyn Shell, input: &mut dyn BufRead, dir: &Path) -> u8 {
    // wt.exe splits its command line on ';' — a project path containing one would start a
    // second command. Refuse it rather than escape it.
    if dir.as_os_str().to_string_lossy().contains(';') {
        eprintln!(
            "rein-app: the project path contains ';', which Windows Terminal treats as a command separator"
        );
        return EXIT_ERROR;
    }
    match detect(shell) {
        WslState::Absent => {
            if !ask(
                input,
                "WSL2 is not installed. Install it now (administrator rights, then a restart)?",
            ) {
                eprintln!("rein-app: Rein on Windows runs inside WSL2; not installed");
                return EXIT_WSL_DECLINED;
            }
            match shell.run("wsl.exe", &args(&["--install"])) {
                Ok(o) if o.ok() => {
                    println!(
                        "rein-app: WSL2 installed — restart Windows, finish the Ubuntu setup, then run `rein-app launch` again"
                    );
                    0
                }
                _ => {
                    eprintln!(
                        "rein-app: `wsl --install` failed — run it in an administrator terminal"
                    );
                    EXIT_ERROR
                }
            }
        }
        WslState::NoDistro => match shell.run("wsl.exe", &args(&["--install", "-d", "Ubuntu"])) {
            Ok(o) if o.ok() => {
                println!(
                    "rein-app: Ubuntu installed — finish its first-run setup, then run `rein-app launch` again"
                );
                0
            }
            _ => {
                eprintln!("rein-app: `wsl --install -d Ubuntu` failed");
                EXIT_ERROR
            }
        },
        WslState::Ready => start(shell, dir),
    }
}

fn start(shell: &dyn Shell, dir: &Path) -> u8 {
    match shell.run("wsl.exe", &args(&["-e", "rein-app", "--version"])) {
        Ok(o) if o.ok() => {}
        _ => {
            eprintln!(
                "rein-app: rein-app is not installed inside WSL — install the Linux package there (Homebrew or .deb), then re-run"
            );
            return EXIT_RUNTIME;
        }
    }
    // 0 = updated, 10 = already current; anything else is reported by rein-app itself.
    match shell.run("wsl.exe", &args(&["-e", "rein-app", "update", "--yes"])) {
        Ok(o) if matches!(o.code, Some(0 | 10)) => {}
        Ok(o) => {
            return o
                .code
                .and_then(|c| u8::try_from(c).ok())
                .unwrap_or(EXIT_ERROR);
        }
        Err(e) => {
            eprintln!("rein-app: cannot run wsl.exe: {e}");
            return EXIT_ERROR;
        }
    }
    let mut session: Vec<OsString> = vec!["--cd".into(), dir.as_os_str().to_owned()];
    session.extend(args(&["-e", "rein", "start"]));
    let mut wt: Vec<OsString> = vec![OsStr::new("wsl.exe").to_owned()];
    wt.extend(session.iter().cloned());
    let result = match shell.run("wt.exe", &wt) {
        Ok(o) => Ok(o),
        // No Windows Terminal: run in this console.
        Err(_) => shell.run("wsl.exe", &session),
    };
    match result {
        Ok(o) if o.ok() => 0,
        _ => {
            eprintln!("rein-app: could not open a terminal into WSL");
            EXIT_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Answers by argv; records every call.
    struct Stub {
        answers: Vec<(&'static str, Vec<&'static str>, io::Result<ShellOut>)>,
        calls: RefCell<Vec<String>>,
    }

    fn out(code: i32, stdout: &[u8]) -> io::Result<ShellOut> {
        Ok(ShellOut {
            code: Some(code),
            stdout: stdout.to_vec(),
        })
    }

    impl Shell for Stub {
        fn run(&self, program: &str, a: &[OsString]) -> io::Result<ShellOut> {
            let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", a.join(" ")));
            for (p, want, answer) in &self.answers {
                if *p == program && want.iter().map(|s| s.to_string()).collect::<Vec<_>>() == a {
                    return match answer {
                        Ok(o) => Ok(o.clone()),
                        Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
                    };
                }
            }
            Err(io::Error::new(io::ErrorKind::NotFound, "absent"))
        }
    }

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn no_wsl_declined_exits_20_and_accepted_runs_wsl_install() {
        let none = Stub {
            answers: vec![],
            calls: RefCell::default(),
        };
        assert_eq!(
            launch(&none, &mut &b"n\n"[..], Path::new("C:\\p")),
            EXIT_WSL_DECLINED
        );
        assert_eq!(*none.calls.borrow(), ["wsl.exe --status"]);

        let installable = Stub {
            answers: vec![("wsl.exe", vec!["--install"], out(0, b""))],
            calls: RefCell::default(),
        };
        assert_eq!(
            launch(&installable, &mut &b"y\n"[..], Path::new("C:\\p")),
            0
        );
        assert_eq!(
            installable.calls.borrow().last().unwrap(),
            "wsl.exe --install"
        );
    }

    #[test]
    fn wsl_without_a_distro_installs_ubuntu() {
        let s = Stub {
            answers: vec![
                ("wsl.exe", vec!["--status"], out(0, b"")),
                ("wsl.exe", vec!["--list", "--quiet"], out(0, &utf16("\r\n"))),
                ("wsl.exe", vec!["--install", "-d", "Ubuntu"], out(0, b"")),
            ],
            calls: RefCell::default(),
        };
        assert_eq!(detect(&s), WslState::NoDistro);
        assert_eq!(launch(&s, &mut &b""[..], Path::new("C:\\p")), 0);
        assert_eq!(
            s.calls.borrow().last().unwrap(),
            "wsl.exe --install -d Ubuntu"
        );
    }

    #[test]
    fn ready_updates_inside_wsl_then_opens_windows_terminal_into_rein_start() {
        let s = Stub {
            answers: vec![
                ("wsl.exe", vec!["--status"], out(0, b"")),
                (
                    "wsl.exe",
                    vec!["--list", "--quiet"],
                    out(0, &utf16("Ubuntu\r\n")),
                ),
                (
                    "wsl.exe",
                    vec!["-e", "rein-app", "--version"],
                    out(0, b"rein-app 0.1.0"),
                ),
                (
                    "wsl.exe",
                    vec!["-e", "rein-app", "update", "--yes"],
                    out(10, b""),
                ),
                (
                    "wt.exe",
                    vec!["wsl.exe", "--cd", "C:\\proj", "-e", "rein", "start"],
                    out(0, b""),
                ),
            ],
            calls: RefCell::default(),
        };
        assert_eq!(launch(&s, &mut &b""[..], Path::new("C:\\proj")), 0);
        assert_eq!(s.calls.borrow().len(), 5);
    }

    #[test]
    fn missing_linux_rein_app_and_a_semicolon_path_are_refused() {
        let s = Stub {
            answers: vec![
                ("wsl.exe", vec!["--status"], out(0, b"")),
                ("wsl.exe", vec!["--list", "--quiet"], out(0, b"Ubuntu\n")),
            ],
            calls: RefCell::default(),
        };
        assert_eq!(launch(&s, &mut &b""[..], Path::new("C:\\p")), EXIT_RUNTIME);
        assert_eq!(
            launch(&s, &mut &b""[..], Path::new("C:\\a;calc")),
            EXIT_ERROR
        );
        assert_eq!(s.calls.borrow().len(), 3, "the ';' path runs nothing");
    }
}
