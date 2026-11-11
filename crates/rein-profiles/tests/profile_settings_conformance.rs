//! rein's `profile-settings-cases.json` (E-17; D-014; rein D-155 r3), vendored verbatim from
//! rein commit d84dd17 (`tests/fixtures/release/`). Extends
//! `merge_preserves_keys.rs` (one rule, random documents) with rein's shared rows. A row
//! our code fails is reported to the Architect, never edited here.
//!
//! Each row is a settings document `before` and the document `after` a write would leave;
//! the table states what an app MAY write (D-030). The writer is `rein_profiles::plan` with
//! a `Patch`, which names allowed keys only; the patch tried is the strongest the row
//! allows: every allowed key whose value differs, set to its `after` value.
//! - `refuse`: unreachable by our writer, no exceptions.
//! - `accept`: reachable by `plan`, or listed in [`UNDO_ONLY`] with a reason. A listed row
//!   must be unreachable by `plan` (so the list cannot go stale) and is produced by a
//!   file-level `--undo` test ending in exactly the row's `after`.

use std::fs;

use rein_profiles::{AllowedKey, Patch, enumerate, plan, undo};
use serde_json::Value;

/// Accept rows rein-app reaches only through `profiles apply --undo` (D-030), with why.
const UNDO_ONLY: &[(&str, &str)] = &[(
    "remove an allowed key",
    "a Patch sets or merges allowed keys; only --undo removes one the backup lacked",
)];

fn table() -> Value {
    serde_json::from_str(include_str!("fixtures/profile-settings-cases.json")).unwrap()
}

fn produced(before: &Value, after: &Value) -> bool {
    let patch = AllowedKey::ALL
        .into_iter()
        .fold(Patch::new(), |patch, key| match after.get(key.json_key()) {
            Some(v) if before.get(key.json_key()) != Some(v) => patch.set(key, v.clone()),
            _ => patch,
        });
    matches!(plan(before, &patch), Ok((doc, _)) if &doc == after)
}

#[test]
fn every_profile_settings_row_is_what_the_writer_can_do() {
    let doc = table();
    let allowed: Vec<&str> = doc["allowed_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    let ours: Vec<&str> = AllowedKey::ALL.iter().map(|k| k.json_key()).collect();
    assert_eq!(allowed, ours, "the allow-list is rein's");

    let failed: Vec<String> = doc["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| {
            let label = row["label"].as_str().unwrap();
            let expected = row["expected"].as_str().unwrap();
            let can = produced(&row["before"], &row["after"]);
            let undo_only = UNDO_ONLY.iter().any(|(l, _)| *l == label);
            let ok = match expected {
                "accept" if undo_only => !can, // still unreachable by plan: the list is current
                "accept" => can,
                "refuse" => !undo_only && !can,
                other => panic!("{label}: expected '{other}'"),
            };
            (!ok).then(|| {
                format!(
                    "{label}: rein says {expected}, our writer {}",
                    if can { "can" } else { "cannot" }
                )
            })
        })
        .collect();
    assert!(
        failed.is_empty(),
        "rows our code fails:\n{}",
        failed.join("\n")
    );
}

/// D-030: each [`UNDO_ONLY`] row, produced through `--undo` at the file level: `before` is
/// the profile's settings.json, the pre-Rein backup is `before` without the allowed keys
/// `after` lacks, and undo leaves exactly `after`.
#[test]
fn every_undo_only_row_is_produced_by_undo() {
    let doc = table();
    for (label, _) in UNDO_ONLY {
        let row = doc["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["label"] == *label)
            .unwrap_or_else(|| panic!("UNDO_ONLY names '{label}', which rein's table lacks"));
        let (before, after) = (&row["before"], &row["after"]);
        let mut backup = before.as_object().unwrap().clone();
        for key in AllowedKey::ALL.map(AllowedKey::json_key) {
            if after.get(key).is_none() {
                backup.remove(key);
            }
        }
        let home = tempfile::tempdir().unwrap();
        let claude = home.path().join(".claude");
        fs::create_dir_all(&claude).unwrap();
        fs::write(claude.join("settings.json"), before.to_string()).unwrap();
        fs::write(
            claude.join("settings.json.rein-bak"),
            Value::Object(backup).to_string(),
        )
        .unwrap();
        let profile = enumerate(home.path()).profiles.remove(0);
        undo(&profile).unwrap();
        let written: Value =
            serde_json::from_slice(&fs::read(claude.join("settings.json")).unwrap()).unwrap();
        assert_eq!(&written, after, "{label}");
    }
}
