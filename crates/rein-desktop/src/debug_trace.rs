//! Debug builds only: two traces a window run reads from stderr (tests/e2e/home-screens.sh,
//! E-80). Release builds ignore both variables, as they do `REIN_DESKTOP_RELOAD_AFTER_MS`.
//! - `REIN_DESKTOP_TRACE_FRAMES`: once a second the core asks the page for the longest gap
//!   between two animation frames since the last ask (a `requestAnimationFrame` loop the first
//!   ask installs) and prints `frame-trace <unix s> max_ms=<n> frames=<n>`.
//! - `REIN_DESKTOP_TRACE_PTY`: one `pty-trace <unix ms> pty=<id> <cols>x<rows>` line per
//!   `pty_resize`, so a run can show that going Home and back resizes no pty.
//!
//! Neither reads or prints any text of a project or a terminal.

#[cfg(debug_assertions)]
const FRAME_PROBE: &str = "(() => { const w = window; if (!w.__reinFrames) { w.__reinFrames = { max: 0, n: 0, last: performance.now() }; const f = (t) => { const s = w.__reinFrames; s.max = Math.max(s.max, t - s.last); s.last = t; s.n += 1; requestAnimationFrame(f); }; requestAnimationFrame(f); } const s = w.__reinFrames; const out = Math.round(s.max) + ' ' + s.n; s.max = 0; s.n = 0; return out; })()";

/// Starts the frame trace when its variable is set (debug builds only).
pub fn frames(window: &tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    if crate::dirs::process_env("REIN_DESKTOP_TRACE_FRAMES").is_some() {
        let w = window.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let r = w.eval_with_callback(FRAME_PROBE, |answer| {
                    let a = answer.trim_matches('"');
                    let mut it = a.split(' ');
                    let (max, n) = (it.next().unwrap_or("?"), it.next().unwrap_or("?"));
                    eprintln!("frame-trace {} max_ms={max} frames={n}", unix_ms() / 1000);
                });
                if r.is_err() {
                    return;
                }
            }
        });
    }
    #[cfg(not(debug_assertions))]
    let _ = window;
}

/// One line per pty resize when its variable is set (debug builds only).
pub fn pty_resize(pty: u32, cols: u32, rows: u32) {
    #[cfg(debug_assertions)]
    if crate::dirs::process_env("REIN_DESKTOP_TRACE_PTY").is_some() {
        eprintln!("pty-trace {} pty={pty} {cols}x{rows}", unix_ms());
    }
    #[cfg(not(debug_assertions))]
    let _ = (pty, cols, rows);
}

#[cfg(debug_assertions)]
fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
