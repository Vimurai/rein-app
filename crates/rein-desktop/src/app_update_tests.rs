//! E-86: the window's own update — parse, offer, each `Step`, and end to end with a test key
//! over the real `ReqwestHttp` against a loopback server.

use super::*;
use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;

struct TestKey {
    pk: minisign::PublicKey,
    sk: minisign::SecretKey,
}

impl TestKey {
    fn generate() -> Self {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        Self {
            pk: kp.pk,
            sk: kp.sk,
        }
    }
    fn sign(&self, data: &[u8]) -> String {
        minisign::sign(Some(&self.pk), &self.sk, data, None, None)
            .unwrap()
            .into_string()
    }
    fn verifier(&self) -> MinisignVerifier {
        MinisignVerifier::from_pub_file(&self.pk.to_box().unwrap().into_string()).unwrap()
    }
    fn id(&self) -> String {
        self.verifier().key_id().unwrap()
    }
}

fn sha_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn doc_json(key: &TestKey, version: &str, url: &str, installer: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "schema": 1,
        "version": version,
        "key_id": key.id(),
        "notes": "Faster start.\nFewer clicks.",
        "windows_x64": {
            "url": url,
            "size": installer.len(),
            "sha256": sha_hex(installer),
            "sig": key.sign(installer),
        }
    })
}

const URL: &str =
    "https://github.com/Vimurai/rein-app/releases/download/v9.0.0/rein-app_9.0.0_x64-setup.exe";

fn parse(v: &serde_json::Value) -> Result<Doc, String> {
    parse_doc(v.to_string().as_bytes(), ASSET_PREFIXES)
}

#[test]
fn a_document_parses_strictly() {
    let key = TestKey::generate();
    let good = doc_json(&key, "9.0.0", URL, b"installer");
    let d = parse(&good).unwrap();
    assert_eq!(d.version, semver::Version::new(9, 0, 0));
    assert_eq!((d.size, d.url.as_str()), (9, URL));
    type Edit = Box<dyn Fn(&mut serde_json::Value)>;
    let cases: Vec<(&str, Edit)> = vec![
        ("schema", Box::new(|v| v["schema"] = 2.into())),
        ("unknown field", Box::new(|v| v["extra"] = 1.into())),
        (
            "unknown asset field",
            Box::new(|v| v["windows_x64"]["args"] = "/S".into()),
        ),
        ("short version", Box::new(|v| v["version"] = "9.0".into())),
        (
            "version text",
            Box::new(|v| v["version"] = "9.0.0 beta".into()),
        ),
        (
            "version newline",
            Box::new(|v| v["version"] = "9.0.0\n1".into()),
        ),
        (
            "lower key id",
            Box::new(|v| v["key_id"] = "abcdef0123456789".into()),
        ),
        ("short key id", Box::new(|v| v["key_id"] = "ABC".into())),
        (
            "long notes",
            Box::new(|v| v["notes"] = "x".repeat(4097).into()),
        ),
        (
            "escape in notes",
            Box::new(|v| v["notes"] = "\u{1b}[31mred".into()),
        ),
        (
            "http url",
            Box::new(|v| v["windows_x64"]["url"] = "http://github.com/x.exe".into()),
        ),
        (
            "other host",
            Box::new(|v| v["windows_x64"]["url"] = "https://evil.example/x.exe".into()),
        ),
        (
            "lookalike host",
            Box::new(|v| v["windows_x64"]["url"] = "https://github.com.evil.example/x".into()),
        ),
        (
            "userinfo host",
            Box::new(|v| v["windows_x64"]["url"] = "https://github.com@evil.example/x".into()),
        ),
        (
            "bare host",
            Box::new(|v| v["windows_x64"]["url"] = "https://github.com/".into()),
        ),
        (
            "space in url",
            Box::new(|v| v["windows_x64"]["url"] = "https://github.com/a b.exe".into()),
        ),
        ("size 0", Box::new(|v| v["windows_x64"]["size"] = 0.into())),
        (
            "size over cap",
            Box::new(|v| v["windows_x64"]["size"] = (MAX_INSTALLER_BYTES + 1).into()),
        ),
        (
            "upper sha",
            Box::new(|v| {
                let s = v["windows_x64"]["sha256"].as_str().unwrap().to_uppercase();
                v["windows_x64"]["sha256"] = s.into();
            }),
        ),
        (
            "short sha",
            Box::new(|v| v["windows_x64"]["sha256"] = "ab".into()),
        ),
        (
            "empty sig",
            Box::new(|v| v["windows_x64"]["sig"] = "".into()),
        ),
        (
            "another repository",
            Box::new(|v| {
                v["windows_x64"]["url"] =
                    "https://github.com/someone/else/releases/download/v9/x.exe".into()
            }),
        ),
        (
            "direction mark in notes",
            Box::new(|v| v["notes"] = "safe \u{202E}exe.txt".into()),
        ),
        (
            "missing asset",
            Box::new(|v| {
                v.as_object_mut().unwrap().remove("windows_x64");
            }),
        ),
    ];
    for (name, f) in &cases {
        let mut v = good.clone();
        f(&mut v);
        assert!(parse(&v).is_err(), "{name}");
    }
    let mut big = good.to_string().into_bytes();
    big.resize(MAX_DOC_BYTES as usize + 1, b' ');
    assert!(parse_doc(&big, ASSET_PREFIXES).is_err(), "over 64 KiB");
    // The objects CDN is the other allowed host.
    let mut cdn = good.clone();
    cdn["windows_x64"]["url"] = "https://objects.githubusercontent.com/a/b".into();
    assert!(parse(&cdn).is_ok());
}

