//! The closed set of keys rein-app may write, and the pure merge (blueprint §Components 6,
//! §Security trust boundary 3; contract §2.3; rein D-132 hard-deny).
//!
//! `hooks`, `autoMode`, `permissions` and `defaultMode` are not representable: the only way
//! to name a key is [`AllowedKey`], which has no such variant. `hooks` belongs to `rein
//! install` alone (rein D-155 r3, D-014): the app never reads, copies, writes or removes it.
//!
//! ```compile_fail
//! // A forbidden key does not exist, so this cannot compile.
//! let _ = rein_profiles::AllowedKey::Permissions;
//! ```
//!
//! ```compile_fail
//! // Nor does `hooks`.
//! let _ = rein_profiles::AllowedKey::Hooks;
//! ```
//!
//! ```
//! // The three keys that do exist.
//! let _ = [
//!     rein_profiles::AllowedKey::EnabledPlugins,
//!     rein_profiles::AllowedKey::ExtraKnownMarketplaces,
//!     rein_profiles::AllowedKey::Attribution,
//! ];
//! ```

use serde_json::{Map, Value};

/// A settings key rein-app owns (contract §2.3). Closed: no other key can be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AllowedKey {
    /// Entries merged in; other plugins' entries kept.
    EnabledPlugins,
    /// Entries merged in; other marketplaces kept.
    ExtraKnownMarketplaces,
    /// Replaced wholesale.
    Attribution,
}

impl AllowedKey {
    pub const ALL: [AllowedKey; 3] = [
        AllowedKey::EnabledPlugins,
        AllowedKey::ExtraKnownMarketplaces,
        AllowedKey::Attribution,
    ];

    /// The JSON key in `settings.json`.
    pub fn json_key(self) -> &'static str {
        match self {
            AllowedKey::EnabledPlugins => "enabledPlugins",
            AllowedKey::ExtraKnownMarketplaces => "extraKnownMarketplaces",
            AllowedKey::Attribution => "attribution",
        }
    }

    /// Object keys merge entry by entry; the others are replaced.
    fn merges_entries(self) -> bool {
        matches!(
            self,
            AllowedKey::EnabledPlugins | AllowedKey::ExtraKnownMarketplaces
        )
    }
}

/// Desired values for some allowed keys. For merging keys the value must be an object
/// whose entries are set; for replaced keys it is the whole new value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Patch {
    changes: Vec<(AllowedKey, Value)>,
}

impl Patch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `key` to `value` (later calls for the same key win).
    pub fn set(mut self, key: AllowedKey, value: Value) -> Self {
        self.changes.retain(|(k, _)| *k != key);
        self.changes.push((key, value));
        self.changes.sort_by_key(|(k, _)| *k);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn changes(&self) -> &[(AllowedKey, Value)] {
        &self.changes
    }
}

/// Why a merge could not be planned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// `settings.json` is not a JSON object.
    NotAnObject,
    /// An existing value under a merging key is not an object, so entries cannot be merged.
    ExistingNotObject(&'static str),
    /// The patch value for a merging key is not an object.
    PatchNotObject(&'static str),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::NotAnObject => write!(f, "settings.json is not a JSON object"),
            PlanError::ExistingNotObject(k) => {
                write!(f, "existing '{k}' is not an object; refusing to merge")
            }
            PlanError::PatchNotObject(k) => write!(f, "patch value for '{k}' is not an object"),
        }
    }
}

impl std::error::Error for PlanError {}

/// One key whose value changes.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyChange {
    pub key: AllowedKey,
    pub before: Option<Value>,
    pub after: Value,
}

/// Merge `patch` into `settings`. Pure: returns the new document and what changed.
/// Every key that is not in the patch is carried over untouched, in its original position.
pub fn plan(settings: &Value, patch: &Patch) -> Result<(Value, Vec<KeyChange>), PlanError> {
    let Value::Object(original) = settings else {
        return Err(PlanError::NotAnObject);
    };
    let mut next: Map<String, Value> = original.clone();
    let mut changes = Vec::new();
    for (key, value) in patch.changes() {
        let name = key.json_key();
        let before = original.get(name).cloned();
        let after = if key.merges_entries() {
            let Value::Object(entries) = value else {
                return Err(PlanError::PatchNotObject(name));
            };
            let mut merged = match &before {
                None => Map::new(),
                Some(Value::Object(m)) => m.clone(),
                Some(_) => return Err(PlanError::ExistingNotObject(name)),
            };
            for (k, v) in entries {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        } else {
            value.clone()
        };
        if before.as_ref() != Some(&after) {
            next.insert(name.to_owned(), after.clone());
            changes.push(KeyChange {
                key: *key,
                before,
                after,
            });
        }
    }
    Ok((Value::Object(next), changes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merging_keys_keep_foreign_entries_and_replaced_keys_do_not() {
        let before = json!({
            "permissions": {"allow": ["x"]},
            "enabledPlugins": {"other@m": true},
            "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "mine"}]}]}
        });
        let patch = Patch::new()
            .set(
                AllowedKey::EnabledPlugins,
                json!({"rein-ux@rein-plugins": true}),
            )
            .set(AllowedKey::Attribution, json!({"commit": "x"}));
        let (after, changes) = plan(&before, &patch).unwrap();
        assert_eq!(after["permissions"], before["permissions"]);
        assert_eq!(
            after["hooks"], before["hooks"],
            "hooks belong to rein install"
        );
        assert_eq!(
            after["enabledPlugins"],
            json!({"other@m": true, "rein-ux@rein-plugins": true})
        );
        assert_eq!(after["attribution"], json!({"commit": "x"}));
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn unchanged_values_produce_no_change() {
        let before = json!({"enabledPlugins": {"a@m": true}});
        let patch = Patch::new().set(AllowedKey::EnabledPlugins, json!({"a@m": true}));
        let (after, changes) = plan(&before, &patch).unwrap();
        assert_eq!(after, before);
        assert!(changes.is_empty());
    }

    #[test]
    fn refuses_non_objects() {
        let p = Patch::new().set(AllowedKey::EnabledPlugins, json!({"a@m": true}));
        assert_eq!(plan(&json!([1]), &p).unwrap_err(), PlanError::NotAnObject);
        assert_eq!(
            plan(&json!({"enabledPlugins": "x"}), &p).unwrap_err(),
            PlanError::ExistingNotObject("enabledPlugins")
        );
        let bad = Patch::new().set(AllowedKey::ExtraKnownMarketplaces, json!(1));
        assert!(matches!(
            plan(&json!({}), &bad),
            Err(PlanError::PatchNotObject(_))
        ));
    }
}
