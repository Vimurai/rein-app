//! `WslHost` against a stubbed `wsl.exe`: the shared stub, then the tests by subject.

use super::*;
use std::collections::VecDeque;
use std::sync::Arc;

const EXE: &str = "C:\\Windows\\System32\\wsl.exe";

/// Answers by exact argv; records every call.
#[derive(Default)]
struct Stub {
    answers: Vec<(Vec<String>, Option<i32>, Vec<u8>)>,
    calls: Mutex<VecDeque<Vec<String>>>,
    limits: Mutex<Vec<Duration>>,
}

impl Stub {
    fn on(mut self, args: &[&str], code: i32, out: &[u8]) -> Self {
        self.answers.push((
            args.iter().map(|s| s.to_string()).collect(),
            Some(code),
            out.to_vec(),
        ));
        self
    }
    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().iter().cloned().collect()
    }
}

impl WslShell for Stub {
    fn run_within(
        &self,
        program: &str,
        args: &[OsString],
        limit: Duration,
        _cap: u64,
    ) -> io::Result<ShellOut> {
        self.limits.lock().unwrap().push(limit);
        self.run(program, args)
    }

    fn run_script(
        &self,
        program: &str,
        args: &[OsString],
        _script: &str,
        limit: Duration,
        _alive: &dyn Fn() -> bool,
    ) -> io::Result<Option<i32>> {
        self.limits.lock().unwrap().push(limit);
        self.run(program, args).map(|o| o.code)
    }

    fn run_script_output(
        &self,
        program: &str,
        args: &[OsString],
        _script: &str,
        limit: Duration,
        _cap: u64,
    ) -> io::Result<Option<rein_platform::process::Output>> {
        self.limits.lock().unwrap().push(limit);
        self.run(program, args).map(|o| Some((o.code, o.stdout)))
    }
}

/// Records the CLI runs; answers with fixed lines and code.
#[derive(Default)]
struct Cli {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    code: Option<i32>,
}

impl crate::cli::Runner for Cli {
    fn run(
        &self,
        program: &Path,
        args: &[String],
        line: &mut dyn FnMut(String),
    ) -> io::Result<Option<i32>> {
        assert_eq!(program, Path::new(EXE));
        self.calls.lock().unwrap().push(args.to_vec());
        line("a line".into());
        Ok(self.code)
    }
}

impl Shell for Stub {
    fn run(&self, program: &str, args: &[OsString]) -> io::Result<ShellOut> {
        assert_eq!(program, EXE, "wsl.exe by absolute path only");
        let a: Vec<String> = args
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        self.calls.lock().unwrap().push_back(a.clone());
        for (want, code, out) in &self.answers {
            if *want == a {
                return Ok(ShellOut {
                    code: *code,
                    stdout: out.clone(),
                });
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "absent"))
    }
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn ready() -> Stub {
    Stub::default()
        .on(&["--status"], 0, b"")
        .on(&["--list", "--quiet"], 0, &utf16("Ubuntu\r\nDebian\r\n"))
        .on(
            &["-e", "sh", "-c", PROBE_SCRIPT],
            0,
            b"Ubuntu\n/home/me\n/usr/bin/zsh\n",
        )
}

fn host(s: Stub) -> WslHost<Stub> {
    WslHost::new(s, Box::new(Cli::default()), EXE.into(), None)
}

fn validate(path: &str) -> Vec<&str> {
    vec![
        "-d",
        "Ubuntu",
        "--cd",
        path,
        "-e",
        "sh",
        "-c",
        VALIDATE_SCRIPT,
    ]
}

mod commands;
mod projects;
