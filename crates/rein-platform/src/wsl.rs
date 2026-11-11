//! WSL state detection (distribution.md §Components 8; desktop.md §Windows window W2). Used
//! by the CLI's `WslLauncher` and the window's `WslHost`. Every `wsl.exe` call goes through
//! [`Shell`]; argv only, never a command line.

use std::ffi::OsString;
use std::io;

/// One finished process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOut {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
}

impl ShellOut {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs Windows programs. `Err` means the program could not be started (absent) or did not
/// finish in time.
pub trait Shell {
    fn run(&self, program: &str, args: &[OsString]) -> io::Result<ShellOut>;
}

/// What `wsl.exe` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WslState {
    Absent,
    NoDistro,
    Ready,
}

pub fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

/// `wsl.exe --list --quiet` prints UTF-16LE, with or without a byte-order mark; decode
/// either that or UTF-8.
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        return utf16le(rest);
    }
    if bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).all(|&b| b == 0) {
        utf16le(bytes)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn utf16le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&units)
}

/// The distro names in `wsl.exe --list --quiet` output, in order.
pub fn distros(list_output: &[u8]) -> Vec<String> {
    decode(list_output)
        .replace(['\u{feff}', '\0'], "")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// `wsl.exe --status`, then `wsl.exe --list --quiet`. `program` is how to start `wsl.exe`:
/// the CLI passes the bare name, the window an absolute path.
pub fn detect(shell: &dyn Shell, program: &str) -> WslState {
    match shell.run(program, &args(&["--status"])) {
        Ok(o) if o.ok() => {}
        _ => return WslState::Absent,
    }
    match shell.run(program, &args(&["--list", "--quiet"])) {
        Ok(o) if o.ok() => {
            if distros(&o.stdout).is_empty() {
                WslState::NoDistro
            } else {
                WslState::Ready
            }
        }
        _ => WslState::NoDistro,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn distros_decode_utf16_and_utf8() {
        assert_eq!(
            distros(&utf16("\u{feff}Ubuntu\r\nDebian\r\n")),
            ["Ubuntu", "Debian"]
        );
        assert_eq!(distros(b"Ubuntu-24.04\n"), ["Ubuntu-24.04"]);
        assert!(distros(&utf16("\r\n")).is_empty());
    }

    struct Fixed(Vec<(Vec<&'static str>, Option<i32>, Vec<u8>)>);
    impl Shell for Fixed {
        fn run(&self, program: &str, a: &[OsString]) -> io::Result<ShellOut> {
            assert_eq!(program, "C:\\Windows\\System32\\wsl.exe");
            for (want, code, out) in &self.0 {
                if args(want) == a {
                    return Ok(ShellOut {
                        code: *code,
                        stdout: out.clone(),
                    });
                }
            }
            Err(io::Error::new(io::ErrorKind::NotFound, "absent"))
        }
    }

    #[test]
    fn detect_reports_absent_no_distro_and_ready() {
        let p = "C:\\Windows\\System32\\wsl.exe";
        assert_eq!(detect(&Fixed(vec![]), p), WslState::Absent);
        let status = (vec!["--status"], Some(0), vec![]);
        let empty = (vec!["--list", "--quiet"], Some(0), utf16("\r\n"));
        assert_eq!(
            detect(&Fixed(vec![status.clone(), empty]), p),
            WslState::NoDistro
        );
        let one = (vec!["--list", "--quiet"], Some(0), utf16("Ubuntu\r\n"));
        assert_eq!(detect(&Fixed(vec![status, one]), p), WslState::Ready);
    }
}
