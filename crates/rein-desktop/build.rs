//! Tauri's build step: capability/ACL codegen, with the app's own commands declared, so each
//! one needs an explicit `allow-<command>` grant in the capability file (desktop.md
//! §Security 1: a closed IPC table).
//!
//! The web bundle is built by `npm ci && npm run build` in `ui/` (hosted CI does this before
//! cargo). A checkout without it — a clean worktree such as `rein ci run` uses — still
//! compiles and runs the tests: a placeholder page is written to the gitignored `ui/dist`
//! saying how to build the UI, and cargo prints a warning.

include!("src/ipc_commands.rs");

const PLACEHOLDER: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>rein-app</title></head>\n<body><p>The window's UI is not built. Run <code>npm ci --ignore-scripts &amp;&amp; npm run build</code> in crates/rein-desktop/ui, then build again.</p></body></html>\n";

/// `OUT_DIR/actions.rs` from `actions.json`: the closed Action list for Rust (desktop.md
/// §Data Model; the TypeScript side is generated from the same file by `ui/gen-actions.mjs`).
fn generate_actions() {
    println!("cargo:rerun-if-changed=actions.json");
    let text = std::fs::read_to_string("actions.json").expect("read actions.json");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("actions.json is JSON");
    let mut out = String::from(
        "/// The closed Action list, generated from `actions.json` by build.rs.\n\
         pub const ACTIONS: &[Action] = &[\n",
    );
    for a in doc["actions"].as_array().expect("an `actions` array") {
        let name = a["name"].as_str().expect("an action name");
        let arg = match a["arg"].as_str() {
            None => "ActionArg::None",
            Some("n") => "ActionArg::TabNumber",
            Some("name") => "ActionArg::ThemeName",
            Some("style") => "ActionArg::InputStyle",
            Some("dir") => "ActionArg::Dir",
            Some(other) => panic!("actions.json: unknown arg kind {other:?}"),
        };
        assert!(
            name.bytes().all(|b| b.is_ascii_lowercase() || b == b'.'),
            "actions.json: bad name {name:?}"
        );
        out.push_str(&format!("    Action {{ name: {name:?}, arg: {arg} }},\n"));
    }
    out.push_str("];\n");
    let dest = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("actions.rs");
    std::fs::write(dest, out).expect("write actions.rs");
}

fn main() {
    generate_actions();
    println!("cargo:rerun-if-changed=ui/dist");
    println!("cargo:rerun-if-changed=src/ipc_commands.rs");
    let dist = std::path::Path::new("ui/dist");
    if !dist.join("index.html").exists() {
        // A release build must carry the real UI, never the placeholder.
        if std::env::var("PROFILE").as_deref() == Ok("release") {
            panic!(
                "rein-desktop: release build without the web UI. Run `npm ci --ignore-scripts \
                 && npm run build` in crates/rein-desktop/ui first."
            );
        }
        println!(
            "cargo:warning=rein-desktop: ui/dist is not built — the window shows a placeholder. \
             Run `npm ci --ignore-scripts && npm run build` in crates/rein-desktop/ui."
        );
        std::fs::create_dir_all(dist).expect("create ui/dist");
        std::fs::write(dist.join("index.html"), PLACEHOLDER).expect("write placeholder");
    }
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build step");
}
