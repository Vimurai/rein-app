//! The `rein-plugins` marketplace registration (D-002 and its addendum).

use serde_json::{Value, json};

/// Marketplace name == the `name` in rein-plugins' `.claude-plugin/marketplace.json`.
pub const MARKETPLACE: &str = "rein-plugins";
/// GitHub repository of the marketplace (D-002 addendum: owner confirmed).
pub const REPO: &str = "Vimurai/rein-plugins";
/// Moving release branch rein-plugins maintains (D-002: never rewritten per release).
pub const REF: &str = "stable";
/// The plugins `--enable all` selects (D-002).
pub const KNOWN_PLUGINS: [&str; 4] = ["rein-seo", "rein-ux", "rein-ops", "rein-data"];

/// The `extraKnownMarketplaces` entries to merge.
pub fn marketplace_entry() -> Value {
    json!({ MARKETPLACE: { "source": { "source": "github", "repo": REPO, "ref": REF } } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_matches_d002_literally() {
        assert_eq!(
            marketplace_entry(),
            serde_json::from_str::<Value>(
                r#"{"rein-plugins":{"source":{"source":"github","repo":"Vimurai/rein-plugins","ref":"stable"}}}"#
            )
            .unwrap()
        );
    }
}
