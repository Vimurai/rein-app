//! The real `wsl.exe` runner and the checks on what the probe returns.

use super::*;

/// `<SystemRoot>\System32\wsl.exe`, with `SystemRoot` only if it is drive-absolute: a
/// relative value would be resolved against the working directory, or by the pty layer
/// against every `PATH` entry (security review of E-43). Otherwise the default location.
pub fn wsl_exe(system_root: Option<String>) -> String {
    let root = system_root
        .filter(|r| crate::dirs::is_drive_path(r))
        .unwrap_or_else(|| "C:\\Windows".into());
    format!("{}\\System32\\wsl.exe", root.trim_end_matches('\\'))
}

/// The Windows UNC form of a Linux path in `distro` (the inverse of `wsl_path::to_linux`).
pub fn unc_of(distro: &str, linux: &str) -> String {
    format!(
        "\\\\wsl.localhost\\{distro}{}",
        linux.trim_end_matches('/').replace('/', "\\")
    )
}

/// The probe's three lines: distro name, home, shell (empty → `/bin/sh`).
pub(super) fn parse_probe(stdout: &[u8]) -> Result<Distro, IpcError> {
    let bad = |why: &str| err("wsl_failed", format!("the WSL distro probe returned {why}"));
    let text = std::str::from_utf8(stdout).map_err(|_| bad("text that is not UTF-8"))?;
    let mut lines = text.lines();
    let name = lines.next().unwrap_or_default();
    let home = lines.next().unwrap_or_default();
    let shell = lines.next().filter(|s| !s.is_empty()).unwrap_or("/bin/sh");
    check_name(name).map_err(|_| bad("an unusable distro name"))?;
    check_posix(home).map_err(|_| bad("an unusable home directory"))?;
    check_posix(shell).map_err(|_| bad("an unusable login shell"))?;
    Ok(Distro {
        name: name.into(),
        home: home.into(),
        shell: shell.into(),
    })
}

/// A distro name: `[A-Za-z0-9._-]{1,64}`.
pub fn check_name(s: &str) -> Result<(), &'static str> {
    let ok = (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    ok.then_some(()).ok_or("not a WSL distro name")
}

/// A home or shell path: absolute POSIX, printable ASCII, no space, no `"` or `\`.
pub fn check_posix(s: &str) -> Result<(), &'static str> {
    let ok = s.starts_with('/')
        && s.len() <= 4096
        && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
        && !s.contains(['"', '\\']);
    ok.then_some(()).ok_or("not an absolute POSIX path")
}

/// The real runner for non-pty `wsl.exe` calls: the shared bounded run (30 s, 64 KiB),
/// stderr discarded, `WSLENV` removed, and on Windows no console window.
pub struct SystemWsl;

impl Shell for SystemWsl {
    fn run(&self, program: &str, args: &[OsString]) -> io::Result<ShellOut> {
        self.run_within(program, args, CALL_LIMIT, CALL_CAP)
    }
}

impl WslShell for SystemWsl {
    fn run_within(
        &self,
        program: &str,
        args: &[OsString],
        limit: Duration,
        cap: u64,
    ) -> io::Result<ShellOut> {
        let mut cmd = Self::command(program, args);
        match rein_platform::process::output(&mut cmd, limit, cap)? {
            Some((code, stdout)) => Ok(ShellOut { code, stdout }),
            None => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "wsl.exe did not finish in time",
            )),
        }
    }

    fn run_script(
        &self,
        program: &str,
        args: &[OsString],
        script: &str,
        limit: Duration,
        alive: &dyn Fn() -> bool,
    ) -> io::Result<Option<i32>> {
        tmux_overlay::run_stdin(&mut Self::command(program, args), script, limit, alive)
    }

    fn run_script_output(
        &self,
        program: &str,
        args: &[OsString],
        script: &str,
        limit: Duration,
        cap: u64,
    ) -> io::Result<Option<rein_platform::process::Output>> {
        rein_platform::process::output_with_stdin(
            &mut Self::command(program, args),
            Some(script),
            limit,
            cap,
        )
    }
}

impl SystemWsl {
    fn command(program: &str, args: &[OsString]) -> Command {
        let mut cmd = Command::new(program);
        cmd.args(args).stderr(Stdio::null());
        for k in ENV_REMOVED {
            cmd.env_remove(k);
        }
        hide_console(&mut cmd);
        cmd
    }
}

use rein_platform::process::hide_console;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_exe_is_always_drive_absolute() {
        assert_eq!(
            wsl_exe(Some("D:\\Win".into())),
            "D:\\Win\\System32\\wsl.exe"
        );
        assert_eq!(
            wsl_exe(Some("C:\\Windows\\".into())),
            "C:\\Windows\\System32\\wsl.exe"
        );
        for bad in [None, Some(""), Some("x"), Some("\\\\srv\\w"), Some("C:rel")] {
            assert_eq!(
                wsl_exe(bad.map(String::from)),
                "C:\\Windows\\System32\\wsl.exe"
            );
        }
    }
}