/// E-87: the `minisign` tool writes its trusted comment as "timestamp:<n>\tfile:<name>\thashed";
/// a signature it made parses and verifies (the first window run offered nothing for it).
#[test]
fn a_signature_from_the_minisign_tool_parses_and_verifies() {
    let key = TestKey::generate();
    let installer = b"installer";
    let tool = |data: &[u8]| {
        minisign::sign(
            Some(&key.pk),
            &key.sk,
            data,
            Some("timestamp:1795445400\tfile:good.exe\thashed"),
            None,
        )
        .unwrap()
        .into_string()
    };
    let mut v = doc_json(&key, "9.0.0", URL, installer);
    v["windows_x64"]["sig"] = tool(installer).into();
    assert!(v["windows_x64"]["sig"].as_str().unwrap().contains('\t'));
    let body = v.to_string().into_bytes();
    let d = verified_doc(
        &body,
        tool(&body).as_bytes(),
        &key.verifier(),
        ASSET_PREFIXES,
    )
    .unwrap();
    assert!(key.verifier().verify(&mut &installer[..], &d.sig).is_ok());
    // Other control characters stay refused.
    v["windows_x64"]["sig"] = "untrusted comment: x\rRUQ=\n".into();
    assert!(parse(&v).is_err());
}

#[test]
fn only_a_newer_version_for_the_baked_key_is_offered_in_place() {
    let key = TestKey::generate();
    let id = key.id();
    let d = |v: &str| parse(&doc_json(&key, v, URL, b"x")).unwrap();
    assert_eq!(offer(&d("9.0.0"), "0.5.0", Some(&id)), Offer::InPlace);
    assert_eq!(
        offer(&d("0.5.0"), "0.5.0", Some(&id)),
        Offer::Nothing,
        "same version"
    );
    assert_eq!(
        offer(&d("0.4.9"), "0.5.0", Some(&id)),
        Offer::Nothing,
        "downgrade"
    );
    assert_eq!(
        offer(&d("0.5.1-rc.1"), "0.5.1", Some(&id)),
        Offer::Nothing,
        "older pre-release"
    );
    assert_eq!(
        offer(&d("9.0.0"), "0.5.0", Some("0000000000000000")),
        Offer::ByHand
    );
    assert_eq!(
        offer(&d("9.0.0"), "0.5.0", None),
        Offer::ByHand,
        "no key baked"
    );
    assert_eq!(mode(Offer::Nothing), ("none", None));
    if in_place_here() {
        assert_eq!(mode(Offer::InPlace), ("in_place", None));
        assert_eq!(mode(Offer::ByHand), ("by_hand", None));
    } else {
        assert_eq!(mode(Offer::InPlace).0, "command");
        assert!(mode(Offer::ByHand).1.is_some());
    }
}

#[test]
fn no_key_is_baked_until_the_release_task() {
    assert!(BAKED_APP_KEY.is_none());
    if std::env::var_os("REIN_DESKTOP_UPDATE_KEY").is_none() {
        assert!(
            verifier().key_id().is_err(),
            "nothing verifies without a key"
        );
    }
}

#[test]
fn the_installer_argv_is_constant() {
    assert_eq!(INSTALLER_ARGS, ["/P", "/UPDATE", "/R"]);
}

