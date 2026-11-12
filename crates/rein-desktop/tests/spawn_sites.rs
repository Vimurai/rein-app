//! desktop.md §Look and feel v2 V5: the window is a GUI-subsystem program on Windows, so every
//! helper process it starts (`wsl.exe`, `rein-app`) must be started with `CREATE_NO_WINDOW`,
//! or a console window flashes up beside it. This enumerates the spawn sites in the window's
//! sources and in `rein-platform` and fails when one builds a `Command` without
//! `hide_console`. The ConPTY child (`CommandBuilder` in pty.rs) is a pseudo-console by
//! design and is not a `Command`.

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The source without its `#[cfg(test)] mod tests` block (tests build commands that never
/// run in the window) and without line comments.
fn production(text: &str) -> String {
    let body = match text.find("#[cfg(test)]\nmod tests") {
        Some(i) => &text[..i],
        None => text,
    };
    body.lines()
        .map(|l| match l.trim_start().starts_with("//") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `Command::new(` site, with the text up to the next site (where it is spawned).
fn sites(text: &str) -> Vec<&str> {
    let starts: Vec<usize> = text
        .match_indices("Command::new(")
        .map(|(i, _)| i)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &i)| &text[i..starts.get(n + 1).copied().unwrap_or(text.len())])
        .collect()
}

#[test]
fn every_helper_process_is_started_without_a_console_window() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    rust_files(&root.join("../rein-platform/src"), &mut files);
    let mut checked = Vec::new();
    let mut missing = Vec::new();
    for f in &files {
        // A Windows checkout may have CRLF line endings.
        let text = production(&std::fs::read_to_string(f).unwrap().replace("\r\n", "\n"));
        for site in sites(&text) {
            let name = f
                .strip_prefix(root)
                .unwrap_or(f)
                .display()
                .to_string()
                .replace('\\', "/");
            checked.push(name.clone());
            if !site.contains("hide_console(&mut cmd)") {
                missing.push(name);
            }
        }
        // A `Command` built any other way would escape this check.
        assert!(
            !text.contains("Command::new_") && !text.contains("process::Command {"),
            "{}: build processes with Command::new so this test sees them",
            f.display()
        );
    }
    assert!(
        missing.is_empty(),
        "spawned without hide_console: {missing:?}"
    );
    // The known sites: the CLI runner and the local doctor (cli.rs), the wsl.exe calls
    // (wsl_host.rs). A new one is fine as long as it hides the console; none may vanish
    // unnoticed because this test stopped matching.
    for expected in ["src/cli.rs", "src/wsl_host.rs"] {
        assert!(
            checked.iter().any(|c| c == expected),
            "no spawn site found in {expected}: {checked:?}"
        );
    }
}

#[test]
fn hide_console_sets_create_no_window() {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../rein-platform/src/process.rs"),
    )
    .unwrap();
    assert!(text.contains("const CREATE_NO_WINDOW: u32 = 0x0800_0000;"));
    assert!(text.contains("cmd.creation_flags(CREATE_NO_WINDOW);"));
}

#[test]
fn release_builds_are_gui_subsystem_on_windows() {
    let main =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs")).unwrap();
    assert!(main.contains(
        r#"#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]"#
    ));
}
