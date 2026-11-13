//! desktop.md §Test Plan "Capabilities": the capability file grants nothing beyond the IPC
//! table and window basics, no Tauri plugin is a dependency, and the CSP is asserted.

use std::path::Path;

/// Permissions a capability may grant: the window basics (listening for core events) and
/// the IPC command table. Each task that adds an IPC command adds its permission here.
const ALLOWED: &[&str] = &[
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    // E-38: `:qa` closes the window (a window basic, desktop.md §Test Plan "Capabilities").
    "core:window:allow-close",
    // E-49 (V2): the title bar is ours — dragging and our minimise / maximise buttons. With
    // allow-close these are exactly the four window permissions V2 lists.
    "core:window:allow-start-dragging",
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    // E-35 — desktop.md §API
    "allow-tab-open",
    "allow-pty-write",
    "allow-pty-resize",
    "allow-pty-ack",
    "allow-tab-detach",
    "allow-tab-tmux",
    // E-36: cmd.yank copies the last shown command (desktop.md §API, D-042).
    "allow-clipboard-write",
    // E-36: `:open` Tab completion of directories, through the host.
    "allow-dir-complete",
    // E-37: the config, its path and its last error.
    "allow-config-get",
    // E-38: recent projects.
    "allow-projects-list",
    "allow-projects-forget",
    // E-39: health, release and profiles through the rein-app CLI (D-045).
    "allow-health-get",
    "allow-release-check",
    "allow-release-apply",
    "allow-profiles-plan",
    "allow-profiles-apply",
    // E-40: the folder dialog (opened by the core — no plugin, D-046), paste, ui.json.
    "allow-project-choose",
    "allow-clipboard-read",
    "allow-ui-get",
    "allow-ui-set",
    "allow-panel-data",
    "allow-project-agents",
    "allow-project-close",
];

fn json(rel: &str) -> serde_json::Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap()
}

#[test]
fn every_capability_grants_only_allowed_permissions() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let cap: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        files += 1;
        assert_eq!(
            cap["windows"],
            serde_json::json!(["main"]),
            "{}",
            path.display()
        );
        assert!(
            cap.get("remote").is_none(),
            "no remote origin may reach IPC"
        );
        assert!(cap.get("webviews").is_none());
        for p in cap["permissions"].as_array().unwrap() {
            // An object form ({identifier, allow, deny}) is a scoped plugin permission.
            let id = p
                .as_str()
                .unwrap_or_else(|| panic!("scoped permission {p}"));
            assert!(
                ALLOWED.contains(&id),
                "permission {id} is not in the IPC table"
            );
        }
    }
    assert_eq!(files, 1, "one capability file");
    let conf = json("tauri.conf.json");
    assert_eq!(
        conf["app"]["security"]["capabilities"],
        serde_json::json!(["main"])
    );
}

#[test]
fn the_csp_is_default_src_self_with_no_escape_hatch() {
    let conf = json("tauri.conf.json");
    let sec = &conf["app"]["security"];
    assert_eq!(sec["csp"], "default-src 'self'");
    for hatch in [
        "dangerousDisableAssetCspModification",
        "devCsp",
        "assetProtocol",
        "dangerousRemoteDomainIpcAccess",
    ] {
        assert!(sec.get(hatch).is_none(), "{hatch} must not be set");
    }
    assert_eq!(conf["app"]["withGlobalTauri"], false);
    // Object.prototype is frozen by the UI itself once its libraries have loaded (ui/src/
    // harden.ts, called from main.tsx): Tauri's `freezePrototype` freezes it before xterm.js loads, which breaks
    // xterm (security review of E-35). Either way it is frozen before untrusted data arrives.
    assert!(sec.get("freezePrototype").is_none());
    let main = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/main.tsx"),
    )
    .unwrap()
    .replace("\r\n", "\n"); // a Windows checkout has CRLF line endings
    assert!(
        main.contains("\nhardenPrototypes();\n"),
        "main.tsx must freeze Object.prototype at start"
    );
    assert_eq!(
        conf["app"]["windows"],
        serde_json::json!([]),
        "the window is built in code, with the navigation guard"
    );
    assert!(
        conf["build"].get("devUrl").is_none(),
        "the UI is always the bundled one"
    );
}

#[test]
fn no_tauri_plugin_is_in_the_build() {
    // The lockfile sees every route a plugin could take: a direct dependency in any table
    // form, a tauri feature that pulls one in, or a transitive dependency.
    let lock = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    let lock = std::fs::read_to_string(lock).unwrap();
    let plugins: Vec<&str> = lock
        .lines()
        .filter_map(|l| l.strip_prefix("name = \""))
        .filter(|n| n.starts_with("tauri-plugin"))
        .collect();
    assert!(
        plugins.is_empty(),
        "Tauri plugin(s) in Cargo.lock: {plugins:?}"
    );
}

#[test]
fn the_bundled_ui_has_no_inline_script_or_style() {
    let html = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/index.html"))
        .unwrap();
    assert!(!html.contains("<style"), "inline style element");
    assert!(!html.contains("style="), "inline style attribute");
    for tag in html.split("<script").skip(1) {
        assert!(
            tag.split('>').next().unwrap().contains("src="),
            "inline script"
        );
    }
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "remote resource"
    );
}

/// desktop.md §Look and feel v2 V2 (E-49): the title bar is ours, and the window may be moved,
/// minimised, maximised and closed — exactly these four window permissions, nothing more.
#[test]
fn the_window_permissions_are_exactly_the_four_the_title_bar_needs() {
    let cap = json("capabilities/main.json");
    let mut window: Vec<&str> = cap["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p.as_str())
        .filter(|p| p.starts_with("core:window:"))
        .collect();
    window.sort_unstable();
    assert_eq!(
        window,
        [
            "core:window:allow-close",
            "core:window:allow-minimize",
            "core:window:allow-start-dragging",
            "core:window:allow-toggle-maximize",
        ]
    );
}
