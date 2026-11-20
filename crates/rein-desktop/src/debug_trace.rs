//! Debug builds only: four traces a window run reads from stderr (tests/e2e/home-screens.sh,
//! tests/e2e/key-fields.sh; E-80, E-98, E-99). Release builds ignore all four variables, as they
//! do `REIN_DESKTOP_RELOAD_AFTER_MS`.
//! - `REIN_DESKTOP_TRACE_FRAMES`: once a second the core asks the page for the longest gap
//!   between two animation frames since the last ask (a `requestAnimationFrame` loop the first
//!   ask installs) and prints `frame-trace <unix s> max_ms=<n> frames=<n>`.
//! - `REIN_DESKTOP_TRACE_PTY`: one `pty-trace <unix ms> pty=<id> <cols>x<rows>` line per
//!   `pty_resize`, so a run can show that going Home and back resizes no pty.
//! - `REIN_DESKTOP_TRACE_STAGE` (E-98): once a second, what the stage shows —
//!   `stage-trace <unix s> home=<0|1> setup=<0|1> tabs=<n> current=<the current tab's label>`
//!   (a label is the project's folder name).
//! - `REIN_DESKTOP_TRACE_KEYS` (E-99): the fields of every keydown the page receives —
//!   `key-trace <unix s> {"key":…,"code":…,"ctrlKey":…,"shiftKey":…,"altKey":…,"metaKey":…}`,
//!   one line per key, read once a second (a capture listener the first ask installs).
//!
//! None reads or prints any text of a terminal; the only project text printed is the current
//! tab's label (its folder name), by the stage trace.

#[cfg(debug_assertions)]
const FRAME_PROBE: &str = "(() => { const w = window; if (!w.__reinFrames) { w.__reinFrames = { max: 0, n: 0, last: performance.now() }; const f = (t) => { const s = w.__reinFrames; s.max = Math.max(s.max, t - s.last); s.last = t; s.n += 1; requestAnimationFrame(f); }; requestAnimationFrame(f); } const s = w.__reinFrames; const out = Math.round(s.max) + ' ' + s.n; s.max = 0; s.n = 0; return out; })()";

#[cfg(debug_assertions)]
const STAGE_PROBE: &str = "(() => { const sel = document.querySelector('[role=tab][aria-selected=true]:not([data-home])'); return [document.getElementById('home') || document.querySelector('[data-home][aria-selected=true]') ? 1 : 0, document.getElementById('setup-card') ? 1 : 0, document.querySelectorAll('[role=tab]:not([data-home])').length, sel ? (sel.textContent || '').trim() : '-'].join(' '); })()";

#[cfg(debug_assertions)]
const KEYS_PROBE: &str = "(() => { const w = window; if (!w.__reinKeys) { w.__reinKeys = []; w.addEventListener('keydown', (e) => w.__reinKeys.push({ key: e.key, code: e.code, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey, metaKey: e.metaKey }), true); } const out = JSON.stringify(w.__reinKeys); w.__reinKeys = []; return out; })()";

/// Starts the frame, stage and key traces when their variables are set (debug builds).
pub fn frames(window: &tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    if crate::dirs::process_env("REIN_DESKTOP_TRACE_KEYS").is_some() {
        let w = window.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let r = w.eval_with_callback(KEYS_PROBE, |answer| {
                    // The page returns a JSON string holding a JSON array.
                    let list = serde_json::from_str::<String>(&answer)
                        .ok()
                        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
                        .unwrap_or_default();
                    for k in list {
                        eprintln!("key-trace {} {k}", unix_ms() / 1000);
                    }
                });
                if r.is_err() {
                    return;
                }
            }
        });
    }
    #[cfg(debug_assertions)]
    if crate::dirs::process_env("REIN_DESKTOP_TRACE_STAGE").is_some() {
        let w = window.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let r = w.eval_with_callback(STAGE_PROBE, |answer| {
                    let a = answer.trim_matches('"');
                    let mut it = a.splitn(4, ' ');
                    let (home, setup, tabs, current) = (
                        it.next().unwrap_or("?"),
                        it.next().unwrap_or("?"),
                        it.next().unwrap_or("?"),
                        it.next().unwrap_or("?"),
                    );
                    eprintln!(
                        "stage-trace {} home={home} setup={setup} tabs={tabs} current={current}",
                        unix_ms() / 1000
                    );
                });
                if r.is_err() {
                    return;
                }
            }
        });
    }
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
