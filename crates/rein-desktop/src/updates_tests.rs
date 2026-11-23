// The update watch's tests (moved out of updates.rs by E-87, unchanged but for E-87's own).
use super::*;
use std::cell::{Cell, RefCell};

fn cfg() -> Updates {
    Updates::default()
}

#[test]
fn a_version_is_one_short_line_of_safe_bytes() {
    assert_eq!(parse_version(b"0.5.0\n"), Some("0.5.0".into()));
    assert_eq!(
        parse_version(b"0.6.0-rc.1+g12\r\n"),
        Some("0.6.0-rc.1+g12".into())
    );
    assert_eq!(parse_version(b"1.0.0"), Some("1.0.0".into()));
    for bad in [
        &b""[..],
        b"\n0.5.0",
        b"0.5.0 beta",
        b"0.5.0;rm",
        b"\x1b[31m0.5",
        &[b'1'; 65],
    ] {
        assert_eq!(parse_version(bad), None, "{bad:?}");
    }
    assert_eq!(parse_version(&[b'1'; 64]).map(|v| v.len()), Some(64));
}

/// The schedule: start delay, interval, focus rule, nothing during a run, a read at its end.
#[test]
fn the_schedule_follows_the_clock() {
    let mut s = Schedule::default();
    let c = cfg();
    // Start: the first read at once, no check before 30 s.
    assert_eq!(
        s.due(0, &c, 5, false, false),
        Due {
            read: true,
            check: false
        }
    );
    s.read_done(0, Some(Some("0.5.0".into())));
    assert_eq!(s.due(4, &c, 5, false, false), Due::default());
    assert_eq!(
        s.due(5, &c, 5, false, false),
        Due {
            read: true,
            check: false
        }
    );
    s.read_done(5, Some(Some("0.5.0".into())));
    assert!(!s.due(29, &c, 5, false, false).check);
    assert!(s.due(30, &c, 5, false, false).check);
    s.check_done(30);
    // The interval: 30 minutes after the last check, not before.
    assert!(!s.due(30 + 1799, &c, 5, false, false).check);
    assert!(s.due(30 + 1800, &c, 5, false, false).check);
    s.check_done(1830);
    // Focus: a read at once; a check only when the last one is 10 minutes old.
    assert_eq!(
        s.due(1831, &c, 30, true, false),
        Due {
            read: true,
            check: false
        }
    );
    s.read_done(1831, Some(Some("0.5.0".into())));
    assert_eq!(
        s.due(1830 + 600, &c, 30, true, false),
        Due {
            read: true,
            check: true
        }
    );
    s.check_done(2430);
    s.read_done(2430, Some(Some("0.5.0".into())));
    // During a run nothing is due, even when it would be; its end is one read.
    assert_eq!(s.due(4300, &c, 30, true, true), Due::default());
    assert_eq!(s.due(4301, &c, 30, false, true), Due::default());
    let end = s.due(4302, &c, 30, false, false);
    assert!(end.read && end.check, "{end:?}");
    // check = false: nothing at all.
    let off = Updates {
        check: false,
        ..cfg()
    };
    assert_eq!(
        Schedule::default().due(10_000, &off, 5, true, false),
        Due::default()
    );
    // interval_min is read each tick.
    let mut s = Schedule::default();
    s.check_done(100);
    let ten = Updates {
        interval_min: 10,
        ..cfg()
    };
    assert!(s.due(700, &ten, 5, false, false).check);
}

#[test]
fn a_change_is_seen_once_and_never_on_the_first_read_or_a_failed_one() {
    let mut s = Schedule::default();
    assert!(!s.read_done(0, Some(Some("0.5.0".into()))));
    assert!(!s.read_done(5, Some(Some("0.5.0".into()))));
    assert!(
        !s.read_done(10, None),
        "a read that did not answer is no change"
    );
    assert!(s.read_done(15, Some(Some("0.5.1".into()))));
    assert!(!s.read_done(20, Some(Some("0.5.1".into()))));
    assert!(
        s.read_done(25, Some(None)),
        "a garbled file is a change to unknown"
    );
}

#[test]
fn a_check_answers_with_0_or_10_and_offline_keeps_the_last_newest() {
    let check = |code, newest: Option<&str>| ReleaseCheck {
        installed: Some("0.5.0".into()),
        newest: newest.map(Into::into),
        lines: vec![],
        code: Some(code),
    };
    let mut r = ReinFacts::default();
    apply_check(&mut r, &check(0, Some("0.5.1")), 7);
    assert_eq!(
        r,
        ReinFacts {
            installed: Some("0.5.0".into()),
            newest: Some("0.5.1".into()),
            checked_at: Some(7),
            offline: false
        }
    );
    apply_check(&mut r, &check(11, None), 8);
    assert_eq!(
        (r.newest.as_deref(), r.offline, r.checked_at),
        (Some("0.5.1"), true, Some(7))
    );
    apply_check(&mut r, &check(2, None), 9);
    assert_eq!(r.newest.as_deref(), Some("0.5.1"));
    apply_check(&mut r, &check(10, Some("0.5.1")), 10);
    assert_eq!((r.offline, r.checked_at), (false, Some(10)));
}

