//! Layout equality against rein's own installer (E-9; blueprint §Test Plan "Layout
//! equality"; D-012 ruling 3; rein D-155 addendum r1).
//!
//! From a rein checkout (`REIN_CHECKOUT`, a path; the test is skipped without it):
//! - HOME A: `bash install-rein.sh`, then `rein install` (the clone install);
//! - HOME B: `rein release stage` + `rein release manifest` (rein's own generator),
//!   signed with a throwaway TEST key, served from a loopback fixture server, and
//!   installed by the same `install_cmd::install` that `rein-app install` runs (which
//!   runs `rein install` and `rein doctor` from the new tree). Only the key and the
//!   server differ from production: the shipped binary has no seam to trust another key.
//!
//! `$HOME/.rein` of A and B must then be equal except the documented differences:
//! `manifest.json` only in B, `config/install-stamp.json` differs,
//! `config/rein-workspace.txt` only in A, and two per-install values differ (D-028):
//! `secrets/role-hmac.key` (a random secret) and `shared/toolchain.sh` (its recorded
//! timestamp). A second case seeds both homes with a retired server directory before
//! re-running; `rein install` removes it on both sides.
//!
//! `rein doctor` also checks the host (Claude, Chromium, gh copilot, the cwd as a project),
//! which a sterile HOME never passes, so install may end 3 (D-028): the exit must be 0 or 3,
//! and doctor run from the installed tree must not report `[INSTALL_TREE_MISMATCH]` or
//! `[INSTALL_INCOMPLETE]`. At rein 3f68eff it does (rein `rein install` rewrites a shipped
//! lock file): the test is red there by design until rein fixes it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rein_release::{HostPolicy, ReleaseSource, ReqwestHttp};
use semver::Version;

use crate::install_cmd::support::{Route, TestKey, serve};
use crate::install_cmd::{Deps, install};
use crate::runner::SystemProcess;

/// Differences the blueprint documents (D-155 addendum r1).
const EXPECTED: &[(&str, &str)] = &[
    ("manifest.json", "only in B"),
    ("config/install-stamp.json", "differs"),
    ("config/rein-workspace.txt", "only in A"),
    ("secrets/role-hmac.key", "differs"),
    ("shared/toolchain.sh", "differs"),
];

/// Doctor markers that mean the installed tree itself is wrong (D-028).
const TREE_MARKERS: &[&str] = &["[INSTALL_TREE_MISMATCH]", "[INSTALL_INCOMPLETE]"];

/// `rein doctor` from the installed tree, from a neutral cwd: the tree findings it prints.
fn tree_findings(home: &Path, cwd: &Path) -> Vec<String> {
    let out = clean(&mut Command::new(home.join(".rein/bin/rein")), home)
        .arg("doctor")
        .current_dir(cwd)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| TREE_MARKERS.iter().any(|m| l.contains(m)))
        .map(|(i, l)| {
            // The marker line and the paths rein names under it.
            let detail: Vec<&str> = lines[i + 1..]
                .iter()
                .take_while(|n| n.starts_with("    "))
                .copied()
                .collect();
            format!(
                "{}{}",
                l.trim(),
                detail
                    .iter()
                    .map(|d| format!("\n  {}", d.trim()))
                    .collect::<String>()
            )
        })
        .collect()
}

/// Exit 0 or 3, no tree finding from doctor, no undocumented difference (D-028).
fn check(code: u8, a: &Path, b: &Path, cwd: &Path, case: &str) {
    let findings = tree_findings(b, cwd);
    let diff = undocumented(&a.join(".rein"), &b.join(".rein"));
    assert!(
        matches!(code, 0 | 3) && findings.is_empty() && diff.is_empty(),
        "{case}: rein-app install exited {code} (0 or 3 allowed)\n\
         doctor tree findings:\n{}\nundocumented differences:\n{}",
        findings.join("\n"),
        diff.join("\n")
    );
}

/// A clean environment for rein's own scripts: PATH (node, npm, git) and HOME only.
fn clean<'c>(cmd: &'c mut Command, home: &Path) -> &'c mut Command {
    cmd.env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed ({:?}):\n{}\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Every entry under `root`: relative path → file bytes, symlink target, or directory.
fn tree(root: &Path) -> BTreeMap<String, String> {
    fn go(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            let meta = fs::symlink_metadata(&p).unwrap();
            if meta.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("link -> {}", fs::read_link(&p).unwrap().display()),
                );
            } else if meta.is_dir() {
                out.insert(rel.clone(), "dir".into());
                go(root, &p, out);
            } else {
                let bytes = fs::read(&p).unwrap();
                out.insert(
                    rel,
                    format!("file {} bytes, {:x}", bytes.len(), md5ish(&bytes)),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    go(root, root, &mut out);
    out
}

/// A cheap content fingerprint (FNV-1a); equality of bytes is what matters here.
fn md5ish(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x100_0000_01b3)
    })
}

