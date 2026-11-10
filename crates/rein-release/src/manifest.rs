//! `manifest.json` schema 1 (rein D-153 r2, binding).
//!
//! ```json
//! { "schema": 1, "version": "X.Y.Z", "min_node": "22.5.0", "key_id": "<minisign key id>",
//!   "layout": { "mirrored": ["bin", ...], "operator_owned": ["accounts.json", ...] },
//!   "files": [ { "path": "bin/rein", "sha256": "<hex>", "size": 123, "mode": "0755" } ] }
//! ```

use std::collections::HashMap;

use semver::Version;
use serde::Deserialize;

use crate::error::VerifyError;
use crate::matcher::OperatorOwnedMatcher;
use crate::paths::{check_case_collisions, check_path_alphabet, check_rel_path};

/// The only manifest schema this build understands.
pub const SUPPORTED_SCHEMA: u64 = 1;
/// Upper bound on the manifest file itself.
pub const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
/// Upper bound on the sum of all listed file sizes.
pub const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

/// File permission the manifest may request. Anything else is rejected (fail closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    /// `"0644"`
    Regular,
    /// `"0755"`
    Executable,
}

impl FileMode {
    /// Unix permission bits.
    pub fn bits(self) -> u32 {
        match self {
            FileMode::Regular => 0o644,
            FileMode::Executable => 0o755,
        }
    }
}

/// One listed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    /// Lower-case hex.
    pub sha256: String,
    pub size: u64,
    pub mode: FileMode,
}

/// The `layout` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub mirrored: Vec<String>,
    pub operator_owned: Vec<String>,
}

/// A validated schema-1 manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    pub min_node: Version,
    /// Upper-case 16-hex-digit minisign key id.
    pub key_id: String,
    pub layout: Layout,
    pub files: Vec<FileEntry>,
    matcher: OperatorOwnedMatcher,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema: u64,
    version: String,
    min_node: String,
    key_id: String,
    layout: RawLayout,
    files: Vec<RawFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawLayout {
    pub(crate) mirrored: Vec<String>,
    pub(crate) operator_owned: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    path: String,
    sha256: String,
    size: u64,
    mode: String,
}

/// The three fields read before the signature is checked (steps a–b). Everything else is
/// validated only once the manifest is proven equal to the signed in-tarball copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestHeader {
    pub version: Version,
    /// Upper-case 16-hex-digit minisign key id.
    pub key_id: String,
}

impl ManifestHeader {
    /// Read `schema`, `version` and `key_id` only. Linear in the input; builds no matcher.
    pub fn peek(bytes: &[u8]) -> Result<Self, VerifyError> {
        let invalid = |m: String| VerifyError::ManifestInvalid(m);
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
        match value.get("schema").and_then(serde_json::Value::as_u64) {
            Some(SUPPORTED_SCHEMA) => {}
            Some(other) => return Err(VerifyError::UnsupportedSchema(other)),
            None => return Err(invalid("missing or non-integer 'schema'".into())),
        }
        let field = |name: &str| {
            value
                .get(name)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| invalid(format!("missing string '{name}'")))
        };
        let raw_version = field("version")?;
        let version = Version::parse(raw_version)
            .map_err(|e| invalid(format!("version '{raw_version}': {e}")))?;
        let raw_key = field("key_id")?;
        let key_id = normalise_key_id(raw_key)
            .ok_or_else(|| invalid(format!("key_id '{raw_key}' is not 16 hex digits")))?;
        Ok(Self { version, key_id })
    }
}