#[derive(Default)]
struct Fake {
    busy: Cell<bool>,
    version: RefCell<Option<Vec<u8>>>,
    newest: RefCell<Option<&'static str>>,
    checks: Cell<u32>,
    emitted: RefCell<Vec<UpdatesState>>,
    health: Cell<u32>,
    doc: RefCell<Option<Vec<u8>>>,
    key_id: RefCell<Option<String>>,
    app_docs: Cell<u32>,
    offered: Cell<u32>,
}

impl Io for Fake {
    fn busy(&self) -> bool {
        self.busy.get()
    }
    fn version(&self) -> Option<Vec<u8>> {
        self.version.borrow().clone()
    }
    fn check(&self) -> Result<ReleaseCheck, IpcError> {
        self.checks.set(self.checks.get() + 1);
        Ok(ReleaseCheck {
            installed: None,
            newest: self.newest.borrow().map(Into::into),
            lines: vec![],
            code: Some(0),
        })
    }
    fn emit(&self, state: &UpdatesState) {
        self.emitted.borrow_mut().push(state.clone());
    }
    fn installed_changed(&self) {
        self.health.set(self.health.get() + 1);
    }
    fn app_offered(&self) {
        self.offered.set(self.offered.get() + 1);
    }
    fn app_doc(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        self.app_docs.set(self.app_docs.get() + 1);
        self.doc.borrow().clone().map(|d| (d, b"signed".to_vec()))
    }
    fn app_trust(
        &self,
    ) -> (
        Box<dyn SignatureVerifier + Send + Sync>,
        &'static [&'static str],
    ) {
        (
            Box::new(Signed(self.key_id.borrow().clone().unwrap_or_default())),
            crate::app_update::ASSET_PREFIXES,
        )
    }
}

/// A verifier for one key id: a signature is good when it reads `signed`.
struct Signed(String);

impl SignatureVerifier for Signed {
    fn key_id(&self) -> Result<String, String> {
        Ok(self.0.clone())
    }
    fn verify(&self, _data: &mut dyn std::io::Read, signature: &str) -> Result<(), String> {
        (signature == "signed")
            .then_some(())
            .ok_or("bad signature".into())
    }
}

/// A changed VERSION under a running window: one event with the new version and one health
/// check, however many reads follow; no check while a run holds the runner.
#[test]
fn a_version_change_is_told_once_and_rechecks_health_once() {
    let w = Watch::new(Asked::default(), None);
    let io = Fake::default();
    *io.version.borrow_mut() = Some(b"0.5.0\n".to_vec());
    *io.newest.borrow_mut() = Some("0.5.1");
    let c = cfg();
    for t in 0..30 {
        w.step(t, 1000 + t, &c, 5, &io);
    }
    assert_eq!(
        io.emitted.borrow().len(),
        1,
        "the first read tells the version"
    );
    assert_eq!(io.health.get(), 0);
    *io.version.borrow_mut() = Some(b"0.5.1\n".to_vec());
    for t in 30..60 {
        w.step(t, 1000 + t, &c, 5, &io);
    }
    assert_eq!(io.health.get(), 1);
    assert_eq!(io.checks.get(), 1, "the start check, once");
    let last = w.state();
    assert_eq!(last.rein.installed.as_deref(), Some("0.5.1"));
    assert_eq!(last.rein.newest.as_deref(), Some("0.5.1"));
    assert_eq!(last.rein.checked_at, Some(1030));
    // A run in progress: no read, no check; focus waits for it.
    io.busy.set(true);
    w.focused();
    *io.version.borrow_mut() = Some(b"0.5.2\n".to_vec());
    for t in 60..3000 {
        w.step(t, 1000 + t, &c, 5, &io);
    }
    assert_eq!((io.health.get(), io.checks.get()), (1, 1));
    io.busy.set(false);
    w.step(3000, 4000, &c, 5, &io);
    assert_eq!((io.health.get(), io.checks.get()), (2, 2));
}

#[test]
fn later_is_kept_in_the_state() {
    let w = Watch::new(
        Asked {
            rein: Some("0.5.1".into()),
            app: None,
        },
        None,
    );
    assert_eq!(w.state().asked.rein.as_deref(), Some("0.5.1"));
    assert!(w.update(|s| s.asked.rein = Some("0.5.1".into())).is_none());
    assert!(w.update(|s| s.asked.rein = Some("0.5.2".into())).is_some());
}

fn app_doc_json(version: &str, key_id: &str) -> Vec<u8> {
    serde_json::json!({
        "schema": 1,
        "version": version,
        "key_id": key_id,
        "notes": "Notes.",
        "windows_x64": {
            "url": "https://github.com/Vimurai/rein-app/releases/download/v9/x.exe",
            "size": 10,
            "sha256": "0".repeat(64),
            "sig": "untrusted comment: x\nRUQ=\n",
        }
    })
    .to_string()
    .into_bytes()
}