/// The differences between A and B that the blueprint does not document.
fn undocumented(a: &Path, b: &Path) -> Vec<String> {
    let (ta, tb) = (tree(a), tree(b));
    let mut keys: Vec<&String> = ta.keys().chain(tb.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| {
            let kind = match (ta.get(k), tb.get(k)) {
                (Some(x), Some(y)) if x == y => return None,
                (Some(_), Some(_)) => "differs",
                (Some(_), None) => "only in A",
                (None, Some(_)) => "only in B",
                (None, None) => unreachable!(),
            };
            (!EXPECTED.contains(&(k.as_str(), kind))).then(|| format!("{k}: {kind}"))
        })
        .collect()
}

struct Built {
    tarball: Vec<u8>,
    manifest: Vec<u8>,
    version: Version,
}

/// rein's own generator over rein's own stage, keyed to `key`.
fn build_artifact(checkout: &Path, work: &Path, key: &TestKey) -> Built {
    let (stage, out, pubkey) = (work.join("stage"), work.join("out"), work.join("test.pub"));
    fs::write(&pubkey, key.pub_file()).unwrap();
    run(
        clean(Command::new("bash").current_dir(checkout), work)
            .args(["install-rein.sh", "--stage"])
            .arg(&stage),
        "install-rein.sh --stage",
    );
    run(
        clean(Command::new("node").current_dir(checkout), work)
            .args([
                "--no-warnings",
                "src/shared/release-manifest.mjs",
                "manifest",
            ])
            .arg(&stage)
            .arg("--out")
            .arg(&out)
            .arg("--pubkey")
            .arg(&pubkey),
        "rein release manifest",
    );
    let manifest = fs::read(out.join("manifest.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    let version = Version::parse(doc["version"].as_str().unwrap()).unwrap();
    let tarball = fs::read(out.join(format!("rein-{version}.tar.gz"))).unwrap();
    Built {
        tarball,
        manifest,
        version,
    }
}

fn install_b(home: &Path, built: &Built, key: &TestKey) -> u8 {
    let v = &built.version;
    let p = |n: &str| format!("/v{v}/{n}");
    let routes = [
        (p(&format!("rein-{v}.tar.gz")), built.tarball.clone()),
        (
            p(&format!("rein-{v}.tar.gz.minisig")),
            key.sign(&built.tarball).into_bytes(),
        ),
        (p("manifest.json"), built.manifest.clone()),
    ]
    .into_iter()
    .map(|(k, b)| (k, Route::Body(b)))
    .collect();
    let http = ReqwestHttp::new(HostPolicy::loopback_only()).unwrap();
    let verifier = key.verifier();
    install(
        home,
        v,
        &Deps {
            http: &http,
            source: &ReleaseSource {
                base_url: serve(routes),
            },
            verifier: &verifier,
            process: &SystemProcess,
            run_uid: rein_release::InstallLock::run_uid,
        },
    )
}

fn clone_install(checkout: &Path, home: &Path) {
    run(
        clean(Command::new("bash").current_dir(checkout), home).arg("install-rein.sh"),
        "install-rein.sh",
    );
    rein_install(home);
}

fn rein_install(home: &Path) {
    run(
        clean(&mut Command::new(home.join(".rein/bin/rein")), home).arg("install"),
        "rein install (A)",
    );
}

#[test]
fn rein_app_installs_the_same_tree_as_reins_clone_install() {
    let Some(checkout) = std::env::var_os("REIN_CHECKOUT").map(PathBuf::from) else {
        eprintln!("layout equality skipped: set REIN_CHECKOUT to a rein checkout");
        return;
    };
    let work = tempfile::tempdir().unwrap();
    let (a, b, cwd) = (
        work.path().join("A"),
        work.path().join("B"),
        work.path().join("cwd"),
    );
    for d in [&a, &b, &cwd] {
        fs::create_dir_all(d).unwrap();
    }
    let key = TestKey::generate();
    let built = build_artifact(&checkout, work.path(), &key);

    clone_install(&checkout, &a);
    let code = install_b(&b, &built, &key);
    check(code, &a, &b, &cwd, "fresh install");

    // Second case: a retired server directory in both homes; `rein install` removes it.
    for home in [&a, &b] {
        let gone = home.join(".rein/mcp/gone-mcp/node_modules");
        fs::create_dir_all(&gone).unwrap();
        fs::write(gone.join("x"), "retired").unwrap();
    }
    rein_install(&a);
    let code = install_b(&b, &built, &key);
    check(code, &a, &b, &cwd, "retired server");
}
