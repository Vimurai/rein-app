//! The strict reader of `rein doctor --json` (rein's schema 1, `docs/contract/doctor.schema.json`;
//! D-029, D-032). Moved here from the `rein-app` CLI (D-045) so the CLI's install verdict and
//! the window's health view parse with one implementation: exact keys, no duplicate key, no
//! unknown status or scope, and every string from rein escaped before display.

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Install,
    Host,
    Project,
}

#[derive(Debug)]
pub struct Check {
    pub id: String,
    pub scope: Scope,
    pub subject: Option<String>,
    pub status: Status,
    pub line: String,
    /// What to do about it, if rein says (shown by the window's health view).
    pub fix: Option<String>,
}

#[derive(Debug)]
pub struct Document {
    pub version: String,
    pub status: Status,
    pub checks: Vec<Check>,
}

/// An object with exactly `keys` (schema `required` + `additionalProperties: false`).
fn exact<'a>(v: &'a Value, keys: &[&str], what: &str) -> Result<&'a Map<String, Value>, String> {
    let obj = v.as_object().ok_or(format!("{what} is not an object"))?;
    if let Some(k) = keys.iter().find(|k| !obj.contains_key(**k)) {
        return Err(format!("{what} lacks `{k}`"));
    }
    if let Some(k) = obj.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(format!("{what} has an unknown key `{}`", escape(k)));
    }
    Ok(obj)
}

fn status(v: &Value) -> Result<Status, String> {
    match v.as_str() {
        Some("ok") => Ok(Status::Ok),
        Some("warn") => Ok(Status::Warn),
        Some("fail") => Ok(Status::Fail),
        _ => Err("a status that is not ok, warn or fail".into()),
    }
}

fn nullable_string(v: &Value, what: &str) -> Result<Option<String>, String> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(format!("{what} is neither a string nor null")),
    }
}

