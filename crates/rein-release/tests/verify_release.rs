//! Verifier behaviour against fixture releases (blueprint §Test Plan, first bullet).
//! One table: each row is a release defect and the check that must reject it. Every bad
//! row also asserts that nothing was written under the temp HOME.

mod support;

use std::fs;
use std::path::Path;

use rein_release::{MinisignVerifier, Rejecting, SignatureVerifier, VerifyError, verify_release};
use support::*;

fn requested() -> semver::Version {
    semver::Version::parse(VERSION).unwrap()
}

fn home_is_empty(home: &Path) -> bool {
    fs::read_dir(home).unwrap().next().is_none()
}

/// Verify `release` with `verifier` into `<tmp HOME>/.rein.extract`.
fn run(release: &Release, verifier: &dyn SignatureVerifier) -> (Result<(), VerifyError>, bool) {
    let tmp = tempfile::tempdir().unwrap();
    let (dl, home) = (tmp.path().join("dl"), tmp.path().join("home"));
    fs::create_dir_all(&dl).unwrap();
    fs::create_dir_all(&home).unwrap();
    let assets = release.write_to(&dl);
    let result =
        verify_release(&assets, verifier, &requested(), &home.join(".rein.extract")).map(|_| ());
    let clean = home_is_empty(&home);
    (result, clean)
}

#[test]
fn good_release_extracts_every_listed_file_with_its_mode() {
    let key = TestKey::generate();
    let tmp = tempfile::tempdir().unwrap();
    let assets = Release::good(&key).write_to(tmp.path());
    let dest = tmp.path().join(".rein.extract");
    let verified = verify_release(&assets, &key.verifier(), &requested(), &dest).unwrap();

    assert_eq!(verified.manifest.version.to_string(), VERSION);
    for f in good_files() {
        assert_eq!(fs::read(dest.join(f.path)).unwrap(), f.bytes, "{}", f.path);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dest.join(f.path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(format!("0{mode:o}"), f.mode, "{}", f.path);
        }
    }
    assert_eq!(
        fs::read_to_string(dest.join("manifest.json")).unwrap(),
        fs::read_to_string(&assets.manifest).unwrap()
    );
}

/// A defective release and the predicate its error must satisfy.
struct Case {
    label: &'static str,
    build: fn(&TestKey) -> Release,
    expect: fn(&VerifyError) -> bool,
}

fn with_manifest_edit(key: &TestKey, edit: fn(String) -> String) -> Release {
    let files = good_files();
    let manifest = edit(manifest_json(&key.key_id(), &files));
    Release::signed(key, &entries_for(&files, &manifest), manifest)
}

fn with_entries(key: &TestKey, extra: Vec<Entry>) -> Release {
    let files = good_files();
    let manifest = manifest_json(&key.key_id(), &files);
    let mut entries = entries_for(&files, &manifest);
    entries.extend(extra);
    Release::signed(key, &entries, manifest)
}

