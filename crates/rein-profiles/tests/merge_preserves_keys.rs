//! Property: for random pre-existing settings, `plan` changes only allowed keys, and every
//! other key/value — including the hard-denied `autoMode`/`permissions`/`defaultMode` —
//! survives untouched; merged keys keep their foreign entries; the result round-trips.
//! (blueprint §Test Plan "Profile merge"). Seeded xorshift generator: deterministic,
//! no property-testing dependency; the seed is printed on failure.

use rein_profiles::{AllowedKey, Patch, plan, rein_plugins};
use serde_json::{Map, Number, Value, json};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const KEY_POOL: &[&str] = &[
    "autoMode",
    "permissions",
    "defaultMode",
    "hooks",
    "enabledPlugins",
    "extraKnownMarketplaces",
    "attribution",
    "model",
    "env",
    "statusLine",
    "ünïcode",
    "a\"b",
];

fn random_string(r: &mut Rng) -> String {
    let alphabet = ['a', 'Z', '0', ' ', '"', '\\', '\n', 'é', '中', '😀', '/'];
    (0..r.below(8))
        .map(|_| alphabet[r.below(alphabet.len() as u64) as usize])
        .collect()
}

fn random_value(r: &mut Rng, depth: u32) -> Value {
    match r.below(if depth > 3 { 5 } else { 7 }) {
        0 => Value::Null,
        1 => Value::Bool(r.below(2) == 0),
        2 => Value::Number(Number::from(r.next() as i64)),
        // Large integers and long floats: must survive byte-exact in value.
        3 => serde_json::from_str("123456789012345678901234567890.000000000000000000001").unwrap(),
        4 => Value::String(random_string(r)),
        5 => Value::Array(
            (0..r.below(4))
                .map(|_| random_value(r, depth + 1))
                .collect(),
        ),
        _ => Value::Object(random_object(r, depth + 1)),
    }
}

fn random_object(r: &mut Rng, depth: u32) -> Map<String, Value> {
    let mut m = Map::new();
    for _ in 0..r.below(6) {
        let key = if r.below(2) == 0 {
            KEY_POOL[r.below(KEY_POOL.len() as u64) as usize].to_owned()
        } else {
            random_string(r)
        };
        m.insert(key, random_value(r, depth));
    }
    m
}

#[test]
fn only_allowed_keys_change_and_everything_else_is_preserved() {
    let patch = Patch::new()
        .set(
            AllowedKey::ExtraKnownMarketplaces,
            rein_plugins::marketplace_entry(),
        )
        .set(
            AllowedKey::EnabledPlugins,
            json!({"rein-ux@rein-plugins": true}),
        )
        .set(AllowedKey::Attribution, json!({"commit": "", "pr": ""}));
    let allowed: Vec<&str> = AllowedKey::ALL.iter().map(|k| k.json_key()).collect();

    for seed in 1..=2000u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut before = random_object(&mut rng, 0);
        // Merging keys must hold objects for a merge to be possible; other shapes are a
        // refusal, tested in the unit tests.
        for k in ["enabledPlugins", "extraKnownMarketplaces"] {
            if before.get(k).is_some_and(|v| !v.is_object()) {
                before.insert(k.into(), Value::Object(random_object(&mut rng, 1)));
            }
        }
        let before = Value::Object(before);
        let (after, _) = plan(&before, &patch).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let (b, a) = (before.as_object().unwrap(), after.as_object().unwrap());

        for (k, v) in b {
            if !allowed.contains(&k.as_str()) {
                assert_eq!(a.get(k), Some(v), "seed {seed}: key {k:?} changed");
            }
        }
        for k in a.keys() {
            assert!(
                b.contains_key(k) || allowed.contains(&k.as_str()),
                "seed {seed}: new key {k:?}"
            );
        }
        for k in ["enabledPlugins", "extraKnownMarketplaces"] {
            if let Some(Value::Object(old)) = b.get(k) {
                let new = a[k].as_object().unwrap();
                for (child, v) in old {
                    let patched = patch
                        .changes()
                        .iter()
                        .any(|(pk, pv)| pk.json_key() == k && pv.get(child).is_some());
                    if !patched {
                        assert_eq!(new.get(child), Some(v), "seed {seed}: {k}.{child} lost");
                    }
                }
            }
        }
        let text = serde_json::to_string_pretty(&after).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap(),
            after,
            "seed {seed}: round trip"
        );
    }
}
