//! Relative-path rules shared by the manifest, the verifier and the layout writer.
//!
//! A release path is relative to the Rein home, uses `/`, and never escapes it
//! (rein D-153 r2). Anything else is rejected; nothing is normalised.

/// Check that `p` is a safe release path: non-empty, relative, `/`-separated, with no
/// empty, `.` or `..` segment, no backslash, no `:` (Windows drive/ADS) and no NUL.
pub fn check_rel_path(p: &str) -> Result<(), String> {
    if p.is_empty() {
        return Err("empty path".into());
    }
    if p.starts_with('/') {
        return Err(format!("absolute path '{p}'"));
    }
    if let Some(c) = p.chars().find(|c| matches!(c, '\\' | ':' | '\0')) {
        return Err(format!("forbidden character {c:?} in '{p}'"));
    }
    for seg in p.split('/') {
        match seg {
            "" => return Err(format!("empty segment in '{p}'")),
            "." | ".." => return Err(format!("'{seg}' segment in '{p}'")),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_rel_path;

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