#[test]
fn the_job_is_sent_in_the_data_model_shape() {
    let j = |job: Job| serde_json::to_value(job).unwrap();
    assert_eq!(j(Job::Idle), serde_json::json!("idle"));
    assert_eq!(
        j(Job::Downloading { done: 1, total: 2 }),
        serde_json::json!({"downloading": {"done": 1, "total": 2}})
    );
    assert_eq!(j(Job::Verifying), serde_json::json!("verifying"));
    assert_eq!(j(Job::Restarting), serde_json::json!("restarting"));
    assert_eq!(
        j(Job::Failed(Step::TooLarge)),
        serde_json::json!({"failed": "too_large"})
    );
}

/// Serves `body` for every request (`Http` without a network).
struct Fixed(Vec<u8>);

impl Http for Fixed {
    fn download(&self, url: &str, cap: u64, out: &mut dyn Write) -> Result<u64, FetchError> {
        // As the real client: at most `cap` bytes reach `out`, then too large.
        let n = (self.0.len() as u64).min(cap + 1) as usize;
        for chunk in self.0[..n].chunks(64 * 1024) {
            out.write_all(chunk)?;
        }
        if n as u64 > cap {
            return Err(FetchError::TooLarge {
                url: url.into(),
                cap,
            });
        }
        Ok(n as u64)
    }
}

/// What one install did.
#[derive(Debug, Default)]
struct Seen {
    jobs: Vec<Job>,
    calls: Vec<String>,
}

fn run(
    doc: &Doc,
    http: &dyn Http,
    key: &TestKey,
    dir: &Path,
    cancel: &AtomicBool,
    start_ok: bool,
) -> (Result<Outcome, Step>, Seen) {
    let seen = RefCell::new(Seen::default());
    let v = key.verifier();
    let ctx = Ctx {
        http,
        verifier: &v,
        dir,
        run: "t1",
        cancel,
    };
    let r = install(
        doc,
        &ctx,
        &mut |j| seen.borrow_mut().jobs.push(j),
        &mut || seen.borrow_mut().calls.push("quit".into()),
        &mut |p| {
            assert!(p.is_file(), "the installer exists when it is started");
            seen.borrow_mut().calls.push(format!(
                "start {}",
                p.file_name().unwrap().to_string_lossy()
            ));
            if start_ok {
                Ok(())
            } else {
                Err(io::Error::other("no"))
            }
        },
    );
    (r, seen.into_inner())
}

