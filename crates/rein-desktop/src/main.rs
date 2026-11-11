//! `rein-desktop [DIR]` — the rein-app window (desktop.md, D-033, D-034).
//!
//! E-34: the window, its security settings and the grid shell drawn from static data. The
//! terminal stage (E-35), modes (E-36), config (E-37), projects and tabs (E-38) and the
//! health / release / profile actions (E-39) build on this.

#[cfg(unix)]
mod dirs;
#[cfg(unix)]
mod ipc;
mod project_path;
#[cfg(unix)]
mod pty;
mod security;
#[cfg(unix)]
mod single_instance;

use std::process::ExitCode;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// The window label the capability file names.
const MAIN_WINDOW: &str = "main";
/// Event carrying a DIR handed over by a second start (escaped by construction: the path
/// passed `project_path::check`).
const EVENT_OPEN_DIR: &str = "open_dir";

fn usage() -> ExitCode {
    eprintln!("usage: rein-desktop [DIR]");
    ExitCode::from(64)
}

/// The DIR argument as an absolute path string, or `None` when no DIR was given.
fn parse_args(args: &[String]) -> Result<Option<String>, ExitCode> {
    match args {
        [] => Ok(None),
        [a] if a == "-h" || a == "--help" => {
            println!(
                "usage: rein-desktop [DIR]\nOpen the rein-app window, with DIR as the first tab."
            );
            Err(ExitCode::SUCCESS)
        }
        [a] if a.starts_with('-') => Err(usage()),
        [a] => {
            let abs = std::path::absolute(a).map_err(|_| usage())?;
            let s = abs.to_string_lossy().into_owned();
            project_path::check(&s).map_err(|why| {
                eprintln!("rein-desktop: {why}");
                ExitCode::from(1)
            })?;
            Ok(Some(s))
        }
        _ => Err(usage()),
    }
}

/// `globalThis.__REIN_START_DIR__ = <JSON string or null>;` — the DIR is JSON-encoded, so
/// no path can break out of the string literal; it has passed `project_path::check` too.
fn start_dir_script(dir: Option<&str>) -> String {
    format!(
        "Object.defineProperty(globalThis, '__REIN_START_DIR__', {{ value: {}, writable: false }});",
        serde_json::to_string(&dir).unwrap_or_else(|_| "null".into())
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match parse_args(&args) {
        Ok(d) => d,
        Err(code) => return code,
    };

    #[cfg(unix)]
    let listener = {
        let Some(sock_dir) = single_instance::socket_dir(&dirs::process_env) else {
            eprintln!("rein-desktop: cannot determine a runtime or state directory (set HOME)");
            return ExitCode::from(1);
        };
        match single_instance::acquire(&sock_dir, dir.as_deref()) {
            Ok(single_instance::Instance::HandedOff) => return ExitCode::SUCCESS,
            Ok(single_instance::Instance::Primary(l)) => l,
            Err(e) => {
                eprintln!("rein-desktop: single-instance lock: {e}");
                return ExitCode::from(1);
            }
        }
    };

    // X11 WM_CLASS and the Wayland app id come from the program name: `rein-app`, so i3,
    // sway and Hyprland rules can match the window (desktop.md "For tiling window managers").
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    glib::set_prgname(Some("rein-app"));

    let builder = tauri::Builder::default();
    #[cfg(unix)]
    let builder = builder
        .manage(ipc::Desktop::new(
            dirs::process_env("HOME").map(std::path::PathBuf::from),
        ))
        .invoke_handler(tauri::generate_handler![
            ipc::tab_open,
            ipc::pty_write,
            ipc::pty_resize,
            ipc::pty_ack,
            ipc::tab_detach
        ]);
    let result = builder
        .setup(move |app| {
            let window =
                WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
                    .title("rein-app")
                    .inner_size(1100.0, 700.0)
                    .min_inner_size(480.0, 240.0)
                    .on_navigation(security::navigation_allowed)
                    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                    // The start DIR is set before any page script runs, so the UI reads it at
                    // start instead of racing an event (see `start_dir_script`).
                    .initialization_script(start_dir_script(dir.as_deref()))
                    .build()?;
            // Debug builds only: `REIN_DEVTOOLS=1` opens the web inspector.
            #[cfg(debug_assertions)]
            if std::env::var_os("REIN_DEVTOOLS").is_some() {
                window.open_devtools();
            }
            #[cfg(not(debug_assertions))]
            let _ = window;
            #[cfg(unix)]
            {
                let handle = app.handle().clone();
                single_instance::serve(listener, move |req| {
                    let Some(w) = handle.get_webview_window(MAIN_WINDOW) else {
                        return;
                    };
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                    if let Some(p) = req {
                        let _ = w.emit(EVENT_OPEN_DIR, p.to_string_lossy());
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!());
    let result = result.map(|app| {
        app.run(|_app, event| {
            // Closing the window detaches every tab: the tmux sessions keep running, and
            // every pty child is reaped before the process ends (desktop.md §Components 2).
            #[cfg(unix)]
            if let tauri::RunEvent::Exit = event {
                _app.state::<ipc::Desktop>().detach_all();
            }
            #[cfg(not(unix))]
            let _ = (_app, event);
        })
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rein-desktop: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn the_start_dir_is_a_json_literal() {
        assert_eq!(
            start_dir_script(None),
            "Object.defineProperty(globalThis, '__REIN_START_DIR__', { value: null, writable: false });"
        );
        let s = start_dir_script(Some(r#"/a/"b'</script>\c"#));
        assert!(s.contains(r#"value: "/a/\"b'</script>\\c""#), "{s}");
    }

    #[test]
    fn args_take_at_most_one_dir() {
        assert_eq!(parse_args(&[]).unwrap(), None);
        assert_eq!(parse_args(&s(&["/tmp/p"])).unwrap(), Some("/tmp/p".into()));
        let rel = parse_args(&s(&["p"])).unwrap().unwrap();
        assert!(
            std::path::Path::new(&rel).is_absolute(),
            "a relative DIR is made absolute"
        );
        assert!(parse_args(&s(&["/a", "/b"])).is_err());
        assert!(parse_args(&s(&["--bogus"])).is_err());
        assert!(parse_args(&s(&["/a\u{1b}b"])).is_err());
    }
}