const CASES: &[Case] = &[
    Case {
        label: "bad .minisig (signature over other bytes)",
        build: |k| {
            let mut r = Release::good(k);
            r.signature = k.sign(b"not the tarball");
            r
        },
        expect: |e| matches!(e, VerifyError::BadSignature(_)),
    },
    Case {
        label: "tampered file (same size, different bytes)",
        build: |k| {
            let files = good_files();
            let manifest = manifest_json(&k.key_id(), &files);
            let mut tampered = files.clone();
            tampered[2].bytes = b"rein_locate() { X; }\n".to_vec();
            Release::signed(k, &entries_for(&tampered, &manifest), manifest)
        },
        expect: |e| matches!(e, VerifyError::HashMismatch(p) if p == "shared/locate.sh"),
    },
    Case {
        label: "manifest missing from the tarball",
        build: |k| {
            let files = good_files();
            let manifest = manifest_json(&k.key_id(), &files);
            let entries: Vec<Entry> = entries_for(&files, &manifest).into_iter().skip(1).collect();
            Release::signed(k, &entries, manifest)
        },
        expect: |e| matches!(e, VerifyError::ManifestNotInTarball),
    },
    Case {
        label: "older signed release served for the requested version",
        build: |k| {
            with_manifest_edit(k, |m| {
                m.replacen("\"version\":\"0.5.0\"", "\"version\":\"0.4.0\"", 1)
            })
        },
        expect: |e| matches!(e, VerifyError::VersionMismatch { .. }),
    },
    Case {
        label: "directory entry carrying data",
        build: |k| {
            let files = good_files();
            let manifest = manifest_json(&k.key_id(), &files);
            let mut entries = entries_for(&files, &manifest);
            entries.push(Entry::DirWithData("bin/sub/", b"payload".to_vec()));
            Release::signed(k, &entries, manifest)
        },
        expect: |e| matches!(e, VerifyError::UnsupportedEntry { .. }),
    },
    Case {
        label: "manifest schema 2",
        build: |k| with_manifest_edit(k, |m| m.replacen("\"schema\":1", "\"schema\":2", 1)),
        expect: |e| matches!(e, VerifyError::UnsupportedSchema(2)),
    },
    Case {
        label: "key_id mismatch names the newer rein-app",
        build: |k| {
            let other = TestKey::generate();
            let files = good_files();
            let manifest = manifest_json(&other.key_id(), &files);
            Release::signed(k, &entries_for(&files, &manifest), manifest)
        },
        expect: |e| {
            matches!(e, VerifyError::NeedsNewerApp { .. })
                && e.to_string().contains("install a newer rein-app")
                && !e.to_string().contains('\n')
        },
    },
    Case {
        label: "in-tarball manifest differs from the beside one",
        build: |k| {
            let files = good_files();
            let manifest = manifest_json(&k.key_id(), &files);
            let inner = format!("{manifest} ");
            let mut r = Release::signed(k, &entries_for(&files, &inner), manifest.clone());
            r.manifest = manifest;
            r
        },
        expect: |e| matches!(e, VerifyError::ManifestMismatch),
    },
    Case {
        label: "unlisted tarball file",
        build: |k| with_entries(k, vec![Entry::File("shared/extra.mjs", b"x".to_vec())]),
        expect: |e| matches!(e, VerifyError::Unlisted(p) if p == "shared/extra.mjs"),
    },
    Case {
        label: "listed file missing from the tarball",
        build: |k| {
            let files = good_files();
            let manifest = manifest_json(&k.key_id(), &files);
            Release::signed(k, &entries_for(&files[..3], &manifest), manifest)
        },
        expect: |e| matches!(e, VerifyError::Missing(p) if p == "config/defaults.json"),
    },
    Case {
        label: "'..' traversal entry",
        build: |k| with_entries(k, vec![Entry::File("bin/../../evil", b"x".to_vec())]),
        expect: |e| matches!(e, VerifyError::UnsafePath(_)),
    },
    Case {
        label: "absolute path entry",
        build: |k| with_entries(k, vec![Entry::File("/tmp/evil", b"x".to_vec())]),
        expect: |e| matches!(e, VerifyError::UnsafePath(_)),
    },
    Case {
        label: "tarball member not in `files` (outside the layout)",
        build: |k| with_entries(k, vec![Entry::File("evil/x", b"x".to_vec())]),
        expect: |e| matches!(e, VerifyError::Unlisted(p) if p == "evil/x"),
    },
    Case {
        label: "manifest lists a path outside the layout",
        build: |k| {
            with_manifest_edit(k, |m| {
                m.replace("\"config/defaults.json\"", "\"evil/defaults.json\"")
            })
        },
        expect: |e| matches!(e, VerifyError::OutsideLayout(_)),
    },
    Case {
        label: "wrong mode in the manifest",
        build: |k| with_manifest_edit(k, |m| m.replacen("\"0644\"", "\"0777\"", 1)),
        expect: |e| matches!(e, VerifyError::ManifestInvalid(m) if m.contains("0777")),
    },
    Case {
        label: "symlink entry",
        build: |k| with_entries(k, vec![Entry::Symlink("bin/link", "/etc/passwd")]),
        expect: |e| matches!(e, VerifyError::UnsupportedEntry { kind, .. } if kind == "Symlink"),
    },
    Case {
        label: "duplicate entry",
        build: |k| {
            with_entries(
                k,
                vec![Entry::File("bin/rein", b"#!/bin/sh\necho rein\n".to_vec())],
            )
        },
        expect: |e| matches!(e, VerifyError::Duplicate(p) if p == "bin/rein"),
    },
    Case {
        label: "a directory entry (rein's writer emits none)",
        build: |k| with_entries(k, vec![Entry::Dir("bin/")]),
        expect: |e| matches!(e, VerifyError::UnsupportedEntry { kind, .. } if kind == "Directory"),
    },
    // Member rules, rein D-155 r1 (E-12).
    Case {
        label: "hard link member",
        build: |k| with_entries(k, vec![Entry::HardLink("bin/again", "bin/rein")]),
        expect: |e| matches!(e, VerifyError::UnsupportedEntry { kind, .. } if kind == "Link"),
    },
    Case {
        label: "pax extension header",
        build: |k| {
            let files = good_files();
            let manifest = manifest_json(&k.key_id(), &files);
            let mut entries = entries_for(&files, &manifest);
            entries.insert(
                1,
                Entry::Pax("PaxHeader/bin/rein", b"21 path=bin/rein2\n".to_vec()),
            );
            Release::signed(k, &entries, manifest)
        },
        expect: |e| matches!(e, VerifyError::UnsupportedEntry { kind, .. } if kind == "XHeader"),
    },
    Case {
        label: "manifest.json not the first member",
        build: |k| {
            let files = good_files();
            let manifest = manifest_json(&k.key_id(), &files);
            let mut entries = entries_for(&files, &manifest);
            let m = entries.remove(0);
            entries.push(m);
            Release::signed(k, &entries, manifest)
        },
        expect: |e| matches!(e, VerifyError::ManifestNotFirst),
    },
    Case {
        label: "old-style NUL typeflag (rein writes '0')",
        build: |k| with_entries(k, vec![Entry::Typed("shared/x.mjs", b"x".to_vec(), 0)]),
        expect: |e| matches!(e, VerifyError::UnsupportedEntry { kind, .. } if kind.contains("0x00")),
    },
    Case {
        label: "GNU long-name header",
        build: |k| {
            with_entries(
                k,
                vec![Entry::Typed("././@LongLink", b"bin/rein2\0".to_vec(), b'L')],
            )
        },
        expect: |e| matches!(e, VerifyError::UnsupportedEntry { kind, .. } if kind.contains("GNULongName")),
    },
    Case {
        label: "a member after the end-of-archive marker",
        build: |k| {
            with_entries(
                k,
                vec![Entry::EndMarker, Entry::File("bin/evil", b"x".to_vec())],
            )
        },
        expect: |e| matches!(e, VerifyError::TarballUnreadable(m) if m.contains("after the end")),
    },
    Case {
        label: "a VERSION member",
        build: |k| with_entries(k, vec![Entry::File("VERSION", b"0.5.0\n".to_vec())]),
        expect: |e| matches!(e, VerifyError::ForbiddenMember(p) if p == "VERSION"),
    },
    Case {
        label: "a top-level path in `files`",
        build: |k| shipping(k, "accounts.json"),
        expect: |e| matches!(e, VerifyError::OutsideLayout(p) if p == "accounts.json"),
    },
    Case {
        label: "an operator-owned-only path in `files` (not under a mirrored dir)",
        build: |k| shipping(k, "secrets/token"),
        expect: |e| matches!(e, VerifyError::OutsideLayout(p) if p == "secrets/token"),
    },
    // Path alphabet and case rule, rein D-155 addendum 3 (D-020, E-26).
    Case {
        label: "two `files` paths differing only by case",
        build: |k| shipping(k, "shared/Locate.sh"),
        expect: |e| {
            matches!(e, VerifyError::ManifestInvalid(m)
            if m.contains("'shared/locate.sh' and 'shared/Locate.sh'"))
        },
    },
    Case {
        label: "a `files` path colliding by case with another's directory",
        build: |k| shipping(k, "MCP/x.json"),
        expect: |e| matches!(e, VerifyError::ManifestInvalid(m) if m.contains("'mcp' and 'MCP'")),
    },
    Case {
        label: "a non-ASCII `files` path",
        build: |k| shipping(k, "shared/r\u{e9}in.sh"),
        expect: |e| matches!(e, VerifyError::ManifestInvalid(m) if m.contains("printable ASCII")),
    },
    Case {
        label: "legacy (non-prehashed) signature",
        build: |k| {
            let mut r = Release::good(k);
            r.signature = k.legacy_sign(&r.tarball);
            r
        },
        expect: |e| matches!(e, VerifyError::BadSignature(m) if m.to_lowercase().contains("legacy")),
    },
];

