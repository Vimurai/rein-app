//! `WslHost` (desktop.md §Windows window W2, D-038): the window on Windows. Rein stays inside
//! WSL; every process this host starts is `wsl.exe`, by absolute path, with a fixed argv
//! shape. Scripts are compile-time constants with no interpolation: the only variable argv
//! elements are the validated distro name, the validated shell path and the validated
//! project path (as `--cd`).
//!
//! Plain Rust on every platform, so its tests run everywhere against a stubbed [`Shell`];
//! only [`SystemWsl`] differs on Windows (no console window).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use rein_platform::wsl::{self, Shell, ShellOut, WslState};

use crate::host::Host;
use crate::ipc::{IpcError, err};
use crate::pty::ChildSpec;
use crate::tmux_overlay;
use crate::wsl_path::{self, PathError};

mod host_impl;
mod scripts;
mod system;

pub use scripts::*;
use system::parse_probe;
pub use system::*;

/// What the probe learned. Every field has passed [`check_name`] / [`check_posix`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    pub name: String,
    pub home: String,
    pub shell: String,
}

/// A [`Shell`] whose calls can carry their own limits: doctor (120 s, 4 MiB) and directory
/// listing (5 s, 64 KiB) differ from the 30 s default (§Windows window "Limits", W4).
pub trait WslShell: Shell {
    fn run_within(
        &self,
        program: &str,
        args: &[OsString],
        limit: Duration,
        cap: u64,
    ) -> io::Result<ShellOut>;

    /// Run `program args` with the tmux overlay script on stdin (V6), output discarded: the
    /// exit code, `None` when it did not finish within `limit`.
    fn run_script(
        &self,
        program: &str,
        args: &[OsString],
        script: &str,
        limit: Duration,
        alive: &dyn Fn() -> bool,
    ) -> io::Result<Option<i32>>;

    /// Run `program args` with a constant `script` on stdin, stdout captured up to `cap`.
    fn run_script_output(
        &self,
        program: &str,
        args: &[OsString],
        script: &str,
        limit: Duration,
        cap: u64,
    ) -> io::Result<Option<rein_platform::process::Output>>;
}

pub struct WslHost<S: WslShell + Send + Sync> {
    shell: S,
    /// Streams the `rein-app` CLI's lines (release and profile actions, W4).
    runner: Box<dyn crate::cli::Runner>,
    /// `%SystemRoot%\System32\wsl.exe` — never a `PATH` search.
    wsl_exe: String,
    /// `wsl.distro` from `config.toml` (E-37 supplies it; `None` = WSL's default).
    configured: Option<String>,
    probed: Mutex<Option<Distro>>,
}

impl WslHost<SystemWsl> {
    /// The real host: `wsl.exe` under `%SystemRoot%`.
    pub fn system(configured: Option<String>) -> Self {
        Self::new(
            SystemWsl,
            Box::new(crate::cli::SystemRunner),
            wsl_exe(std::env::var("SystemRoot").ok()),
            configured,
        )
    }
}

impl<S: WslShell + Send + Sync> WslHost<S> {
    pub fn new(
        shell: S,
        runner: Box<dyn crate::cli::Runner>,
        wsl_exe: String,
        configured: Option<String>,
    ) -> Self {
        Self {
            shell,
            runner,
            wsl_exe,
            configured,
            probed: Mutex::new(None),
        }
    }

    fn run(&self, args: &[&str]) -> io::Result<ShellOut> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        self.shell.run(&self.wsl_exe, &args)
    }

    fn run_within(&self, args: &[&str], limit: Duration, cap: u64) -> io::Result<ShellOut> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        self.shell.run_within(&self.wsl_exe, &args, limit, cap)
    }

    /// The window's distro name, if WSL has been probed successfully (recent projects
    /// record it, E-38).
    pub fn distro_name(&self) -> Option<String> {
        self.probed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|d| d.name.clone())
    }

    /// The window's distro, probed once (W2). Refusals name the command that fixes them.
    pub fn probe_distro(&self) -> Result<Distro, IpcError> {
        let mut probed = self.probed.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(d) = probed.as_ref() {
            return Ok(d.clone());
        }
        let d = self.probe()?;
        *probed = Some(d.clone());
        Ok(d)
    }

    fn probe(&self) -> Result<Distro, IpcError> {
        match wsl::detect(&self.shell, &self.wsl_exe) {
            WslState::Absent => {
                return Err(err(
                    "wsl_absent",
                    "WSL is not installed — run `wsl --install` in an administrator terminal, then restart Windows",
                ));
            }
            WslState::NoDistro => {
                return Err(err(
                    "wsl_no_distro",
                    "WSL has no Linux distribution — run `wsl --install -d Ubuntu`",
                ));
            }
            WslState::Ready => {}
        }
        let mut args: Vec<&str> = Vec::new();
        let configured = match &self.configured {
            Some(want) => Some(self.listed(want)?),
            None => None,
        };
        if let Some(name) = &configured {
            args.extend(["-d", name.as_str()]);
        }
        args.extend(["-e", "sh", "-c", PROBE_SCRIPT]);
        let out = self
            .run(&args)
            .ok()
            .filter(ShellOut::ok)
            .ok_or_else(|| err("wsl_failed", "wsl.exe did not answer the distro probe"))?;
        parse_probe(&out.stdout)
    }

    /// The configured distro as `wsl.exe --list` spells it, if it is installed.
    fn listed(&self, want: &str) -> Result<String, IpcError> {
        check_name(want).map_err(|why| err("wsl_distro_missing", why))?;
        let names = self
            .run(&["--list", "--quiet"])
            .ok()
            .filter(ShellOut::ok)
            .map(|o| wsl::distros(&o.stdout))
            .unwrap_or_default();
        names
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(want))
            .ok_or_else(|| {
                err(
                    "wsl_distro_missing",
                    format!("the WSL distro {want} set in config.toml is not installed"),
                )
            })
    }
}

#[cfg(test)]
mod tests;
