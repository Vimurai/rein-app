//! Operator-owned matcher conformance (blueprint §Test Plan): every row of the vendored
//! case table yields `expected`. The table is rein-app's interim copy until rein publishes
//! `tests/fixtures/release/operator-owned-cases.json`; see its `_source` field.
//! Replaces the hand-written yes/no lists formerly in matcher.rs unit tests.

use rein_release::OperatorOwnedMatcher;
use serde_json::Value;

#[test]
fn every_case_row_matches_its_expected_value() {
    let doc: Value =
        serde_json::from_str(include_str!("fixtures/operator-owned-cases.json")).unwrap();
    let patterns: Vec<&str> = doc["operator_owned"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    let matcher = OperatorOwnedMatcher::new(&patterns).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(cases.len() >= 20, "case table unexpectedly small");
    for row in cases {
        let path = row["path"].as_str().unwrap();
        let expected = row["expected"].as_bool().unwrap();
        assert_eq!(matcher.matches(path, false), expected, "{path}");
    }
}
