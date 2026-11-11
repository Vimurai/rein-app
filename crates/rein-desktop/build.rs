//! Tauri's build step (capability/ACL codegen). The web bundle must already exist: it is
//! built by `npm ci && npm run build` in `ui/` (CI does this before cargo).

fn main() {
    let dist = std::path::Path::new("ui/dist/index.html");
    println!("cargo:rerun-if-changed=ui/dist");
    if !dist.exists() {
        panic!(
            "rein-desktop: the web UI is not built. Run `npm ci && npm run build` in \
             crates/rein-desktop/ui, then build again."
        );
    }
    tauri_build::build()
}
