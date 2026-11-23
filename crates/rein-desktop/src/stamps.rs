//! The review stamps of a project (desk.md §Components 1 T2 and §Data Model `Stamp`, D-074;
//! E-106): `state.json`'s `stamps` list, the newest [`MAX_STAMPS`] kept. Same rules as the
//! task parse (`panel_sources`): untrusted text, every field cut and cleaned, a shape this
//! version does not read is skipped — a stamp that is not an object, or has no `type` or
//! `task_id`, is not a stamp. A list longer than [`MAX_STAMPS_READ`] gives no stamps at all.

use std::collections::VecDeque;
use std::fmt;

use serde::de::{Deserializer, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

use crate::panel_sources::{Lenient, SHORT, shown};

/// Stamps kept: the last ones in the file (they are appended).
pub const MAX_STAMPS: usize = 300;
/// More stamps than this in the file → none are given (bounded work).
pub const MAX_STAMPS_READ: usize = 50_000;
/// Characters kept of a stamp's summary (2 KiB).
pub const STAMP_SUMMARY: usize = 2048;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Stamp {
    /// E.g. `ACCEPT_FAIL`.
    #[serde(rename = "type")]
    pub kind: String,
    pub task_id: String,
    pub timestamp: String,
    pub sha: String,
    pub summary: String,
}

#[derive(Deserialize)]
struct RawStamp {
    #[serde(rename = "type", default)]
    kind: Lenient,
    #[serde(default)]
    task_id: Lenient,
    #[serde(default)]
    timestamp: Lenient,
    #[serde(default)]
    sha: Lenient,
    #[serde(default)]
    summary: Lenient,
}

/// One element: a stamp, or anything else (skipped without storing it).
enum Element {
    Stamp(RawStamp),
    Other,
}

impl<'de> Deserialize<'de> for Element {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Buffer through serde_json's Value only for objects small enough to matter: an
        // element is read as a value once, then taken apart; a non-object is `Other`.
        let v = serde_json::Value::deserialize(d)?;
        Ok(match v {
            serde_json::Value::Object(_) => match serde_json::from_value::<RawStamp>(v) {
                Ok(r) => Element::Stamp(r),
                Err(_) => Element::Other,
            },
            _ => Element::Other,
        })
    }
}

/// The list, read element by element, keeping only the last [`MAX_STAMPS`].
struct Ring(Option<VecDeque<RawStamp>>);

impl<'de> Deserialize<'de> for Ring {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ring;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list of stamps")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Ring, A::Error> {
                let mut out = VecDeque::with_capacity(MAX_STAMPS);
                let mut n = 0usize;
                while let Some(e) = seq.next_element::<Element>()? {
                    n += 1;
                    if n > MAX_STAMPS_READ {
                        while seq.next_element::<IgnoredAny>()?.is_some() {}
                        return Ok(Ring(None));
                    }
                    if let Element::Stamp(s) = e {
                        if out.len() == MAX_STAMPS {
                            out.pop_front();
                        }
                        out.push_back(s);
                    }
                }
                Ok(Ring(Some(out)))
            }
            // Anything but a list is no stamps.
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Ring, A::Error> {
                while m.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Ring(None))
            }
            fn visit_str<E>(self, _: &str) -> Result<Ring, E> {
                Ok(Ring(None))
            }
            fn visit_i64<E>(self, _: i64) -> Result<Ring, E> {
                Ok(Ring(None))
            }
            fn visit_u64<E>(self, _: u64) -> Result<Ring, E> {
                Ok(Ring(None))
            }
            fn visit_f64<E>(self, _: f64) -> Result<Ring, E> {
                Ok(Ring(None))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Ring, E> {
                Ok(Ring(None))
            }
            fn visit_unit<E>(self) -> Result<Ring, E> {
                Ok(Ring(None))
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Deserialize)]
struct RawState {
    #[serde(default)]
    stamps: Option<Ring>,
}

/// The stamps of a `state.json`, oldest first (as in the file). Anything this version does not
/// read gives an empty list, never an error: the stamps are extra facts, the tasks stand alone.
pub fn parse_stamps(bytes: &[u8]) -> Vec<Stamp> {
    let Ok(raw) = serde_json::from_slice::<RawState>(bytes) else {
        return Vec::new();
    };
    let Some(Ring(Some(list))) = raw.stamps else {
        return Vec::new();
    };
    list.into_iter()
        .filter_map(|r| {
            let (Some(kind), Some(task_id)) = (r.kind.0, r.task_id.0) else {
                return None;
            };
            let s = |v: Option<String>| shown(v.as_deref().unwrap_or(""), SHORT, false);
            Some(Stamp {
                kind: shown(&kind, SHORT, false),
                task_id: shown(&task_id, SHORT, false),
                timestamp: s(r.timestamp.0),
                sha: s(r.sha.0),
                summary: shown(r.summary.0.as_deref().unwrap_or(""), STAMP_SUMMARY, false),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_repos_own_stamps_are_read() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let bytes = std::fs::read(root.join(".ai/state.json")).unwrap();
        let s = parse_stamps(&bytes);
        assert!(!s.is_empty() && s.len() <= MAX_STAMPS, "{}", s.len());
        assert!(
            s.iter()
                .all(|x| !x.kind.is_empty() && !x.task_id.is_empty())
        );
    }

    #[test]
    fn a_hostile_stamps_array_gives_bounded_clean_stamps() {
        let long = "y".repeat(10_000);
        let one = format!(
            r#"{{"type":"ACCEPT_FAIL\u001b[31m","task_id":"E-1\u202e","timestamp":"2026-11-23T10:00:00Z","summary":"{long}","sha":"abc"}}"#
        );
        let mut items = vec![
            "1".to_string(),
            "null".into(),
            "[1,2]".into(),
            r#"{"type":"X"}"#.into(),
            r#"{"task_id":"E-9"}"#.into(),
            r#"{"type":"A","task_id":"E-2","sha":{"x":1}}"#.into(),
        ];
        items.extend(std::iter::repeat_n(one, 400));
        let bytes = format!(r#"{{"tasks":[],"stamps":[{}]}}"#, items.join(","));
        let s = parse_stamps(bytes.as_bytes());
        assert_eq!(s.len(), MAX_STAMPS, "the last 300 stamps, the rest skipped");
        let few = format!(r#"{{"stamps":[{},{}]}}"#, items[..6].join(","), items[6]);
        let f = parse_stamps(few.as_bytes());
        assert_eq!(f.len(), 1, "only the whole stamp is a stamp: {f:?}");
        let x = &s[0];
        assert!(x.kind.starts_with("ACCEPT_FAIL␛"), "{}", x.kind);
        assert!(x.task_id.contains("⟨U+202E⟩"), "{}", x.task_id);
        assert!(x.summary.chars().count() <= STAMP_SUMMARY + 1);
        assert!(x.summary.ends_with('…'));
    }

    #[test]
    fn a_stamps_field_of_another_shape_gives_none() {
        for bad in [
            r#"{"stamps":{"a":1}}"#,
            r#"{"stamps":"x"}"#,
            r#"{"stamps":7}"#,
            r#"{"stamps":null}"#,
            r#"{"tasks":[]}"#,
            "not json",
        ] {
            assert!(parse_stamps(bad.as_bytes()).is_empty(), "{bad}");
        }
    }

    #[test]
    fn too_many_stamps_give_none() {
        let bytes = format!(
            r#"{{"stamps":[{}]}}"#,
            vec!["0"; MAX_STAMPS_READ + 1].join(",")
        );
        assert!(parse_stamps(bytes.as_bytes()).is_empty());
    }
}
