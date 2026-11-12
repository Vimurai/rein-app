//! The closed Action list (desktop.md §Data Model `Action`, D-033 ruling 2), generated from
//! `actions.json` — the same source the web side's `actions.ts` comes from. A key binding in
//! `config.toml` (E-37) may name only an action listed here.

/// What an action takes after its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionArg {
    None,
    /// `tab.goto`: 1–9.
    TabNumber,
    /// `theme.set`: a built-in theme or `custom`.
    ThemeName,
    /// `input.style`: `simple` | `modal`.
    InputStyle,
    /// `project.open`: a project path.
    Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    pub name: &'static str,
    pub arg: ActionArg,
}

include!(concat!(env!("OUT_DIR"), "/actions.rs"));

/// The action called `name`, if it is in the closed list.
pub fn find(name: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_the_blueprint_data_model() {
        assert_eq!(ACTIONS.len(), 28);
        assert_eq!(find("tab.goto").unwrap().arg, ActionArg::TabNumber);
        assert_eq!(find("theme.set").unwrap().arg, ActionArg::ThemeName);
        assert_eq!(find("mode.normal").unwrap().arg, ActionArg::None);
        assert!(find("shell.exec").is_none(), "no action can run a program");
    }

    #[test]
    fn the_web_side_is_generated_from_the_same_source() {
        let ts = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/actions.ts"),
        )
        .unwrap();
        for a in ACTIONS {
            assert!(
                ts.contains(&format!("name: {:?}", a.name)),
                "{} missing from actions.ts",
                a.name
            );
        }
        let entries = ts.lines().filter(|l| l.starts_with("  { name: ")).count();
        assert_eq!(entries, ACTIONS.len());
    }
}