impl Manifest {
    /// Parse and validate. The schema number is checked before the shape, so a schema-2
    /// manifest reports "unsupported schema", not a parse error.
    pub fn parse(bytes: &[u8]) -> Result<Self, VerifyError> {
        let invalid = |m: String| VerifyError::ManifestInvalid(m);
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
        match value.get("schema").and_then(serde_json::Value::as_u64) {
            Some(SUPPORTED_SCHEMA) => {}
            Some(other) => return Err(VerifyError::UnsupportedSchema(other)),
            None => return Err(invalid("missing or non-integer 'schema'".into())),
        }
        let raw: RawManifest = serde_json::from_value(value).map_err(|e| invalid(e.to_string()))?;
        debug_assert_eq!(raw.schema, SUPPORTED_SCHEMA);

        let version = Version::parse(&raw.version)
            .map_err(|e| invalid(format!("version '{}': {e}", raw.version)))?;
        let min_node = Version::parse(&raw.min_node)
            .map_err(|e| invalid(format!("min_node '{}': {e}", raw.min_node)))?;
        let key_id = normalise_key_id(&raw.key_id)
            .ok_or_else(|| invalid(format!("key_id '{}' is not 16 hex digits", raw.key_id)))?;

        // Path alphabet and case rule (rein D-155 addendum 3, D-020): here, not in
        // `validate_layout`, so a snapshot's `layout.json` is not re-checked (D-020 r5).
        for d in &raw.layout.mirrored {
            check_path_alphabet(d).map_err(invalid)?;
        }
        let layout = validate_layout(raw.layout).map_err(invalid)?;
        let matcher = OperatorOwnedMatcher::new(&layout.operator_owned).map_err(invalid)?;

        let mut seen = HashMap::new();
        let mut total: u64 = 0;
        let mut files = Vec::with_capacity(raw.files.len());
        for f in raw.files {
            check_path_alphabet(&f.path).map_err(invalid)?;
            check_rel_path(&f.path).map_err(invalid)?;
            if f.path == "manifest.json" {
                return Err(invalid("'manifest.json' must not list itself".into()));
            }
            if seen.insert(f.path.clone(), ()).is_some() {
                return Err(invalid(format!("'{}' listed twice", f.path)));
            }
            let sha256 = f.sha256.to_ascii_lowercase();
            if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid(format!(
                    "'{}': sha256 is not 64 hex digits",
                    f.path
                )));
            }
            let mode = match f.mode.as_str() {
                "0644" => FileMode::Regular,
                "0755" => FileMode::Executable,
                other => {
                    return Err(invalid(format!(
                        "'{}': mode '{other}' is not 0644 or 0755",
                        f.path
                    )));
                }
            };
            total = total.saturating_add(f.size);
            files.push(FileEntry {
                path: f.path,
                sha256,
                size: f.size,
                mode,
            });
        }
        check_case_collisions(
            layout
                .mirrored
                .iter()
                .chain(files.iter().map(|f| &f.path))
                .map(String::as_str),
        )
        .map_err(invalid)?;
        if total > MAX_TOTAL_BYTES {
            return Err(invalid(format!(
                "listed files total {total} bytes (cap {MAX_TOTAL_BYTES})"
            )));
        }

        let manifest = Manifest {
            version,
            min_node,
            key_id,
            layout,
            files,
            matcher,
        };
        if let Some(f) = manifest
            .files
            .iter()
            .find(|f| !manifest.under_mirrored(&f.path))
        {
            return Err(VerifyError::OutsideLayout(f.path.clone()));
        }
        Ok(manifest)
    }

    /// Is `path` a file under a `layout.mirrored` directory? Nothing else may ship (rein
    /// D-155 r2): not a top-level file, not an operator-owned-only path.
    pub fn under_mirrored(&self, path: &str) -> bool {
        let mut segs = path.split('/');
        let first = segs.next().unwrap_or_default();
        self.layout.mirrored.iter().any(|m| m == first) && segs.next().is_some()
    }

    /// The operator-owned matcher built from `layout.operator_owned`.
    pub fn operator_owned(&self) -> &OperatorOwnedMatcher {
        &self.matcher
    }
}

pub(crate) fn validate_layout(raw: RawLayout) -> Result<Layout, String> {
    if raw.mirrored.is_empty() {
        return Err("layout.mirrored is empty".into());
    }
    for d in &raw.mirrored {
        check_rel_path(d)?;
        if d.contains('/') {
            return Err(format!(
                "layout.mirrored entry '{d}' is not a top-level directory"
            ));
        }
    }
    Ok(Layout {
        mirrored: raw.mirrored,
        operator_owned: raw.operator_owned,
    })
}

