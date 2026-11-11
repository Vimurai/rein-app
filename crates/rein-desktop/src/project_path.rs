//! The `ProjectPath` text rule (desktop.md §Data Model, D-024 rule): a path the window
//! accepts is absolute, at most `MAX_PATH_BYTES`, and free of control and bidi characters,
//! so it can be drawn in the chrome without escaping. Directory checks (exists, contains
//! `.ai/`, canonical) belong to `tab_open` (E-35).

use std::path::{Path, PathBuf};

/// Longer than any real project path; bounds what a second start may hand over.
pub const MAX_PATH_BYTES: usize = 4096;

/// Control characters (C0, DEL, C1), bidi marks, overrides and isolates, zero-width
/// characters and line separators — the set Phase 1 escapes (`rein-release` `paths.rs`).
pub fn is_unsafe_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'
        )
}

/// The text rule without the absolute-path part: non-empty, bounded, printable. What a
/// Windows start may hand over before it is mapped into WSL (§Windows window W3).
pub fn check_text(s: &str) -> Result<&str, &'static str> {
    if s.is_empty() {
        return Err("empty path");
    }
    if s.len() > MAX_PATH_BYTES {
        return Err("path too long");
    }
    if s.chars().any(is_unsafe_char) {
        return Err("path contains a control or bidi character");
    }
    Ok(s)
}

/// `Ok(path)` when `s` satisfies the text rule and is a POSIX absolute path; otherwise the
/// reason, in plain words. POSIX on every platform: on Windows a project path is the Linux
/// path inside WSL (§Windows window W3).
pub fn check(s: &str) -> Result<PathBuf, &'static str> {
    let s = check_text(s)?;
    if !s.starts_with('/') {
        return Err("path is not absolute");
    }
    Ok(Path::new(s).to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_plain_absolute_path() {
        assert_eq!(
            check("/home/u/Dev/x").unwrap(),
            PathBuf::from("/home/u/Dev/x")
        );
    }

    #[test]
    fn refuses_relative_empty_long_and_unsafe() {
        assert!(check("").is_err());
        assert!(check("rel/dir").is_err());
        assert!(check(&format!("/{}", "a".repeat(MAX_PATH_BYTES))).is_err());
        for bad in [
            "/a\nb",
            "/a\u{1b}[31m",
            "/a\u{202e}b",
            "/a\u{200b}b",
            "/a\u{7f}",
        ] {
            assert!(check(bad).is_err(), "{bad:?}");
        }
    }
}
