//! Operator-owned conformance (blueprint §Test Plan "Operator-owned conformance"; rein
//! D-155): every row of the vendored case table — path, shipped yes/no, expected `kept` /
//! `replaced` / `deleted` — is what the LayoutWriter's sync plan does with that path. The
//! table is rein-app's interim copy in rein's three-column shape until rein publishes
//! `tests/fixtures/release/operator-owned-cases.json` (E-17); see its `_source` field.

use rein_release::{OperatorOwnedMatcher, plan};
use serde_json::Value;

#[test]
fn every_case_row_is_what_the_sync_plan_does() {
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
    assert!(
        cases.iter().any(|r| r["shipped"] == true
            && r["expected"] == "replaced"
            && matcher.matches(r["path"].as_str().unwrap(), false)),
        "the table carries the precedence case (shipped and owned: replaced)"
    );

    // One plan over the whole table: every row's path is live; the shipped ones ship.
    let live: Vec<String> = cases
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_owned())
        .collect();
    let shipped: Vec<&str> = cases
        .iter()
        .filter(|r| r["shipped"].as_bool().unwrap())
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    let p = plan(&shipped, &live, &matcher);
    for row in cases {
        let path = row["path"].as_str().unwrap();
        let actual = if p.ship.iter().any(|s| s == path) {
            "replaced"
        } else if p.prune.iter().any(|s| s == path) {
            "deleted"
        } else {
            "kept"
        };
        assert_eq!(actual, row["expected"].as_str().unwrap(), "{path}");
    }
}
