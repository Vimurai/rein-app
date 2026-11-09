//! Fixture releases for the verifier and fetcher tests (blueprint §Test Plan).
//!
//! Everything is signed with a throwaway TEST minisign key generated per test run — never
//! a production key, never rein's own fixture key (rein D-153 addendum r1).

#![allow(dead_code)] // each test binary uses a different subset

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use rein_release::{MinisignVerifier, ReleaseAssets};
use sha2::{Digest, Sha256};

pub const VERSION: &str = "0.5.0";

/// A throwaway minisign key pair.
pub struct TestKey {
    pk: minisign::PublicKey,
    sk: minisign::SecretKey,
}

impl TestKey {
    pub fn generate() -> Self {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        Self {
            pk: kp.pk,
            sk: kp.sk,
        }
    }

    pub fn key_id(&self) -> String {
        let id: [u8; 8] = self.pk.keynum().try_into().unwrap();
        format!("{:016X}", u64::from_le_bytes(id))
    }

    /// The production verifier, trusting this test key.
    pub fn verifier(&self) -> MinisignVerifier {
        MinisignVerifier::from_base64(&self.pk.to_base64()).unwrap()
    }

    /// The same signature relabelled as a legacy (non-prehashed, `Ed`) one.
    pub fn legacy_sign(&self, data: &[u8]) -> String {
        let sig = self.sign(data);
        let mut lines: Vec<String> = sig.lines().map(str::to_owned).collect();
        let mut raw = b64_decode(&lines[1]);
        assert_eq!(b64_encode(&raw), lines[1], "base64 round trip");
        assert_eq!(&raw[..2], b"ED", "minisign writes prehashed signatures");
        raw[1] = b'd'; // "Ed": the legacy algorithm id
        lines[1] = b64_encode(&raw);
        lines.join("\n") + "\n"
    }

    pub fn sign(&self, data: &[u8]) -> String {
        minisign::sign(Some(&self.pk), &self.sk, data, None, None)
            .unwrap()
            .into_string()
    }
}

