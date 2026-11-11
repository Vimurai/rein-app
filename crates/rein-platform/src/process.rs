//! A captured process run with a time limit and an output cap (D-032). Moved here from the
//! CLI's runner so the window's `wsl.exe` calls use the same code.

use std::io;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How a captured run ended: its exit code (`None`: killed by a signal) and its stdout.
pub type Output = (Option<i32>, Vec<u8>);

/// What stdout becomes when it exceeds the cap: it is not a document of the expected kind.
pub const OVER_CAP: &[u8] = b"(output over the size cap)";

/// Run `cmd` with stdin closed and stdout captured (stderr as the caller set it, inherited by
/// default). `Ok(None)`: it did not finish — exit and stdout closed — within `limit`; the
/// child was killed and reaped. Stdout over `cap` bytes is replaced by [`OVER_CAP`].
pub fn output(cmd: &mut Command, limit: Duration, cap: u64) -> io::Result<Option<Output>> {
    let deadline = Instant::now() + limit;
    cmd.stdin(Stdio::null()).stdout(Stdio::piped());
    let mut child = cmd.spawn()?;
    // stdout is read on its own thread, bounded, and the pipe is closed after the cap so a
    // runaway writer ends. A grandchild that keeps the pipe open is not waited for past the
    // deadline (D-032).
    let (tx, rx) = mpsc::channel();
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let read = io::Read::read_to_end(&mut io::Read::take(out, cap + 1), &mut buf);
            let _ = tx.send(read.map(|_| buf));
        });
    } else {
        let _ = tx.send(Ok(Vec::new()));
    }
    let code = loop {
        if let Some(status) = child.try_wait()? {
            break status.code();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    let mut stdout = match rx.recv_timeout(remaining) {
        Ok(read) => read?,
        Err(_) => return Ok(None), // the child is reaped; a holder of its stdout is not waited for
    };
    if stdout.len() as u64 > cap {
        stdout.clear();
        stdout.extend_from_slice(OVER_CAP);
    }
    Ok(Some((code, stdout)))
}