fn files(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_good_installer_is_verified_then_the_quit_runs_then_the_start() {
    let key = TestKey::generate();
    let body = vec![7u8; 300_000];
    let doc = parse(&doc_json(&key, "9.0.0", URL, &body)).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (r, seen) = run(
        &doc,
        &Fixed(body),
        &key,
        tmp.path(),
        &AtomicBool::new(false),
        true,
    );
    assert_eq!(r, Ok(Outcome::Started));
    assert_eq!(seen.calls, ["quit", "start rein-app-update-t1.exe"]);
    assert_eq!(
        seen.jobs.first(),
        Some(&Job::Downloading {
            done: 0,
            total: 300_000
        })
    );
    let tail = &seen.jobs[seen.jobs.len() - 3..];
    assert_eq!(
        tail,
        [
            Job::Downloading {
                done: 300_000,
                total: 300_000
            },
            Job::Verifying,
            Job::Restarting
        ]
    );
    assert!(
        seen.jobs.len() <= 104,
        "at most one progress report per percent"
    );
    assert_eq!(
        files(tmp.path()),
        ["rein-app-update-t1.exe"],
        "left for the installer"
    );
}

#[test]
fn every_failed_step_leaves_no_file_and_starts_nothing() {
    let key = TestKey::generate();
    let body = b"the real installer".to_vec();
    let good = doc_json(&key, "9.0.0", URL, &body);
    let cases: Vec<(&str, serde_json::Value, Vec<u8>, Step)> = vec![
        (
            "tampered",
            good.clone(),
            b"the evil installer".to_vec(),
            Step::Signature,
        ),
        (
            "signed by another key",
            {
                let mut v = good.clone();
                v["windows_x64"]["sig"] = TestKey::generate().sign(&body).into();
                v
            },
            body.clone(),
            Step::Signature,
        ),
        (
            "wrong sha",
            {
                let mut v = good.clone();
                v["windows_x64"]["sha256"] = sha_hex(b"other").into();
                v
            },
            body.clone(),
            Step::Checksum,
        ),
        (
            "larger than the document",
            good.clone(),
            [&body[..], b"!"].concat(),
            Step::TooLarge,
        ),
        (
            "shorter than the document",
            good.clone(),
            body[..5].to_vec(),
            Step::Fetch,
        ),
    ];
    for (name, doc, served, step) in cases {
        let doc = parse(&doc).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let (r, seen) = run(
            &doc,
            &Fixed(served),
            &key,
            tmp.path(),
            &AtomicBool::new(false),
            true,
        );
        assert_eq!(r, Err(step), "{name}");
        assert!(
            seen.calls.is_empty(),
            "{name}: no quit, no start: {:?}",
            seen.calls
        );
        assert!(files(tmp.path()).is_empty(), "{name}: file deleted");
    }
    // The start itself fails: the quit has run, the file is deleted.
    let doc = parse(&good).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (r, seen) = run(
        &doc,
        &Fixed(body),
        &key,
        tmp.path(),
        &AtomicBool::new(false),
        false,
    );
    assert_eq!(r, Err(Step::Start));
    assert_eq!(seen.calls.len(), 2);
    assert!(files(tmp.path()).is_empty());
}

#[test]
fn cancel_stops_the_download_and_deletes_the_file() {
    let key = TestKey::generate();
    let body = vec![1u8; 200_000];
    let doc = parse(&doc_json(&key, "9.0.0", URL, &body)).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(true);
    let (r, seen) = run(&doc, &Fixed(body), &key, tmp.path(), &cancel, true);
    assert_eq!(r, Ok(Outcome::Cancelled));
    assert!(seen.calls.is_empty());
    assert!(files(tmp.path()).is_empty());
}

#[test]
fn an_existing_file_of_the_same_run_is_never_overwritten() {
    let key = TestKey::generate();
    let body = b"x".to_vec();
    let doc = parse(&doc_json(&key, "9.0.0", URL, &body)).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let planted = tmp.path().join("rein-app-update-t1.exe");
    fs::write(&planted, b"planted").unwrap();
    let (r, _) = run(
        &doc,
        &Fixed(body),
        &key,
        tmp.path(),
        &AtomicBool::new(false),
        true,
    );
    assert_eq!(r, Err(Step::Fetch));
}

#[test]
fn clean_deletes_only_our_installers() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("rein-app-update-1.exe"), b"").unwrap();
    fs::write(tmp.path().join("rein-app-update-2.exe"), b"").unwrap();
    fs::write(tmp.path().join("keep.txt"), b"").unwrap();
    fs::create_dir(tmp.path().join("rein-app-update-dir")).unwrap();
    clean(tmp.path());
    let mut left = files(tmp.path());
    left.sort();
    assert_eq!(left, ["keep.txt", "rein-app-update-dir"]);
    clean(&tmp.path().join("missing"));
}

const LOOPBACK: &[&str] = &["http://127.0.0.1:"];

