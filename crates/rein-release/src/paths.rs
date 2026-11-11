//! Relative-path rules shared by the manifest, the verifier and the layout writer.
//!
//! A release path is relative to the Rein home, uses `/`, and never escapes it
//! (rein D-153 r2). Anything else is rejected; nothing is normalised.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// `s` with control characters (C0, DEL, C1) and bidi overrides escaped (`\u{1b}`), for error
/// text built from manifest, tarball or snapshot strings (D-023 r4).
/// A backslash is doubled, so a literal `\u{1b}` in the source cannot pass for an escaped
/// control character (D-032).
pub(crate) fn printable(s: &str) -> std::borrow::Cow<'_, str> {
    escaped(s, true)
}

/// [`printable`] for text built from LOCAL paths (layout errors): control characters are
/// escaped, but a backslash is kept as is, so a Windows path stays readable.
pub(crate) fn printable_path(s: &str) -> std::borrow::Cow<'_, str> {
    escaped(s, false)
}

fn escaped(s: &str, double_backslash: bool) -> std::borrow::Cow<'_, str> {
    // Control characters, bidi marks/overrides/isolates, zero-width and line separators.
    let unsafe_char = |c: char| {
        c.is_control()
            || matches!(
                c,
                '\u{061c}'
                    | '\u{200b}'..='\u{200f}'
                    | '\u{2028}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
                    | '\u{feff}'
            )
    };
    if !s
        .chars()
        .any(|c| (double_backslash && c == '\\') || unsafe_char(c))
    {
        return s.into();
    }
    s.chars()
        .map(|c| {
            if double_backslash && c == '\\' {
                "\\\\".to_string() // so a literal `\u{1b}` never reads as an escape (D-032)
            } else if unsafe_char(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect::<String>()
        .into()
}

/// Check that `p` is a safe release path: non-empty, relative, `/`-separated, with no
/// empty, `.` or `..` segment, no backslash, no `:` (Windows drive/ADS) and no NUL.
pub fn check_rel_path(p: &str) -> Result<(), String> {
    let shown = printable(p);
    if p.is_empty() {
        return Err("empty path".into());
    }
    if p.starts_with('/') {
        return Err(format!("absolute path '{shown}'"));
    }
    if let Some(c) = p.chars().find(|c| matches!(c, '\\' | ':' | '\0')) {
        return Err(format!("forbidden character {c:?} in '{shown}'"));
    }
    for seg in p.split('/') {
        match seg {
            "" => return Err(format!("empty segment in '{shown}'")),
            "." | ".." => return Err(format!("'{seg}' segment in '{shown}'")),
            _ => {}
        }
    }
    Ok(())
}

/// Check that `p` is printable ASCII, bytes 0x20–0x7E only (rein D-155 addendum 3, D-020).
/// This keeps the case rule below to the 26 letters A–Z, the same fold in rein's Node and here.
pub fn check_path_alphabet(p: &str) -> Result<(), String> {
    if p.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        Ok(())
    } else {
        Err(format!(
            "path {p:?} has a byte outside printable ASCII (0x20-0x7E)"
        ))
    }
}

/// Refuse two paths that are equal after ASCII-lowercasing (A–Z only) but differ in bytes:
/// on a case-insensitive filesystem one would overwrite the other (rein D-155 addendum 3,
/// D-020). Every directory prefix of each path counts as a path, so `shared/Lib` collides
/// with `shared/lib/x.mjs`. The first collision found is named, both paths in input order.
///
/// The paths are walked as a tree, one segment per step, so memory stays linear in the
/// input: storing every lowercased prefix would be quadratic in one long path's length.
pub fn check_case_collisions<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    // (parent node, lowercased segment) -> (child node, the first path seen there, the
    // byte offset where that prefix ends in it).
    let mut seen: HashMap<(usize, String), (usize, &'a str, usize)> = HashMap::new();
    for path in paths {
        let mut node = 0;
        let mut start = 0;
        for seg in path.split('/') {
            let end = start + seg.len();
            let next = seen.len() + 1;
            match seen.entry((node, seg.to_ascii_lowercase())) {
                Entry::Occupied(e) => {
                    let (child, first, first_end) = *e.get();
                    let theirs = &first[..first_end];
                    if theirs[theirs.len() - seg.len()..] != *seg {
                        return Err(format!("case collision: '{theirs}' and '{}'", &path[..end]));
                    }
                    node = child;
                }
                Entry::Vacant(e) => {
                    e.insert((next, path, end));
                    node = next;
                }
            }
            start = end + 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_case_collisions, check_path_alphabet, check_rel_path};

    /// D-032: a backslash is doubled, so a literal `\u{1b}` differs from an escaped ESC.
    #[test]
    fn printable_doubles_a_backslash_and_escapes_control_characters() {
        assert_eq!(super::printable(r"a\u{1b}b"), r"a\\u{1b}b");
        assert_eq!(super::printable("a\u{1b}b"), r"a\u{1b}b");
        assert_eq!(super::printable("plain"), "plain");
        assert_eq!(
            super::printable_path(r"C:\h\.rein"),
            r"C:\h\.rein",
            "a local path keeps it"
        );
        assert_eq!(super::printable_path("a\u{1b}b"), r"a\u{1b}b");
    }

    #[test]
    fn the_alphabet_is_printable_ascii() {
        assert!(check_path_alphabet(" bin/~rein-A_z.0").is_ok());
        for p in ["bin/r\u{e9}in", "bin/a\u{1}b", "bin/a\u{7f}b", "bin/a\tb"] {
            let err = check_path_alphabet(p).unwrap_err();
            assert!(err.contains(&format!("{p:?}")), "names the path: {err}");
        }
    }

    #[test]
    fn case_collisions_count_directory_prefixes_and_fold_ascii_only() {
        let ok = |v: &[&str]| check_case_collisions(v.iter().copied());
        assert!(ok(&["bin/rein", "shared/a.mjs", "shared/b.mjs", "bin"]).is_ok());
        assert!(
            ok(&["shared/x", "shared/x"]).is_ok(),
            "equal bytes are not a collision"
        );
        for (paths, named) in [
            (
                &["shared/b.mjs", "shared/B.mjs"][..],
                "'shared/b.mjs' and 'shared/B.mjs'",
            ),
            (
                &["shared/lib/x.mjs", "shared/Lib"],
                "'shared/lib' and 'shared/Lib'",
            ),
            (
                &["shared/Lib/a.mjs", "shared/lib/b.mjs"],
                "'shared/Lib' and 'shared/lib'",
            ),
            (&["Bin", "bin/rein"], "'Bin' and 'bin'"),
        ] {
            let err = ok(paths).unwrap_err();
            assert!(err.contains(named), "{paths:?}: {err}");
        }
        // One very long path: linear, not quadratic, in memory (security_engineer P3).
        let long = "a/".repeat(200_000) + "x";
        assert!(ok(&[&long]).is_ok());
    }

    #[test]
    fn accepts_plain_relative_paths() {
        for p in ["bin/rein", "hooks/pre-tool-use.sh", "a/b/c.d", "VERSION"] {
            assert!(check_rel_path(p).is_ok(), "{p}");
        }
    }

    #[test]
    fn rejects_every_escape_form() {
        for p in [
            "",
            "/etc/passwd",
            "../x",
            "bin/../../x",
            "bin/./rein",
            "bin//rein",
            "bin/",
            "bin\\rein",
            "C:/x",
            "bin/a\0b",
        ] {
            assert!(check_rel_path(p).is_err(), "{p:?}");
        }
    }
}
