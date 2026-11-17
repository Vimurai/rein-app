//! The project tag (desktop.md §Look and feel v2 V12 component 1, D-061, E-69): the only
//! identity the app keeps for a session on its own tmux server. The SHA-256 of the tab's
//! canonical project path — on Windows the distro name, a NUL, then the Linux path — as 64
//! lower-case hex characters. The attach step sets it as the session option
//! `@rein_app_project`; the setup script's live lookup and the attach script find a session
//! by it. No session is ever named or looked up by name.

use std::path::Path;

use sha2::{Digest, Sha256};

/// The tag's length in hex characters.
pub const LEN: usize = 64;

/// The tag of `project` (`distro`: the WSL distro it lives in, Windows only).
pub fn tag(distro: Option<&str>, project: &Path) -> String {
    let mut h = Sha256::new();
    if let Some(d) = distro {
        h.update(d.as_bytes());
        h.update([0u8]);
    }
    h.update(project.to_string_lossy().as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Exactly 64 lower-case hex characters — the check each script repeats before use.
pub fn is_tag(s: &str) -> bool {
    s.len() == LEN && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_is_stable_64_hex_and_differs_by_path_and_distro() {
        let p = Path::new("/home/me/Dev/rein-app");
        let a = tag(None, p);
        assert!(is_tag(&a), "{a}");
        assert_eq!(a, tag(None, p), "stable");
        // The known SHA-256 of the path's bytes: the tag is the plain digest, no salt.
        assert_eq!(
            tag(None, Path::new("abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(a, tag(None, Path::new("/home/me/Dev/rein")));
        let u = tag(Some("Ubuntu"), p);
        assert!(is_tag(&u));
        assert_ne!(u, a, "a distro changes the tag");
        assert_ne!(u, tag(Some("Debian"), p));
        // The NUL separates the two parts: distro `Ub` + path `untu/x` is not `Ubuntu` + `/x`.
        assert_ne!(
            tag(Some("Ub"), Path::new("untu/x")),
            tag(Some("Ubuntu"), Path::new("/x"))
        );
    }

    #[test]
    fn only_64_lower_case_hex_is_a_tag() {
        assert!(is_tag(&"a".repeat(64)));
        for bad in [
            String::new(),
            "a".repeat(63),
            "a".repeat(65),
            "A".repeat(64),
            format!("{}g", "a".repeat(63)),
            format!("{} ", "a".repeat(63)),
            format!("{};", "a".repeat(63)),
        ] {
            assert!(!is_tag(&bad), "{bad:?}");
        }
    }
}