/// A correctly signed release whose manifest lists and ships `path` too.
fn shipping(key: &TestKey, path: &'static str) -> Release {
    let mut files = good_files();
    files.push(Listed {
        path,
        bytes: b"x".to_vec(),
        mode: "0644",
    });
    let manifest = manifest_json(&key.key_id(), &files);
    Release::signed(key, &entries_for(&files, &manifest), manifest)
}

#[test]
fn every_defective_release_is_rejected_by_its_check_and_writes_nothing() {
    for case in CASES {
        let key = TestKey::generate();
        let (result, clean) = run(&(case.build)(&key), &key.verifier());
        let err = result.expect_err(case.label);
        assert!((case.expect)(&err), "{}: wrong error: {err}", case.label);
        assert!(clean, "{}: wrote under HOME", case.label);
    }
}

/// E-16 acceptance, extending `good_release_extracts_every_listed_file_with_its_mode`: the
/// production path builds its verifier from a committed `minisign.pub` file. Built from a
/// test key's file it verifies a release signed with that key and refuses (key id first,
/// nothing written) one signed with another. The production key's secret half is never
/// available to tests, so the baked file itself is covered by the parser and key-id unit
/// tests in `signature.rs`.
#[test]
fn a_verifier_from_a_pub_file_accepts_its_own_key_and_refuses_another() {
    let key = TestKey::generate();
    let verifier = MinisignVerifier::from_pub_file(&key.pub_file()).unwrap();
    assert_eq!(verifier.key_id().unwrap(), key.key_id());
    let (result, clean) = run(&Release::good(&key), &verifier);
    assert!(result.is_ok(), "{result:?}");
    assert!(!clean, "the good release is extracted");

    let other = TestKey::generate();
    let (result, clean) = run(&Release::good(&other), &verifier);
    assert!(
        matches!(result, Err(VerifyError::NeedsNewerApp { .. })),
        "{result:?}"
    );
    assert!(clean);
}

#[test]
fn kill_switch_backend_rejects_a_good_release() {
    let key = TestKey::generate();
    let (result, clean) = run(&Release::good(&key), &Rejecting::new("kill switch"));
    assert!(matches!(result, Err(VerifyError::NoTrustedKey(_))));
    assert!(clean);
}

#[test]
fn existing_destination_is_refused_and_left_untouched() {
    let key = TestKey::generate();
    let tmp = tempfile::tempdir().unwrap();
    let assets = Release::good(&key).write_to(tmp.path());
    let dest = tmp.path().join(".rein.extract");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("keep"), b"mine").unwrap();
    let err = verify_release(&assets, &key.verifier(), &requested(), &dest).unwrap_err();
    assert!(matches!(err, VerifyError::DestinationExists(_)));
    assert_eq!(fs::read(dest.join("keep")).unwrap(), b"mine");
}
