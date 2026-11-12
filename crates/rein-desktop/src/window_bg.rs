//! The window's background colour at creation (desktop.md §Look and feel v2 V7, D-054): the
//! window is created in the theme's background, so it never flashes white before the page
//! paints. The page owns the themes (`ui/src/theme.ts`); this is only each built-in theme's
//! `bg`, and a test holds the two tables equal.

/// `(theme name, bg)` for the six built-in themes, as in `ui/src/theme.ts`.
pub const BUILT_IN: &[(&str, &str)] = &[
    ("gruvbox-dark", "#282828"),
    ("tokyonight", "#1a1b26"),
    ("catppuccin-mocha", "#1e1e2e"),
    ("nord", "#2e3440"),
    ("solarized-dark", "#002b36"),
    ("github-light", "#ffffff"),
];

/// The default theme's name (D-049 addendum 1), used for an unknown name as the page does.
const DEFAULT: &str = "catppuccin-mocha";

fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

/// The background for `theme` with config.toml's `[theme.custom] bg`, if it is a valid
/// `#rrggbb` (the page applies the same override).
pub fn background(theme: &str, custom_bg: Option<&str>) -> (u8, u8, u8) {
    if let Some(c) = custom_bg.and_then(rgb) {
        return c;
    }
    let pick = |name: &str| BUILT_IN.iter().find(|(n, _)| *n == name).map(|(_, bg)| *bg);
    pick(theme)
        .or_else(|| pick(DEFAULT))
        .and_then(rgb)
        .unwrap_or((0x1e, 0x1e, 0x2e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_pages_own() {
        // Each theme's `bg` follows its name within a few tokens in theme.ts, whether the theme
        // is an object literal (`name: "…", fg, bg`) or a `theme("…", fg, bg, …)` call.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/theme.ts");
        let src = std::fs::read_to_string(&path).expect("ui/src/theme.ts");
        let src = src.as_str();
        for (name, bg) in BUILT_IN {
            let at = src
                .find(&format!("\"{name}\""))
                .unwrap_or_else(|| panic!("{name} is not in theme.ts"));
            let near = &src[at..(at + 120).min(src.len())];
            assert!(
                near.contains(&format!("\"{bg}\"")),
                "{name}: theme.ts does not say bg {bg}"
            );
        }
        assert_eq!(BUILT_IN.len(), 6, "six built-in themes, as theme.ts");
        assert!(src.contains(&format!("DEFAULT_THEME_NAME = \"{DEFAULT}\"")));
    }

    #[test]
    fn custom_bg_wins_and_an_unknown_theme_falls_back() {
        assert_eq!(background("github-light", None), (255, 255, 255));
        assert_eq!(
            background("github-light", Some("#102030")),
            (0x10, 0x20, 0x30)
        );
        assert_eq!(
            background("github-light", Some("red")),
            (255, 255, 255),
            "not #rrggbb: ignored"
        );
        assert_eq!(background("no-such-theme", None), (0x1e, 0x1e, 0x2e));
    }
}
