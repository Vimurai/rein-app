//! The `SignatureVerifier` seam (blueprint §Components 2, rein D-153 r1).
//!
//! Trust root: the minisign public key committed at `docs/contract/release-key.pub` — a
//! byte-for-byte copy of rein's file of the same path (D-014 decision 4, E-16) — compiled
//! into the binary. The app never fetches a key. A committed file that does not parse, or a
//! build with the `reject-all-signatures` feature (the kill switch), trusts nothing and
//! every install fails closed.

use std::io::Read;

use minisign_verify::{PublicKey, Signature};

/// The release public key file as committed (`docs/contract/release-key.pub`), baked in.
pub const BAKED_RELEASE_KEY: &str = include_str!("../../../docs/contract/release-key.pub");

/// Upper bound on a `.minisig` file.
pub const MAX_SIGNATURE_BYTES: u64 = 64 * 1024;

/// Verifies a detached signature over a byte stream with one trusted key.
pub trait SignatureVerifier {
    /// The trusted key's id (upper-case hex, as minisign prints it), or why there is none.
    fn key_id(&self) -> Result<String, String>;
    /// Verify `signature` (the `.minisig` text) over everything readable from `data`.
    fn verify(&self, data: &mut dyn Read, signature: &str) -> Result<(), String>;
}

/// Production backend: minisign with one baked public key.
pub struct MinisignVerifier {
    key: PublicKey,
    key_id: String,
}

impl MinisignVerifier {
    /// Build from the text of a `minisign.pub` file, strictly: exactly two lines (one
    /// trailing newline allowed) — `untrusted comment: minisign public key[:] <ID>` and the
    /// base64 key line — where `<ID>` must equal the key id inside the key.
    pub fn from_pub_file(text: &str) -> Result<Self, String> {
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines: Vec<&str> = body.split('\n').collect();
        let [comment, key] = lines[..] else {
            return Err(format!(
                "public key file: expected 2 lines, found {}",
                lines.len()
            ));
        };
        // The minisign CLI writes `…public key <ID>`, the minisign crate `…public key: <ID>`.
        let id = comment
            .strip_prefix("untrusted comment: minisign public key")
            .map(|rest| rest.strip_prefix(':').unwrap_or(rest))
            .and_then(|rest| rest.strip_prefix(' '))
            .ok_or("public key file: first line is not a minisign public key comment")?;
        if key.trim() != key {
            return Err("public key file: whitespace around the key line".into());
        }
        let verifier = Self::from_base64(key)?;
        if id != verifier.key_id {
            return Err(format!(
                "public key file: comment names key {id}, the key is {}",
                verifier.key_id
            ));
        }
        Ok(verifier)
    }

    /// Build from the base64 key line of a `minisign.pub` file.
    pub fn from_base64(public_key_b64: &str) -> Result<Self, String> {
        let b64 = public_key_b64.trim();
        let key = PublicKey::from_base64(b64).map_err(|e| format!("public key: {e}"))?;
        let raw = decode_base64(b64).ok_or("public key: not base64")?;
        // minisign key layout: 2-byte algorithm, 8-byte key id (little-endian), 32-byte key.
        let id_bytes: [u8; 8] = raw
            .get(2..10)
            .and_then(|s| s.try_into().ok())
            .ok_or("public key: too short")?;
        let key_id = format!("{:016X}", u64::from_le_bytes(id_bytes));
        Ok(Self { key, key_id })
    }
}

impl SignatureVerifier for MinisignVerifier {
    fn key_id(&self) -> Result<String, String> {
        Ok(self.key_id.clone())
    }

    fn verify(&self, data: &mut dyn Read, signature: &str) -> Result<(), String> {
        let sig = Signature::decode(signature).map_err(|e| format!("signature file: {e}"))?;
        let mut stream = self.key.verify_stream(&sig).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = data
                .read(&mut buf)
                .map_err(|e| format!("reading tarball: {e}"))?;
            if n == 0 {
                break;
            }
            stream.update(&buf[..n]);
        }
        stream.finalize().map_err(|e| e.to_string())
    }
}