/// Upper-case a 16-hex-digit key id; `None` if it is not one.
pub fn normalise_key_id(s: &str) -> Option<String> {
    (s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit())).then(|| s.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(schema: u64, files: &str) -> String {
        format!(
            r#"{{"schema":{schema},"version":"0.5.0","min_node":"22.5.0","key_id":"0123456789abcdef",
            "layout":{{"mirrored":["bin","hooks"],"operator_owned":["accounts.json","secrets/"]}},
            "files":[{files}]}}"#
        )
    }
    const BIN: &str = r#"{"path":"bin/rein","sha256":"AB00000000000000000000000000000000000000000000000000000000000000","size":3,"mode":"0755"}"#;

    #[test]
    fn parses_a_valid_schema1_manifest() {
        let m = Manifest::parse(doc(1, BIN).as_bytes()).unwrap();
        assert_eq!(m.version, Version::new(0, 5, 0));
        assert_eq!(m.key_id, "0123456789ABCDEF");
        assert_eq!(m.files[0].mode, FileMode::Executable);
        assert!(m.files[0].sha256.starts_with("ab"));
        assert!(m.under_mirrored("bin/x"));
        assert!(
            !m.under_mirrored("secrets/x"),
            "owned-only paths cannot ship"
        );
        assert!(!m.under_mirrored("bin"));
        assert!(!m.under_mirrored("other/x"));
    }

    #[test]
    fn schema_is_checked_before_shape() {
        let err = Manifest::parse(br#"{"schema":2,"whatever":true}"#).unwrap_err();
        assert!(matches!(err, VerifyError::UnsupportedSchema(2)), "{err}");
    }

    #[test]
    fn rejects_bad_fields() {
        let cases = [
            BIN.replace("0755", "0777"),
            // Proves the parser calls check_rel_path (its own test covers every escape form).
            BIN.replace("bin/rein", "bin//rein"),
            BIN.replace("bin/rein", "manifest.json"),
            BIN.replace("AB00", "ZZ00"),
            format!("{BIN},{BIN}"),
        ];
        for files in cases {
            assert!(
                Manifest::parse(doc(1, &files).as_bytes()).is_err(),
                "{files}"
            );
        }
        let outside = BIN.replace("bin/rein", "evil/rein");
        // Path alphabet and case rule (D-020), extending the cases above: each names its path.
        let lib = |p: &str| BIN.replace("bin/rein", p);
        for (files, named) in [
            (lib(r"bin/r\u00e9in"), "bin/r\u{e9}in"),
            (lib(r"bin/a\u0001b"), r"bin/a\u{1}b"),
            (
                format!("{BIN},{}", lib("bin/Rein")),
                "'bin/rein' and 'bin/Rein'",
            ),
            (
                format!("{},{}", lib("hooks/lib/x.sh"), lib("hooks/Lib")),
                "'hooks/lib' and 'hooks/Lib'",
            ),
        ] {
            let err = Manifest::parse(doc(1, &files).as_bytes()).unwrap_err();
            assert!(
                matches!(&err, VerifyError::ManifestInvalid(m) if m.contains(named)),
                "{files}: {err}"
            );
        }
        let mirrored = doc(1, BIN).replace(r#"["bin","hooks"]"#, r#"["bin","hooks","Bin"]"#);
        let err = Manifest::parse(mirrored.as_bytes()).unwrap_err();
        assert!(err.to_string().contains("'bin' and 'Bin'"), "{err}");
        let mirrored = doc(1, BIN).replace(r#"["bin","hooks"]"#, r#"["bin","hooks","h\u0001"]"#);
        let err = Manifest::parse(mirrored.as_bytes()).unwrap_err();
        assert!(
            err.to_string().contains("printable ASCII"),
            "a mirrored name: {err}"
        );
        let distinct = format!("{BIN},{}", lib("hooks/rein"));
        assert!(
            Manifest::parse(doc(1, &distinct).as_bytes()).is_ok(),
            "distinct paths pass"
        );

        assert!(matches!(
            Manifest::parse(doc(1, &outside).as_bytes()),
            Err(VerifyError::OutsideLayout(_))
        ));
        let unknown = doc(1, BIN).replacen("\"schema\":1", "\"schema\":1,\"extra\":0", 1);
        assert!(Manifest::parse(unknown.as_bytes()).is_err());
    }
}
