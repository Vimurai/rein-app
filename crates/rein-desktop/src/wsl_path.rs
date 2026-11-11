//! Windows paths → `ProjectPath` (desktop.md §Windows window W3, D-038). Pure: no process,
//! no filesystem. On Windows a project path is the **Linux** absolute path inside the
//! window's distro.
//!
//! Accepted: a POSIX absolute path (as is), `\\wsl.localhost\<distro>\…` and
//! `\\wsl$\<distro>\…` (mapped to `/…`, the distro must be the window's). Refused: anything
//! else (a drive path, a relative path, another UNC host), and — because `wsl.exe` re-parses
//! its command line and we never escape — a result containing `"`, `\`, a control or a bidi
//! character.

use crate::project_path;

/// Why a path cannot be a project on Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// Not a WSL path: a drive path, a relative path or another UNC host.
    NotWsl { distro: String },
    /// A `\\wsl.localhost\` path into a different distro than the window's.
    OtherDistro { given: String, ours: String },
    /// Fails the text rule, or holds a character `wsl.exe` would re-parse.
    Unsafe(&'static str),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotWsl { distro } => write!(
                f,
                "Rein projects live inside WSL — choose a folder under \\\\wsl.localhost\\{distro}"
            ),
            Self::OtherDistro { given, ours } => write!(
                f,
                "this project is in the WSL distro {given}; the window uses {ours}"
            ),
            Self::Unsafe(why) => f.write_str(why),
        }
    }
}

const UNC_HOSTS: [&str; 2] = ["wsl.localhost", "wsl$"];

/// Map `input` to the Linux path inside `distro`.
pub fn to_linux(input: &str, distro: &str) -> Result<String, PathError> {
    let text = project_path::check_text(input).map_err(PathError::Unsafe)?;
    let linux = if text.starts_with('/') {
        text.to_string()
    } else if let Some(unc) = text.strip_prefix("\\\\") {
        from_unc(unc, distro)?
    } else {
        return Err(PathError::NotWsl {
            distro: distro.to_string(),
        });
    };
    check_linux(&linux)?;
    Ok(linux)
}

/// `<host>\<distro>\rest…` after the leading `\\`.
fn from_unc(unc: &str, distro: &str) -> Result<String, PathError> {
    let mut parts = unc.splitn(3, '\\');
    let host = parts.next().unwrap_or_default();
    if !UNC_HOSTS.iter().any(|h| h.eq_ignore_ascii_case(host)) {
        return Err(PathError::NotWsl {
            distro: distro.to_string(),
        });
    }
    let given = parts.next().unwrap_or_default();
    if given.is_empty() {
        return Err(PathError::NotWsl {
            distro: distro.to_string(),
        });
    }
    if !given.eq_ignore_ascii_case(distro) {
        return Err(PathError::OtherDistro {
            given: given.to_string(),
            ours: distro.to_string(),
        });
    }
    let rest = parts.next().unwrap_or_default().trim_end_matches('\\');
    Ok(format!("/{}", rest.replace('\\', "/")))
}

/// The rule for a Linux path that will be placed in a `wsl.exe` argv: the text rule, POSIX
/// absolute, and no `"` or `\` (we refuse instead of escaping).
pub fn check_linux(path: &str) -> Result<(), PathError> {
    project_path::check(path).map_err(PathError::Unsafe)?;
    if path.contains(['"', '\\']) {
        return Err(PathError::Unsafe(
            "path contains a \" or \\, which wsl.exe would re-parse",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_table() {
        let ok = |i: &str, want: &str| assert_eq!(to_linux(i, "Ubuntu").unwrap(), want, "{i}");
        ok("/home/me/proj", "/home/me/proj");
        ok("\\\\wsl.localhost\\Ubuntu\\home\\me\\proj", "/home/me/proj");
        ok("\\\\wsl$\\Ubuntu\\home\\me\\proj", "/home/me/proj");
        ok(
            "\\\\WSL.LOCALHOST\\ubuntu\\home\\me\\proj\\",
            "/home/me/proj",
        );
        ok("\\\\wsl.localhost\\Ubuntu", "/");

        let not_wsl = PathError::NotWsl {
            distro: "Ubuntu".into(),
        };
        for bad in [
            "C:\\Users\\me\\proj",
            "C:/Users/me",
            "proj",
            "\\\\server\\share\\x",
            "\\\\wsl.localhost\\",
        ] {
            assert_eq!(to_linux(bad, "Ubuntu"), Err(not_wsl.clone()), "{bad}");
        }
        assert_eq!(
            to_linux("\\\\wsl.localhost\\Debian\\home\\me", "Ubuntu"),
            Err(PathError::OtherDistro {
                given: "Debian".into(),
                ours: "Ubuntu".into()
            })
        );
        for bad in [
            "/home/me/a\"b",
            "/home/me/a\\b",
            "/home/me/a\u{1b}[2J",
            "/home/me/a\u{202e}b",
            "",
        ] {
            assert!(
                matches!(to_linux(bad, "Ubuntu"), Err(PathError::Unsafe(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_refusal_names_the_distro_folder() {
        let e = to_linux("C:\\x", "Ubuntu").unwrap_err().to_string();
        assert_eq!(
            e,
            "Rein projects live inside WSL — choose a folder under \\\\wsl.localhost\\Ubuntu"
        );
    }
}