/// E-86: the window's document rides the Rein check's schedule; a newer version for the
/// baked key is offered in place (on Windows) and kept for `app_update_start`; a key
/// mismatch is offered by hand and keeps nothing; `[updates] app = false` fetches nothing.
#[test]
fn the_window_document_rides_the_check_and_offers_only_a_newer_version() {
    const P: &[&str] = crate::app_update::ASSET_PREFIXES;
    let key = "0123456789ABCDEF";
    let w = Watch::new(Asked::default(), Some("0.0.9".into()));
    assert_eq!(w.state().app.updated_from.as_deref(), Some("0.0.9"));
    let io = Fake::default();
    *io.version.borrow_mut() = Some(b"0.5.0\n".to_vec());
    *io.doc.borrow_mut() = Some(app_doc_json("99.0.0", key));
    *io.key_id.borrow_mut() = Some(key.into());
    for t in 0..31 {
        w.step(t, 1000 + t, &cfg(), 5, &io);
    }
    assert_eq!(io.app_docs.get(), 1, "once, with the start check");
    let s = w.state();
    assert_eq!(s.app.newest.as_deref(), Some("99.0.0"));
    assert_eq!(s.app.notes.as_deref(), Some("Notes."));
    let in_place = crate::app_update::in_place_here();
    assert_eq!(s.app.mode, if in_place { "in_place" } else { "command" });
    assert_eq!(s.app.command.is_some(), !in_place);
    assert_eq!(w.doc.lock().unwrap().is_some(), in_place);
    // Another key: by hand on Windows, nothing kept to install.
    let other = Signed("FEDCBA9876543210".into());
    let changed = w.app_checked(&app_doc_json("99.0.1", key), b"signed", &other, P);
    assert!(changed.is_some());
    assert!(w.doc.lock().unwrap().is_none());
    assert_eq!(
        w.state().app.mode,
        if in_place { "by_hand" } else { "command" }
    );
    // Not newer: nothing offered. Garbage: nothing changes.
    let baked = Signed(key.into());
    w.app_checked(
        &app_doc_json(env!("CARGO_PKG_VERSION"), key),
        b"signed",
        &baked,
        P,
    );
    assert_eq!((w.state().app.newest, w.state().app.mode), (None, "none"));
    assert_eq!(
        w.state().app.notes.as_deref(),
        Some("Notes."),
        "the running version's own notes stay, for What's new"
    );
    w.app_checked(&app_doc_json("0.0.1", key), b"signed", &baked, P);
    assert_eq!(w.state().app.notes, None, "an older version's notes do not");
    assert!(w.app_checked(b"{}", b"signed", &baked, P).is_none());
    // A document whose own signature does not verify is no offer at all.
    assert!(
        w.app_checked(&app_doc_json("99.0.2", key), b"forged", &baked, P)
            .is_none()
    );
    assert_eq!(w.state().app.newest, None);
    // app = false: the Rein check runs, the window's document is never fetched.
    let off = Updates {
        app: false,
        ..cfg()
    };
    let w = Watch::new(Asked::default(), None);
    let io2 = Fake::default();
    *io2.doc.borrow_mut() = Some(app_doc_json("99.0.0", key));
    for t in 0..31 {
        w.step(t, 1000 + t, &off, 5, &io2);
    }
    assert_eq!((io2.checks.get(), io2.app_docs.get()), (1, 0));
}

/// E-87: a newer window raises `app_offered` once per version — not again on the next
/// check, not for a version answered "Later", again for a newer one.
#[test]
fn a_newer_window_asks_for_attention_once_per_version() {
    let key = "0123456789ABCDEF";
    let io = Fake::default();
    *io.version.borrow_mut() = Some(b"0.5.0\n".to_vec());
    *io.doc.borrow_mut() = Some(app_doc_json("99.0.0", key));
    *io.key_id.borrow_mut() = Some(key.into());
    let w = Watch::new(Asked::default(), None);
    for t in 0..31 {
        w.step(t, 1000 + t, &cfg(), 5, &io);
    }
    assert_eq!(io.offered.get(), 1);
    for t in 31..4000 {
        w.step(t, 1000 + t, &cfg(), 5, &io);
    }
    assert!(io.app_docs.get() > 1, "checked again");
    assert_eq!(io.offered.get(), 1, "once per version");
    *io.doc.borrow_mut() = Some(app_doc_json("99.0.1", key));
    w.update(|s| s.asked.app = Some("99.0.1".into()));
    for t in 4000..8000 {
        w.step(t, 1000 + t, &cfg(), 5, &io);
    }
    assert_eq!(io.offered.get(), 1, "Later: no attention for that version");
    *io.doc.borrow_mut() = Some(app_doc_json("99.0.2", key));
    for t in 8000..12000 {
        w.step(t, 1000 + t, &cfg(), 5, &io);
    }
    assert_eq!(io.offered.get(), 2, "a newer version asks once more");
}
