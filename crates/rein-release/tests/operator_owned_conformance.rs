//! Shared contract fixtures from rein (E-17; blueprint §Test Plan "Operator-owned
//! conformance" and "Shared contract fixtures"; D-014). Vendored verbatim from rein commit
//! d84dd17 (`tests/fixtures/release/`); a row our code fails is reported to the Architect,
//! never edited here.
//!
//! - `operator-owned-cases.json`: every row (path, shipped, expected kept / replaced /
//!   deleted) is what an install does with that path. An installer writes and deletes only
//!   inside the mirrored directories (rein D-155 addendum 3 r10), so a path outside them is
//!   "kept" without consulting the matcher; inside, the answer is the sync plan's.
//! - `glob_cases`: each pattern/path fact against the matcher.
//! - `layout-v1.json`: equals the layout our fixture manifests carry, and is what the
//!   conformance rows are judged against.

mod support;

use rein_release::{Manifest, OperatorOwnedMatcher, plan};
use serde_json::Value;

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn every_case_row_is_what_an_install_does() {
    let layout = fixture(include_str!("support/layout-v1.json"));
    let (mirrored, patterns) = (
        strings(&layout["mirrored"]),
        strings(&layout["operator_owned"]),
    );
    let matcher = OperatorOwnedMatcher::new(&patterns).unwrap();
    let doc = fixture(include_str!("fixtures/operator-owned-cases.json"));
    let cases = doc["cases"].as_array().unwrap();
    assert!(cases.len() >= 20, "case table unexpectedly small");

    let inside = |p: &str| {
        mirrored
            .iter()
            .any(|m| p.split('/').next() == Some(m.as_str()))
    };
    let shipped = |r: &Value| match r["shipped"].as_str().unwrap() {
        "yes" => true,
        "no" => false,
        other => panic!("shipped '{other}'"),
    };
    // One plan over every row inside the mirror: each path is live; the shipped ones ship.
    let live: Vec<String> = cases
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_owned())
        .filter(|p| inside(p))
        .collect();
    let ship: Vec<&str> = cases
        .iter()
        .filter(|r| shipped(r))
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    let p = plan(&ship, &live, &matcher);
    let failed: Vec<String> = cases
        .iter()
        .filter_map(|row| {
            let path = row["path"].as_str().unwrap();
            let actual = if !inside(path) {
                "kept" // never written or deleted outside the mirror (rule 1)
            } else if p.ship.iter().any(|s| s == path) {
                "replaced"
            } else if p.prune.iter().chain(&p.prune_first).any(|s| s == path) {
                "deleted"
            } else {
                "kept"
            };
            let expected = row["expected"].as_str().unwrap();
            (actual != expected).then(|| format!("{path}: {actual}, rein says {expected}"))
        })
        .collect();
    assert!(
        failed.is_empty(),
        "rows our code fails:\n{}",
        failed.join("\n")
    );
}

#[test]
fn every_glob_case_is_what_the_matcher_answers() {
    let doc = fixture(include_str!("fixtures/operator-owned-cases.json"));
    let globs = doc["glob_cases"].as_array().unwrap();
    assert!(!globs.is_empty());
    for g in globs {
        let (pattern, path) = (g["pattern"].as_str().unwrap(), g["path"].as_str().unwrap());
        let m = OperatorOwnedMatcher::new(&[pattern]).unwrap();
        let owned = m.matches(path, false) || m.matches(path, true);
        assert_eq!(
            owned,
            g["match"].as_bool().unwrap(),
            "{pattern} ~ {path} ({})",
            g["label"]
        );
    }
}

#[test]
fn layout_v1_is_the_layout_our_fixture_manifests_carry() {
    let layout = fixture(include_str!("support/layout-v1.json"));
    let key = support::TestKey::generate();
    let manifest = support::manifest_json(&key.key_id(), &support::good_files());
    let m = Manifest::parse(manifest.as_bytes()).unwrap();
    assert_eq!(m.layout.mirrored, strings(&layout["mirrored"]));
    assert_eq!(m.layout.operator_owned, strings(&layout["operator_owned"]));
}