/// End to end with the test key over the real `ReqwestHttp` (updates.md §Tests): a good update
/// reaches the start; a tampered installer stops at `signature`, a wrong sha at `checksum`; an
/// older version is not offered. The document's own signature (security review of E-86): a
/// tampered document, one signed by another key, a missing signature and a correctly signed
/// document for an older version are never offered.
#[test]
fn end_to_end_with_the_test_key() {
    let key = TestKey::generate();
    let stranger = TestKey::generate();
    let installer = vec![42u8; 1_000_000];
    let tampered = {
        let mut t = installer.clone();
        t[500_000] ^= 1;
        t
    };
    let base = serve_with(|base| {
        let doc = |v: &str, file: &str, sha_of: &[u8]| {
            let mut d = doc_json(&key, v, &format!("{base}/{file}"), &installer);
            d["windows_x64"]["sha256"] = sha_hex(sha_of).into();
            d.to_string().into_bytes()
        };
        // `<name>.json` and its `.minisig`, signed by `by`.
        let signed = |name: &str, body: Vec<u8>, by: &TestKey| {
            vec![
                (format!("/{name}.json.minisig"), by.sign(&body).into_bytes()),
                (format!("/{name}.json"), body),
            ]
        };
        let good = doc("9.0.0", "good.exe", &installer);
        let relabelled = {
            // Signed as 0.0.1, served as 99.0.0: the rollback the review found.
            let older = doc("0.0.1", "good.exe", &installer);
            let sig = key.sign(&older);
            let text = String::from_utf8(older)
                .unwrap()
                .replace("\"0.0.1\"", "\"99.0.0\"");
            vec![
                ("/relabelled.json.minisig".to_string(), sig.into_bytes()),
                ("/relabelled.json".to_string(), text.into_bytes()),
            ]
        };
        let mut routes = Vec::new();
        routes.extend(signed("good", good.clone(), &key));
        routes.extend(signed(
            "tampered",
            doc("9.0.0", "tampered.exe", &installer),
            &key,
        ));
        routes.extend(signed("wrongsha", doc("9.0.0", "good.exe", b"other"), &key));
        routes.extend(signed("older", doc("0.0.1", "good.exe", &installer), &key));
        routes.extend(signed("stranger", good.clone(), &stranger));
        routes.extend(relabelled);
        routes.push(("/unsigned.json".into(), good));
        routes.push(("/good.exe".into(), installer.clone()));
        routes.push(("/tampered.exe".into(), tampered.clone()));
        routes
    });
    let http = rein_release::ReqwestHttp::with_timeout(HostPolicy::loopback_only(), CHECK_TIMEOUT)
        .unwrap();
    let id = key.id();
    let v = key.verifier();
    let document = |name: &str| -> Result<Doc, String> {
        let (bytes, sig) =
            fetch_doc(&http, &format!("{base}/{name}")).map_err(|e| e.to_string())?;
        verified_doc(&bytes, &sig, &v, LOOPBACK)
    };
    let attempt = |name: &str| {
        let doc = document(name).unwrap();
        let o = offer(&doc, "0.5.0", Some(&id));
        if o != Offer::InPlace {
            return (None, o);
        }
        let tmp = tempfile::tempdir().unwrap();
        let (r, seen) = run(&doc, &http, &key, tmp.path(), &AtomicBool::new(false), true);
        (Some((r, seen.calls, files(tmp.path()))), o)
    };
    let (good, _) = attempt("good.json");
    let (r, calls, left) = good.unwrap();
    assert_eq!(r, Ok(Outcome::Started));
    assert_eq!(calls, ["quit", "start rein-app-update-t1.exe"]);
    assert_eq!(left, ["rein-app-update-t1.exe"]);
    let (tampered, _) = attempt("tampered.json");
    let (r, calls, left) = tampered.unwrap();
    assert_eq!((r, calls.len(), left.len()), (Err(Step::Signature), 0, 0));
    let (wrong, _) = attempt("wrongsha.json");
    let (r, calls, left) = wrong.unwrap();
    assert_eq!((r, calls.len(), left.len()), (Err(Step::Checksum), 0, 0));
    let (older, o) = attempt("older.json");
    assert!(older.is_none(), "a correctly signed older document");
    assert_eq!(o, Offer::Nothing);
    for name in [
        "relabelled.json",
        "stranger.json",
        "unsigned.json",
        "none.json",
    ] {
        assert!(document(name).is_err(), "{name} is never offered");
    }
}

/// `GET <path>` → the route's body, else 404; the routes may name the server's own base URL.
fn serve_with(routes: impl FnOnce(&str) -> Vec<(String, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes = routes(&base);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            if reader.read_line(&mut first).is_err() {
                continue;
            }
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok() && line.trim_end() != "" {
                line.clear();
            }
            let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let resp = match routes.iter().find(|(p, _)| *p == path) {
                Some((_, b)) => {
                    let mut r = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        b.len()
                    )
                    .into_bytes();
                    r.extend_from_slice(b);
                    r
                }
                None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_vec(),
            };
            let _ = stream.write_all(&resp);
        }
    });
    base
}

/// D-066 addendum 1 r3: one real HTTPS GET to github.com from the native Windows build
/// (rustls with the platform verifier → the Windows certificate store). Any HTTP status is
/// a pass — there is no release yet, so 404 is expected; a TLS or network failure is not.
#[cfg(windows)]
#[test]
fn a_real_https_get_reaches_github_from_windows() {
    let http = rein_release::ReqwestHttp::with_timeout(HostPolicy::github_assets(), CHECK_TIMEOUT)
        .unwrap();
    let r = fetch_doc(&http, LATEST_URL);
    eprintln!("E-86 windows https GET {LATEST_URL}: {r:?}");
    match r {
        Ok(_) | Err(FetchError::Status { .. }) => {}
        Err(e) if std::env::var_os("CI").is_none() => {
            eprintln!("no network here (not CI), not failing: {e}");
        }
        Err(e) => panic!("the HTTPS GET did not complete: {e}"),
    }
}
