//! `rein-desktop [DIR]` — the rein-app window (desktop.md, D-033, D-034).
//!
//! E-34: the window, its security settings and the grid shell drawn from static data. The
//! terminal stage (E-35), modes (E-36), config (E-37), projects and tabs (E-38) and the
//! health / release / profile actions (E-39) build on this.

mod actions;
mod config;
mod dirs;
#[cfg_attr(not(windows), allow(dead_code))]
mod handoff_dir;
mod host;
mod ipc;
#[cfg(unix)]
mod local_host;
mod project_path;
mod pty;
mod pty_sys;
mod security;
#[cfg(unix)]
mod single_instance;
#[cfg(windows)]
#[path = "single_instance_windows.rs"]
mod single_instance;
// The Windows host: plain Rust everywhere, so its tests run on every platform; only the
// Windows build starts it.
#[cfg_attr(not(windows), allow(dead_code))]
mod wsl_host;
#[cfg_attr(not(windows), allow(dead_code))]
mod wsl_path;

use std::process::ExitCode;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// The window label the capability file names.
const MAIN_WINDOW: &str = "main";
/// Event carrying a DIR handed over by a second start: on Linux / macOS it passed
/// `project_path::check`, on Windows `check_text` (printable, bounded); either way `tab_open`
/// validates it again through the host before anything runs.
const EVENT_OPEN_DIR: &str = "open_dir";
/// Event carrying the config after a save of config.toml (desktop.md §API).
const EVENT_CONFIG_CHANGED: &str = "config_changed";

fn usage() -> ExitCode {
    eprintln!("usage: rein-desktop [DIR]");
    ExitCode::from(64)
}

/// The DIR argument, or `None` when no DIR was given. On Linux / macOS it is made absolute
/// and must pass the path rule; on Windows it is kept as typed (a `\\wsl.localhost\…` or a
/// Linux path) after the text rule, and mapped into WSL when the tab opens (W3).
fn parse_args(args: &[String], windows: bool) -> Result<Option<String>, ExitCode> {
    match args {
        [] => Ok(None),
        [a] if a == "-h" || a == "--help" => {
            println!(
                "usage: rein-desktop [DIR]\nOpen the rein-app window, with DIR as the first tab."
            );
            Err(ExitCode::SUCCESS)
        }
        [a] if a.starts_with('-') => Err(usage()),
        [a] if windows => {
            project_path::check_text(a).map_err(|why| {
                eprintln!("rein-desktop: {why}");
                ExitCode::from(1)
            })?;
            Ok(Some(a.clone()))
        }
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

/// Linux / macOS: projects are local directories (`LocalHost`). Windows: Rein lives inside
/// WSL and every process is `wsl.exe` (`WslHost`, §Windows window W1–W2), in the distro
/// `config.toml` names (`wsl.distro`), else WSL's default.
fn platform_host(_distro: Option<String>) -> Box<dyn host::Host> {
    #[cfg(unix)]
    return Box::new(local_host::LocalHost::new(
        dirs::process_env("HOME").map(std::path::PathBuf::from),
    ));
    #[cfg(windows)]
    return Box::new(wsl_host::WslHost::system(_distro));
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match parse_args(&args, cfg!(windows)) {
        Ok(d) => d,
        Err(code) => return code,
    };

    let listener = {
        let Some(inst_dir) = single_instance::instance_dir(&dirs::process_env) else {
            eprintln!("rein-desktop: cannot determine a runtime or state directory (set HOME)");
            return ExitCode::from(1);
        };
        match single_instance::acquire(&inst_dir, dir.as_deref()) {
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

    // config.toml (E-37): read before the window is built — decorations, opacity and the WSL
    // distro are start-time settings; the rest reloads live.
    let config = std::sync::Arc::new(config::ConfigService::open(dirs::config_dir(
        &dirs::process_env,
    )));
    let start = config.view().config;
    let builder = tauri::Builder::default()
        .manage(ipc::Desktop::new(platform_host(start.wsl.distro.clone())))
        .manage(std::sync::Arc::clone(&config))
        .invoke_handler(tauri::generate_handler![
            ipc::tab_open,
            ipc::pty_write,
            ipc::pty_resize,
            ipc::pty_ack,
            ipc::tab_detach,
            ipc::clipboard_write,
            ipc::dir_complete,
            ipc::config_get
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
                    .decorations(start.window.decorations);
            // Opacity below 1 needs a transparent window, decided at creation (macOS would
            // need Tauri's private-API feature, which this app does not enable).
            #[cfg(not(target_os = "macos"))]
            let window = window.transparent(start.window.opacity < 1.0);
            let window = window.build()?;
            // A save of config.toml reaches the web side as `config_changed`.
            {
                let handle = app.handle().clone();
                let config = std::sync::Arc::clone(&config);
                std::thread::spawn(move || {
                    loop {
                        std::thread::sleep(config::POLL);
                        if let Some(view) = config.reload_if_changed() {
                            if let Some(w) = handle.get_webview_window(MAIN_WINDOW) {
                                let _ = w.set_decorations(view.config.window.decorations);
                            }
                            let _ = handle.emit(EVENT_CONFIG_CHANGED, view);
                        }
                    }
                });
            }
            // Debug builds only: `REIN_DEVTOOLS=1` opens the web inspector.
            #[cfg(debug_assertions)]
            if std::env::var_os("REIN_DEVTOOLS").is_some() {
                window.open_devtools();
            }
            #[cfg(not(debug_assertions))]
            let _ = window;
            let handle = app.handle().clone();
            single_instance::serve(listener, move |req| {
                let Some(w) = handle.get_webview_window(MAIN_WINDOW) else {
                    return;
                };
                let _ = w.unminimize();
                let _ = w.set_focus();
                if let Some(p) = req {
                    let _ = w.emit(EVENT_OPEN_DIR, p);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!());
    let result = result.map(|app| {
        app.run(|_app, event| {
            // Closing the window detaches every tab: the tmux sessions keep running, and
            // every pty child is reaped before the process ends (desktop.md §Components 2).
            if let tauri::RunEvent::Exit = event {
                _app.state::<ipc::Desktop>().detach_all();
            }
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
    #[cfg(unix)] // the Linux / macOS rule; Windows keeps the DIR as typed (below)
    fn args_take_at_most_one_dir() {
        assert_eq!(parse_args(&[], false).unwrap(), None);
        assert_eq!(
            parse_args(&s(&["/tmp/p"]), false).unwrap(),
            Some("/tmp/p".into())
        );
        let rel = parse_args(&s(&["p"]), false).unwrap().unwrap();
        assert!(
            std::path::Path::new(&rel).is_absolute(),
            "a relative DIR is made absolute"
        );
        assert!(parse_args(&s(&["/a", "/b"]), false).is_err());
        assert!(parse_args(&s(&["--bogus"]), false).is_err());
        assert!(parse_args(&s(&["/a\u{1b}b"]), false).is_err());
    }

    #[test]
    fn on_windows_the_dir_is_kept_as_typed_after_the_text_rule() {
        let unc = "\\\\wsl.localhost\\Ubuntu\\home\\me\\p";
        assert_eq!(parse_args(&s(&[unc]), true).unwrap(), Some(unc.into()));
        assert_eq!(
            parse_args(&s(&["/home/me/p"]), true).unwrap(),
            Some("/home/me/p".into())
        );
        assert!(parse_args(&s(&["/a\u{1b}b"]), true).is_err());
    }
}
