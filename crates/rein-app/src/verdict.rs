//! The install verdict (blueprint §Components 4 "Install verdict"; D-029, rein D-158).
//!
//! After every sync (`install`, `update`, `rollback`) rein-app runs `rein install`, then
//! `rein doctor --scope install --json` from the installed tree. The verdict is the
//! document's top-level `status` and nothing else: `fail` → 3, `warn` or `ok` → 0. A
//! command that cannot start, ends without a code or with one other than 0/1, or prints
//! anything but a schema-1 document (`docs/contract/doctor.schema.json`, vendored from rein
//! cac86a4) → 14, never a pass. Checks are printed (id, scope, status, line), escaped; no
//! check id is ever filtered or branched on.

use std::path::Path;

use serde_json::{Map, Value};

use crate::install_cmd::{EXIT_DOCTOR, exit_for};
use crate::runner::{Process, ReinCommand, RunOutcome, rein_invocation, run_rein};
use crate::update_cmd::EXIT_DOCTOR_NOT_RUN;

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
        nullable_string(&o["fix"], &what)?;
        checks.push(Check {
            id: id.to_owned(),
            scope,
            subject: nullable_string(&o["subject"], &what)?,
            status: status(&o["status"]).map_err(|e| format!("{what}: {e}"))?,
            line,
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
fn escape(s: &str) -> String {
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

/// A document that contradicts its own exit code is no document (E-32 security review
/// P2-2): exit 1 says a check failed, so the status must be `fail`. (Exit 0 with `fail` is
/// judged 3.) Otherwise the status alone decides (D-029): a failing host check inside an
/// `ok` document does not.
fn consistent(code: Option<i32>, doc: Document) -> Result<Document, String> {
    if code == Some(1) && doc.status != Status::Fail {
        return Err("rein doctor exited 1 but its status is not fail".into());
    }
    Ok(doc)
}

/// The verdict for a doctor run that ended with `code` and printed `stdout`.
pub fn judge(code: Option<i32>, stdout: &[u8]) -> u8 {
    let kept = "the previous version, if any, is kept as $HOME/.rein.prev-* (`rein-app rollback` restores it)";
    match code {
        Some(0 | 1) => {}
        Some(c) => {
            eprintln!(
                "rein-app: rein doctor --scope install --json exited {c}: no document; not judged a pass"
            );
            return EXIT_DOCTOR_NOT_RUN;
        }
        None => {
            eprintln!("rein-app: rein doctor ended without an exit code (killed by a signal?)");
            return EXIT_DOCTOR_NOT_RUN;
        }
    }
    let doc = match parse(stdout).and_then(|d| consistent(code, d)) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "rein-app: rein doctor printed no valid install document ({}); not judged a pass",
                escape(&e)
            );
            return EXIT_DOCTOR_NOT_RUN;
        }
    };
    println!("rein doctor --scope install ({}):", escape(&doc.version));
    for line in check_lines(&doc) {
        println!("{line}");
    }
    match doc.status {
        Status::Fail => {
            eprintln!("rein-app: the install check failed; {kept}");
            EXIT_DOCTOR
        }
        Status::Warn => {
            println!("rein-app: installed, with warnings above");
            0
        }
        Status::Ok => 0,
    }
}

/// The wall-clock limit on the install check (D-032). `rein install` has none.
pub const INSTALL_CHECK_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