/// Refuse a JSON text that repeats a key within one object: serde keeps the last value, so
/// `"status":"fail", …, "status":"ok"` would otherwise read as ok (E-32 security review
/// P2-1). One linear pass over text that `serde_json` has already accepted as JSON.
fn no_duplicate_keys(text: &[u8]) -> Result<(), String> {
    enum Frame {
        Object {
            expect_key: bool,
            keys: std::collections::HashSet<String>,
        },
        Array,
    }
    let mut stack: Vec<Frame> = Vec::new();
    let mut i = 0;
    while i < text.len() {
        match text[i] {
            b'{' => stack.push(Frame::Object {
                expect_key: true,
                keys: Default::default(),
            }),
            b'[' => stack.push(Frame::Array),
            b'}' | b']' => {
                stack.pop();
            }
            b',' => {
                if let Some(Frame::Object { expect_key, .. }) = stack.last_mut() {
                    *expect_key = true;
                }
            }
            b':' => {
                if let Some(Frame::Object { expect_key, .. }) = stack.last_mut() {
                    *expect_key = false;
                }
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < text.len() && text[i] != b'"' {
                    i += if text[i] == b'\\' { 2 } else { 1 };
                }
                if let Some(Frame::Object {
                    expect_key: true,
                    keys,
                }) = stack.last_mut()
                {
                    let raw = std::str::from_utf8(&text[start..=i]).map_err(|e| e.to_string())?;
                    let key: String = serde_json::from_str(raw).map_err(|e| e.to_string())?;
                    if !keys.insert(key.clone()) {
                        return Err(format!(
                            "the key `{}` appears twice in one object",
                            escape(&key)
                        ));
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

/// Parse a doctor document strictly against schema 1; `Err` names what is wrong.
pub fn parse(stdout: &[u8]) -> Result<Document, String> {
    let value: Value = serde_json::from_slice(stdout).map_err(|e| format!("not JSON ({e})"))?;
    no_duplicate_keys(stdout)?;
    let top = exact(
        &value,
        &["schema", "version", "status", "checks"],
        "the document",
    )?;
    if top["schema"] != 1 {
        return Err("schema is not 1 (this rein-app reads 1)".into());
    }
    let version = top["version"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or("`version` is not a non-empty string")?
        .to_owned();
    let raw = top["checks"].as_array().ok_or("`checks` is not an array")?;
    let mut checks = Vec::with_capacity(raw.len());
    for (i, c) in raw.iter().enumerate() {
        let what = format!("check {i}");
        let o = exact(
            c,
            &["id", "scope", "subject", "status", "line", "fix"],
            &what,
        )?;
        let id = o["id"].as_str().unwrap_or_default();
        let id_ok = id.starts_with(|ch: char| ch.is_ascii_lowercase())
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !id_ok {
            return Err(format!("{what} has an invalid id"));
        }
        let scope = match o["scope"].as_str() {
            Some("install") => Scope::Install,
            Some("host") => Scope::Host,
            Some("project") => Scope::Project,
            _ => return Err(format!("{what} has an unknown scope")),
        };
        let line = o["line"]
            .as_str()
            .filter(|l| !l.is_empty())
            .ok_or(format!("{what} has no line"))?
            .to_owned();
        let fix = nullable_string(&o["fix"], &what)?;
        checks.push(Check {
            id: id.to_owned(),
            scope,
            subject: nullable_string(&o["subject"], &what)?,
            status: status(&o["status"]).map_err(|e| format!("{what}: {e}"))?,
            line,
            fix,
        });
    }
    Ok(Document {
        version,
        status: status(&top["status"])?,
        checks,
    })
}

/// Control characters, bidi marks, overrides and isolates, zero-width characters and line
/// separators escaped; text from rein reaches the terminal only this way.
pub fn escape(s: &str) -> String {
    let unsafe_char = |c: char| {
        c.is_control()
            || matches!(
                c,
                '\u{00ad}'
                    | '\u{034f}'
                    | '\u{061c}'
                    | '\u{115f}'..='\u{1160}'
                    | '\u{17b4}'..='\u{17b5}'
                    | '\u{180b}'..='\u{180f}'
                    | '\u{200b}'..='\u{200f}'
                    | '\u{2028}'..='\u{202e}'
                    | '\u{2060}'..='\u{206f}'
                    | '\u{3164}'
                    | '\u{fe00}'..='\u{fe0f}'
                    | '\u{feff}'
                    | '\u{ffa0}'
                    | '\u{fff9}'..='\u{fffb}'
                    | '\u{e0000}'..='\u{e007f}'
                    | '\u{e0100}'..='\u{e01ef}'
            )
    };
    s.chars()
        .map(|c| {
            if c == '\\' {
                "\\\\".to_string() // so a literal `\u{1b}` never reads as an escape (D-032)
            } else if unsafe_char(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// One printed row per check: status, scope, id[/subject], then each line of `line`.
pub fn check_lines(doc: &Document) -> Vec<String> {
    doc.checks
        .iter()
        .map(|c| {
            let status = match c.status {
                Status::Ok => "ok  ",
                Status::Warn => "warn",
                Status::Fail => "FAIL",
            };
            let scope = match c.scope {
                Scope::Install => "install",
                Scope::Host => "host",
                Scope::Project => "project",
            };
            let name = match &c.subject {
                Some(s) => format!("{}/{}", escape(&c.id), escape(s)),
                None => escape(&c.id),
            };
            let line: Vec<String> = c.line.split('\n').map(escape).collect();
            format!("  {status} [{scope}] {name}: {}", line.join(" / "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_keeps_its_fix_and_the_document_its_status() {
        let text = r#"{"schema":1,"version":"0.5.0","status":"warn","checks":[
            {"id":"node","scope":"host","subject":null,"status":"warn","line":"old node","fix":"install node 22"},
            {"id":"tree","scope":"install","subject":"bin","status":"ok","line":"ok","fix":null}]}"#;
        let d = parse(text.as_bytes()).unwrap();
        assert_eq!(d.status, Status::Warn);
        assert_eq!(d.checks[0].fix.as_deref(), Some("install node 22"));
        assert_eq!(d.checks[1].fix, None);
        assert_eq!(d.checks[1].subject.as_deref(), Some("bin"));
    }

    #[test]
    fn duplicate_keys_unknown_keys_and_other_schemas_are_refused() {
        let ok = r#"{"schema":1,"version":"1","status":"ok","checks":[]}"#;
        assert!(parse(ok.as_bytes()).is_ok());
        for bad in [
            r#"{"schema":1,"version":"1","status":"fail","checks":[],"status":"ok"}"#,
            r#"{"schema":1,"version":"1","status":"ok","checks":[],"extra":1}"#,
            r#"{"schema":2,"version":"1","status":"ok","checks":[]}"#,
            r#"{"schema":1,"version":"1","status":"pass","checks":[]}"#,
            "not json",
        ] {
            assert!(parse(bad.as_bytes()).is_err(), "{bad}");
        }
        assert_eq!(escape("a\u{1b}[2J\\b"), "a\\u{1b}[2J\\\\b");
    }
}
