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

fn main() {
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