/// Kill-switch backend: trusts no key, so every signature is invalid.
pub struct Rejecting {
    reason: String,
}

impl Rejecting {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl SignatureVerifier for Rejecting {
    fn key_id(&self) -> Result<String, String> {
        Err(self.reason.clone())
    }

    fn verify(&self, _data: &mut dyn Read, _signature: &str) -> Result<(), String> {
        Err(self.reason.clone())
    }
}

/// The verifier this build ships with.
pub fn production_verifier() -> Box<dyn SignatureVerifier> {
    if cfg!(feature = "reject-all-signatures") {
        return Box::new(Rejecting::new(
            "built with the reject-all-signatures kill switch",
        ));
    }
    match MinisignVerifier::from_pub_file(BAKED_RELEASE_KEY) {
        Ok(v) => Box::new(v),
        Err(e) => Box::new(Rejecting::new(format!(
            "baked docs/contract/release-key.pub is invalid: {e}"
        ))),
    }
}

/// Minimal standard-alphabet base64 decoder (padding optional). Used only to read the
/// key id out of a public key that `minisign-verify` has already accepted.
fn decode_base64(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    }
    let bytes = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut acc = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            acc |= val(c)? << (18 - 6 * i);
        }
        let take = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => return None,
        };
        out.extend_from_slice(&acc.to_be_bytes()[1..=take]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_known_vectors() {
        assert_eq!(decode_base64("TWFu").unwrap(), b"Man");
        assert_eq!(decode_base64("TWE=").unwrap(), b"Ma");
        assert_eq!(decode_base64("TQ==").unwrap(), b"M");
        assert!(decode_base64("T").is_none());
        assert!(decode_base64("T*==").is_none());
    }

    #[test]
    fn rejecting_backend_trusts_nothing() {
        let r = Rejecting::new("kill switch");
        assert_eq!(r.key_id().unwrap_err(), "kill switch");
        assert!(r.verify(&mut &b"data"[..], "sig").is_err());
    }

    /// Replaces the env-var form of this test (E-16): the committed file must parse, so a
    /// malformed key fails `cargo test` (CI) before any release is built.
    #[test]
    fn production_verifier_trusts_the_committed_key_file() {
        let baked = production_verifier().key_id();
        if cfg!(feature = "reject-all-signatures") {
            assert!(baked.is_err());
            return;
        }
        let id = BAKED_RELEASE_KEY
            .lines()
            .next()
            .and_then(|l| l.rsplit(' ').next())
            .unwrap();
        assert_eq!(baked.unwrap(), id.to_ascii_uppercase());
    }

    #[test]
    fn pub_file_parser_refuses_anything_but_the_minisign_format() {
        let (comment, key) = BAKED_RELEASE_KEY.trim_end().split_once('\n').unwrap();
        assert!(MinisignVerifier::from_pub_file(BAKED_RELEASE_KEY).is_ok());
        assert!(MinisignVerifier::from_pub_file(&format!("{comment}\n{key}")).is_ok());
        let colon = comment.replacen("public key ", "public key: ", 1);
        assert!(MinisignVerifier::from_pub_file(&format!("{colon}\n{key}\n")).is_ok());
        for bad in [
            String::new(),
            key.to_string(),
            format!("{comment}\n{key}\n\n"),
            format!("{comment}\n{key}\nextra"),
            format!("{comment}\r\n{key}\n"),
            format!("{comment}\n {key}\n"),
            format!("untrusted comment: something else\n{key}\n"),
            format!(
                "{}\n{key}\n",
                comment.replacen("public key ", "public key", 1)
            ),
            format!(
                "{}\n{key}\n",
                comment.replacen("public key ", "public key  ", 1)
            ),
            format!("untrusted comment: minisign public key 0000000000000000\n{key}\n"),
            format!("{comment}\n{}\n", &key[..key.len() - 4]),
            format!("{comment}\nnot*base64\n"),
            format!("{}\n{key}\n", comment.to_ascii_lowercase()), // key id in lower case
        ] {
            assert!(MinisignVerifier::from_pub_file(&bad).is_err(), "{bad:?}");
        }
    }
}