/// `rein install`, then the install verdict, from the installed tree under `home`.
pub fn install_then_verdict(proc: &dyn Process, home: &Path) -> u8 {
    match run_rein(proc, home, &[ReinCommand::Install]) {
        RunOutcome::Ok => {}
        other => return exit_for(other),
    }
    let inv = rein_invocation(
        home,
        ReinCommand::DoctorInstallJson,
        std::env::var_os("PATH"),
    );
    match proc.output(&inv, INSTALL_CHECK_LIMIT) {
        Ok(Some((code, stdout))) => judge(code, &stdout),
        Ok(None) => {
            eprintln!(
                "rein-app: the install check (rein doctor --scope install) did not finish within {} seconds; it was stopped and the install lock is released",
                INSTALL_CHECK_LIMIT.as_secs()
            );
            EXIT_DOCTOR_NOT_RUN
        }
        Err(e) => {
            eprintln!(
                "rein-app: could not start rein doctor ({}); not judged a pass",
                escape(&e.to_string())
            );
            EXIT_DOCTOR_NOT_RUN
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A schema-1 document with `status` and one check per `(scope, status)`.
    pub(crate) fn doc(status: &str, checks: &[(&str, &str)]) -> String {
        let rows: Vec<serde_json::Value> = checks
            .iter()
            .enumerate()
            .map(|(i, (scope, st))| {
                serde_json::json!({"id": format!("check_{i}"), "scope": scope, "subject": null,
                    "status": st, "line": format!("line {i}"), "fix": null})
            })
            .collect();
        serde_json::json!({"schema": 1, "version": "0.4.0", "status": status, "checks": rows})
            .to_string()
    }

    #[test]
    fn the_verdict_is_the_top_level_status_and_nothing_else() {
        assert_eq!(
            judge(Some(1), doc("fail", &[("install", "fail")]).as_bytes()),
            3
        );
        assert_eq!(
            judge(Some(0), doc("warn", &[("install", "warn")]).as_bytes()),
            0
        );
        assert_eq!(judge(Some(0), doc("ok", &[]).as_bytes()), 0);
        // A host failure inside a document whose status is ok: still a pass (D-029).
        assert_eq!(judge(Some(0), doc("ok", &[("host", "fail")]).as_bytes()), 0);
    }

    #[test]
    fn no_valid_document_is_14_never_a_pass() {
        let ok = doc("ok", &[]);
        for (code, out) in [
            (Some(0), b"garbage".to_vec()),
            (Some(0), b"{}".to_vec()),
            (Some(2), ok.clone().into_bytes()),
            (None, ok.clone().into_bytes()),
            (
                Some(0),
                ok.replace("\"schema\":1", "\"schema\":2").into_bytes(),
            ),
            (Some(0), ok.replace("\"ok\"", "\"pass\"").into_bytes()),
            (
                Some(0),
                ok.replace("\"checks\":[]", "\"checks\":[],\"extra\":1")
                    .into_bytes(),
            ),
        ] {
            assert_eq!(
                judge(code, &out),
                14,
                "{code:?} {}",
                String::from_utf8_lossy(&out)
            );
        }
        // Security review P2-1 and P2-2: duplicate keys and self-contradiction are 14.
        let dup_status = ok.replacen("{", "{\"status\":\"fail\",", 1);
        let dup_schema = ok.replacen("{", "{\"schema\":2,", 1);
        assert_eq!(judge(Some(0), dup_status.as_bytes()), 14, "{dup_status}");
        assert_eq!(judge(Some(0), dup_schema.as_bytes()), 14, "{dup_schema}");
        let dup_in_check = doc("ok", &[("install", "ok")]).replace(
            "\"status\":\"ok\",\"line\"",
            "\"status\":\"fail\",\"status\":\"ok\",\"line\"",
        );
        assert_eq!(
            judge(Some(0), dup_in_check.as_bytes()),
            14,
            "{dup_in_check}"
        );
        let escaped_key = ok.replacen("{", "{\"st\\u0061tus\":\"fail\",", 1);
        assert_eq!(
            judge(Some(0), escaped_key.as_bytes()),
            14,
            "a key spelled with an escape"
        );
        assert_eq!(judge(Some(1), ok.as_bytes()), 14, "exit 1 with status ok");
        assert_eq!(
            judge(Some(1), doc("warn", &[]).as_bytes()),
            14,
            "exit 1 with status warn"
        );
        assert_eq!(
            judge(Some(0), doc("fail", &[("install", "fail")]).as_bytes()),
            3,
            "exit 0 with fail: still 3"
        );
        let missing_fix = doc("ok", &[("install", "ok")]).replace(",\"fix\":null", "");
        assert!(
            parse(missing_fix.as_bytes()).is_err(),
            "a required nullable key"
        );
        let no_scope = doc("ok", &[("install", "ok")]).replace("\"scope\":\"install\",", "");
        assert!(parse(no_scope.as_bytes()).is_err(), "scope is required");
    }

    /// rein's own sample document (vendored from cac86a4) parses, and its text is escaped.
    #[test]
    fn reins_sample_document_parses_and_prints_escaped() {
        let sample = include_str!("../tests/fixtures/doctor-sample.json");
        let doc = parse(sample.as_bytes()).unwrap();
        assert!(!doc.checks.is_empty());
        let mut evil = doc;
        evil.checks[0].line = "x\u{1b}[2J\u{202e}y".into();
        let lines = check_lines(&evil).join("\n");
        assert!(
            !lines.contains('\u{1b}') && !lines.contains('\u{202e}'),
            "{lines}"
        );
        // D-032: a literal backslash prints doubled, so text that spells `\u{1b}` cannot pass
        // for an escaped ESC.
        evil.checks[0].line = "spelled \\u{1b}, real \u{1b}".into();
        let lines = check_lines(&evil).join("\n");
        assert!(lines.contains(r"spelled \\u{1b}, real \u{1b}"), "{lines}");
    }
}