/// One tarball entry. Paths are written raw, so unsafe names can be produced.
pub enum Entry {
    File(&'static str, Vec<u8>),
    Dir(&'static str),
    DirWithData(&'static str, Vec<u8>),
    Symlink(&'static str, &'static str),
    HardLink(&'static str, &'static str),
    /// A pax extended header (typeflag `x`) carrying `data`.
    Pax(&'static str, Vec<u8>),
    /// A member with this raw typeflag byte (e.g. NUL, GNU `L`).
    Typed(&'static str, Vec<u8>, u8),
    /// Two zero blocks (end-of-archive) written mid-stream; later entries follow them.
    EndMarker,
}

/// A file as the manifest lists it.
#[derive(Clone)]
pub struct Listed {
    pub path: &'static str,
    pub bytes: Vec<u8>,
    pub mode: &'static str,
}

pub fn good_files() -> Vec<Listed> {
    let f = |path, bytes: &[u8], mode| Listed {
        path,
        bytes: bytes.to_vec(),
        mode,
    };
    vec![
        f("bin/rein", b"#!/bin/sh\necho rein\n", "0755"),
        f("hooks/pre-tool-use.sh", b"#!/bin/sh\nexit 0\n", "0755"),
        f("shared/locate.sh", b"rein_locate() { :; }\n", "0644"),
        f("config/defaults.json", b"{\"a\":1}\n", "0644"),
        // Shipped AND matched by `mcp/*/package-lock.json`: a shipped path wins (D-155 r2).
        f(
            "mcp/orchestrator-mcp/package-lock.json",
            b"{\"lock\":\"new\"}\n",
            "0644",
        ),
    ]
}

pub fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// A schema-1 manifest for `files`.
pub fn manifest_json(key_id: &str, files: &[Listed]) -> String {
    let rows: Vec<String> = files
        .iter()
        .map(|f| {
            format!(
                r#"{{"path":"{}","sha256":"{}","size":{},"mode":"{}"}}"#,
                f.path,
                sha256_hex(&f.bytes),
                f.bytes.len(),
                f.mode
            )
        })
        .collect();
    format!(
        r#"{{"schema":1,"version":"{VERSION}","min_node":"22.5.0","key_id":"{key_id}",
"layout":{{"mirrored":["bin","hooks","mcp","shared","claude","contracts","templates","copilot","scripts","config"],
"operator_owned":["accounts.json","secrets/","run/","config/rein-workspace.txt","mcp/*/node_modules/","mcp/*/package-lock.json","shared/toolchain.sh"]}},
"files":[{}]}}"#,
        rows.join(",")
    )
}

/// Entries for `files` plus `manifest.json`.
pub fn entries_for(files: &[Listed], manifest: &str) -> Vec<Entry> {
    let mut e: Vec<Entry> = vec![Entry::File("manifest.json", manifest.as_bytes().to_vec())];
    e.extend(files.iter().map(|f| Entry::File(f.path, f.bytes.clone())));
    e
}

/// gzip'd tar of `entries`, names written raw into the header (no builder validation).
pub fn tarball(entries: &[Entry]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for entry in entries {
        if matches!(entry, Entry::EndMarker) {
            builder.get_mut().write_all(&[0u8; 1024]).unwrap();
            continue;
        }
        let mut h = tar::Header::new_ustar(); // as rein's writer emits (D-155 r1)
        let (name, kind, data): (&str, tar::EntryType, &[u8]) = match entry {
            Entry::File(p, b) => (p, tar::EntryType::Regular, b),
            Entry::Dir(p) => (p, tar::EntryType::Directory, &[]),
            Entry::DirWithData(p, b) => (p, tar::EntryType::Directory, b),
            Entry::Symlink(p, target) => {
                h.set_link_name(target).unwrap();
                (p, tar::EntryType::Symlink, &[])
            }
            Entry::HardLink(p, target) => {
                h.set_link_name(target).unwrap();
                (p, tar::EntryType::Link, &[])
            }
            Entry::Pax(p, b) => (p, tar::EntryType::XHeader, b),
            Entry::Typed(p, b, flag) => (p, tar::EntryType::new(*flag), b),
            Entry::EndMarker => unreachable!(),
        };
        let field = &mut h.as_old_mut().name;
        field[..name.len()].copy_from_slice(name.as_bytes());
        h.set_entry_type(kind);
        if let Entry::Typed(_, _, flag) = entry {
            h.as_mut_bytes()[156] = *flag; // raw: set_entry_type normalises NUL to '0'
        }
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        builder.append(&h, data).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// The three asset bodies.
pub struct Release {
    pub tarball: Vec<u8>,
    pub signature: String,
    pub manifest: String,
}

impl Release {
    /// A correctly built and signed release.
    pub fn good(key: &TestKey) -> Self {
        let files = good_files();
        let manifest = manifest_json(&key.key_id(), &files);
        Self::signed(key, &entries_for(&files, &manifest), manifest)
    }

    /// Sign whatever tarball `entries` produce, with `manifest` beside it.
    pub fn signed(key: &TestKey, entries: &[Entry], manifest: String) -> Self {
        let tarball = tarball(entries);
        let signature = key.sign(&tarball);
        Self {
            tarball,
            signature,
            manifest,
        }
    }

    pub fn write_to(&self, dir: &Path) -> ReleaseAssets {
        let a = ReleaseAssets {
            tarball: dir.join(format!("rein-{VERSION}.tar.gz")),
            signature: dir.join(format!("rein-{VERSION}.tar.gz.minisig")),
            manifest: dir.join("manifest.json"),
        };
        std::fs::write(&a.tarball, &self.tarball).unwrap();
        std::fs::write(&a.signature, &self.signature).unwrap();
        std::fs::write(&a.manifest, &self.manifest).unwrap();
        a
    }

    /// Routes for the fixture server, under `/v<VERSION>/`.
    pub fn routes(&self) -> HashMap<String, Route> {
        let p = |n: &str| format!("/v{VERSION}/{n}");
        HashMap::from([
            (
                p(&format!("rein-{VERSION}.tar.gz")),
                Route::Body(self.tarball.clone()),
            ),
            (
                p(&format!("rein-{VERSION}.tar.gz.minisig")),
                Route::Body(self.signature.clone().into_bytes()),
            ),
            (
                p("manifest.json"),
                Route::Body(self.manifest.clone().into_bytes()),
            ),
        ])
    }
}

/// What the fixture server answers for a path.
#[derive(Clone)]
pub enum Route {
    Body(Vec<u8>),
    Redirect(String),
}

/// Start a local HTTP/1.1 server on 127.0.0.1; returns its base URL. Unknown paths → 404.
/// The thread lives until the test process exits.
pub fn serve(routes: HashMap<String, Route>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok() && line.trim_end() != "" {
                line.clear();
            }
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_owned();
            let response = match routes.get(&path) {
                Some(Route::Body(b)) => {
                    let mut r = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        b.len()
                    )
                    .into_bytes();
                    r.extend_from_slice(b);
                    r
                }
                Some(Route::Redirect(to)) => format!(
                    "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .into_bytes(),
                None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            };
            let _ = stream.write_all(&response);
        }
    });
    base
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_decode(s: &str) -> Vec<u8> {
    let val = |c: u8| B64.iter().position(|&b| b == c).unwrap() as u32;
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c) << (18 - 6 * i);
        }
        for i in 0..chunk.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    out
}

fn b64_encode(b: &[u8]) -> String {
    let mut out = String::new();
    for chunk in b.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &c)| n | (c as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
        out.extend(std::iter::repeat_n('=', 3 - chunk.len()));
    }
    out
}
