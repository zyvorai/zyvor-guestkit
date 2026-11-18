// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! in-toto attestations for an image's SBOM / inventory, wrapped in a DSSE
//! envelope and signed with the same Ed25519 key format as Cutover Passports.
//!
//! * Statement: in-toto Statement v1; subject is the image file's SHA-256
//!   (SLSA-style `digest.sha256`); predicate is a CycloneDX or SPDX document.
//! * Envelope: DSSE v1 (`payloadType` `application/vnd.in-toto+json`), PAE
//!   signed with Ed25519. `keyid` is the lowercase hex public key.
//!
//! Signing and signature verification need `--features agent` (Ed25519), as
//! for passports. No Sigstore / keyless / transparency-log support: cosign
//! interoperability (ECDSA-P256 or cosign key formats, Rekor) is a follow-up.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

pub const PAYLOAD_TYPE: &str = "application/vnd.in-toto+json";
pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
pub const PREDICATE_CYCLONEDX: &str = "https://cyclonedx.org/bom";
pub const PREDICATE_SPDX: &str = "https://spdx.dev/Document";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DsseSignature {
    pub keyid: String,
    pub sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DsseEnvelope {
    #[serde(rename = "payloadType")]
    pub payload_type: String,
    pub payload: String,
    pub signatures: Vec<DsseSignature>,
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 || !s.is_ascii() {
        bail!("invalid hex string");
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).context("invalid hex digit"))
        .collect()
}

/// SHA-256 of a file (streamed), lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f =
        std::fs::File::open(path).with_context(|| format!("open image {}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(to_hex(&h.finalize()))
}

/// Predicate type URI for a CycloneDX or SPDX JSON document.
pub fn detect_predicate_type(sbom: &Value) -> Result<&'static str> {
    if sbom.get("bomFormat").and_then(|v| v.as_str()) == Some("CycloneDX") {
        Ok(PREDICATE_CYCLONEDX)
    } else if sbom.get("spdxVersion").is_some() {
        Ok(PREDICATE_SPDX)
    } else {
        bail!("predicate must be a CycloneDX (bomFormat) or SPDX (spdxVersion) JSON document")
    }
}

/// Build an in-toto Statement v1 with a single sha256 subject.
pub fn build_statement(
    subject_name: &str,
    sha256_hex: &str,
    predicate_type: &str,
    predicate: Value,
) -> Value {
    json!({
        "_type": STATEMENT_TYPE,
        "subject": [{"name": subject_name, "digest": {"sha256": sha256_hex}}],
        "predicateType": predicate_type,
        "predicate": predicate,
    })
}

/// DSSE pre-authentication encoding.
pub fn pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let mut v = format!(
        "DSSEv1 {} {} {} ",
        payload_type.len(),
        payload_type,
        payload.len()
    )
    .into_bytes();
    v.extend_from_slice(payload);
    v
}

/// What to check when verifying.
#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// Ed25519 public key (hex) the signature must verify under.
    pub public_key: Option<String>,
    /// Allowlist of public keys (hex).
    pub trust_keys: Option<Vec<String>>,
    /// Require a subject with this sha256.
    pub expect_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub keyid: String,
    pub predicate_type: String,
    pub subjects: Vec<(String, String)>,
}

/// Structural checks on a decoded statement; returns (predicate type, subjects).
pub fn check_statement(
    statement: &Value,
    expect_sha256: Option<&str>,
) -> Result<(String, Vec<(String, String)>)> {
    if statement.get("_type").and_then(|v| v.as_str()) != Some(STATEMENT_TYPE) {
        bail!("payload is not an in-toto Statement v1");
    }
    let predicate_type = statement
        .get("predicateType")
        .and_then(|v| v.as_str())
        .context("statement has no predicateType")?
        .to_string();
    if statement.get("predicate").is_none() {
        bail!("statement has no predicate");
    }
    let mut subjects = Vec::new();
    for s in statement
        .get("subject")
        .and_then(|v| v.as_array())
        .context("statement has no subject")?
    {
        let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("").into();
        let d = s
            .get("digest")
            .and_then(|d| d.get("sha256"))
            .and_then(|d| d.as_str())
            .context("subject lacks digest.sha256")?
            .to_ascii_lowercase();
        subjects.push((name, d));
    }
    if subjects.is_empty() {
        bail!("statement has no subjects");
    }
    if let Some(want) = expect_sha256 {
        let want = want.to_ascii_lowercase();
        if !subjects.iter().any(|(_, d)| *d == want) {
            bail!(
                "no subject has sha256 {} (image does not match the attestation)",
                want
            );
        }
    }
    Ok((predicate_type, subjects))
}

#[cfg(feature = "agent")]
fn load_seed(path: &Path) -> Result<[u8; 32]> {
    let raw = std::fs::read(path).with_context(|| format!("read sign key {}", path.display()))?;
    if raw.len() == 32 {
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&raw);
        return Ok(seed);
    }
    let decoded = from_hex(String::from_utf8_lossy(&raw).trim())
        .context("sign key must be 32 raw bytes or 64 hex chars")?;
    decoded
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("sign key hex must decode to 32 bytes"))
}

/// Sign a statement with the Ed25519 seed at `key_path` (same format as
/// `passport keygen` / `--sign-key`).
#[cfg(feature = "agent")]
pub fn sign_statement(statement: &Value, key_path: &Path) -> Result<DsseEnvelope> {
    use ed25519_dalek::{Signer, SigningKey};
    let sk = SigningKey::from_bytes(&load_seed(key_path)?);
    let body = serde_json::to_vec(statement)?;
    let sig = sk.sign(&pae(PAYLOAD_TYPE, &body));
    Ok(DsseEnvelope {
        payload_type: PAYLOAD_TYPE.to_string(),
        payload: B64.encode(&body),
        signatures: vec![DsseSignature {
            keyid: to_hex(sk.verifying_key().as_bytes()),
            sig: B64.encode(sig.to_bytes()),
        }],
    })
}

#[cfg(not(feature = "agent"))]
pub fn sign_statement(_statement: &Value, _key_path: &Path) -> Result<DsseEnvelope> {
    bail!("attestation signing requires guestkit built with --features agent (Ed25519)")
}

/// Verify a DSSE envelope: signature under an explicitly trusted key, payload
/// type, in-toto structure, and (optionally) the subject digest.
#[cfg(feature = "agent")]
pub fn verify_envelope(envelope_json: &str, opts: &VerifyOptions) -> Result<VerifyReport> {
    use ed25519_dalek::{Signature, VerifyingKey};

    if opts.public_key.is_none() && opts.trust_keys.is_none() {
        bail!("provide --public-key or --trust-keys: refusing to trust the key embedded in the envelope");
    }
    let env: DsseEnvelope = serde_json::from_str(envelope_json).context("parse DSSE envelope")?;
    if env.payload_type != PAYLOAD_TYPE {
        bail!("unexpected payloadType `{}`", env.payload_type);
    }
    let body = B64.decode(env.payload.trim()).context("decode payload")?;
    let msg = pae(&env.payload_type, &body);

    let allowed = |k: &str| -> bool {
        let k = k.to_ascii_lowercase();
        let pk_ok = opts
            .public_key
            .as_ref()
            .map(|p| p.eq_ignore_ascii_case(&k))
            .unwrap_or(true);
        let tk_ok = opts
            .trust_keys
            .as_ref()
            .map(|t| t.iter().any(|x| x.eq_ignore_ascii_case(&k)))
            .unwrap_or(true);
        pk_ok && tk_ok
    };

    let mut verified_key: Option<String> = None;
    for s in &env.signatures {
        if !allowed(&s.keyid) {
            continue;
        }
        let Ok(pk_bytes) = from_hex(&s.keyid) else {
            continue;
        };
        let Ok(pk_arr) = <[u8; 32]>::try_from(pk_bytes.as_slice()) else {
            continue;
        };
        let Ok(vk) = VerifyingKey::from_bytes(&pk_arr) else {
            continue;
        };
        let Ok(sig_bytes) = B64.decode(s.sig.trim()) else {
            continue;
        };
        let Ok(sig_arr) = <[u8; 64]>::try_from(sig_bytes.as_slice()) else {
            continue;
        };
        if vk
            .verify_strict(&msg, &Signature::from_bytes(&sig_arr))
            .is_ok()
        {
            verified_key = Some(s.keyid.to_ascii_lowercase());
            break;
        }
    }
    let keyid = verified_key
        .context("no valid signature from a trusted key (signature invalid or key not trusted)")?;

    let statement: Value = serde_json::from_slice(&body).context("payload is not JSON")?;
    let (predicate_type, subjects) = check_statement(&statement, opts.expect_sha256.as_deref())?;
    Ok(VerifyReport {
        keyid,
        predicate_type,
        subjects,
    })
}

#[cfg(not(feature = "agent"))]
pub fn verify_envelope(_envelope_json: &str, _opts: &VerifyOptions) -> Result<VerifyReport> {
    bail!("attestation verification requires guestkit built with --features agent (Ed25519)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pae_known_vector() {
        // From the DSSE specification.
        assert_eq!(
            pae("http://example.com/HelloWorld", b"hello world"),
            b"DSSEv1 29 http://example.com/HelloWorld 11 hello world".to_vec()
        );
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(to_hex(&[0, 255, 16]), "00ff10");
        assert_eq!(from_hex("00ff10").unwrap(), vec![0, 255, 16]);
        assert!(from_hex("0").is_err());
        assert!(from_hex("zz").is_err());
    }

    #[test]
    fn sha256_of_file() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("img");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn predicate_detection() {
        assert_eq!(
            detect_predicate_type(&json!({"bomFormat":"CycloneDX"})).unwrap(),
            PREDICATE_CYCLONEDX
        );
        assert_eq!(
            detect_predicate_type(&json!({"spdxVersion":"SPDX-2.3"})).unwrap(),
            PREDICATE_SPDX
        );
        assert!(detect_predicate_type(&json!({"x":1})).is_err());
    }

    #[test]
    fn statement_checks() {
        let st = build_statement(
            "img",
            &"a".repeat(64),
            PREDICATE_SPDX,
            json!({"spdxVersion":"x"}),
        );
        assert!(check_statement(&st, Some(&"A".repeat(64))).is_ok());
        assert!(check_statement(&st, Some(&"b".repeat(64))).is_err());
        assert!(check_statement(&json!({"_type":"other"}), None).is_err());
    }

    #[cfg(feature = "agent")]
    mod signing {
        use super::*;

        /// RFC 8032 test vector 1 seed and public key.
        const SEED_HEX: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        const PUB_HEX: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

        fn key_file(dir: &Path, name: &str, seed: &[u8; 32]) -> std::path::PathBuf {
            let p = dir.join(name);
            std::fs::write(&p, to_hex(seed)).unwrap();
            p
        }

        fn fixture() -> (tempfile::TempDir, std::path::PathBuf, Value) {
            let td = tempfile::tempdir().unwrap();
            let seed: [u8; 32] = from_hex(SEED_HEX).unwrap().try_into().unwrap();
            let key = key_file(td.path(), "attest.key", &seed);
            let st = build_statement(
                "disk.img",
                &"c".repeat(64),
                PREDICATE_CYCLONEDX,
                json!({"bomFormat":"CycloneDX","components":[]}),
            );
            (td, key, st)
        }

        fn opts(pk: &str) -> VerifyOptions {
            VerifyOptions {
                public_key: Some(pk.into()),
                ..Default::default()
            }
        }

        #[test]
        fn sign_and_verify_roundtrip() {
            let (_td, key, st) = fixture();
            let env = sign_statement(&st, &key).unwrap();
            assert_eq!(env.signatures[0].keyid, PUB_HEX);
            let json = serde_json::to_string(&env).unwrap();
            let rep = verify_envelope(&json, &opts(PUB_HEX)).unwrap();
            assert_eq!(rep.predicate_type, PREDICATE_CYCLONEDX);
            assert_eq!(rep.subjects[0].1, "c".repeat(64));
            let mut o = opts(PUB_HEX);
            o.expect_sha256 = Some("c".repeat(64));
            assert!(verify_envelope(&json, &o).is_ok());
            o.expect_sha256 = Some("d".repeat(64));
            assert!(verify_envelope(&json, &o).is_err());
        }

        #[test]
        fn raw_32_byte_seed_file_works() {
            let (td, _key, st) = fixture();
            let p = td.path().join("raw.key");
            std::fs::write(&p, from_hex(SEED_HEX).unwrap()).unwrap();
            let env = sign_statement(&st, &p).unwrap();
            assert_eq!(env.signatures[0].keyid, PUB_HEX);
        }

        #[test]
        fn tampered_payload_fails() {
            let (_td, key, st) = fixture();
            let mut env = sign_statement(&st, &key).unwrap();
            let mut st2 = st.clone();
            st2["subject"][0]["digest"]["sha256"] = json!("e".repeat(64));
            env.payload = B64.encode(serde_json::to_vec(&st2).unwrap());
            let json = serde_json::to_string(&env).unwrap();
            assert!(verify_envelope(&json, &opts(PUB_HEX)).is_err());
        }

        #[test]
        fn wrong_key_and_untrusted_key_fail() {
            let (td, key, st) = fixture();
            let json = serde_json::to_string(&sign_statement(&st, &key).unwrap()).unwrap();
            let other = key_file(td.path(), "other.key", &[9u8; 32]);
            let other_pub = sign_statement(&st, &other).unwrap().signatures[0]
                .keyid
                .clone();
            assert!(verify_envelope(&json, &opts(&other_pub)).is_err());
            let tk = VerifyOptions {
                trust_keys: Some(vec![other_pub]),
                ..Default::default()
            };
            assert!(verify_envelope(&json, &tk).is_err());
            let tk_ok = VerifyOptions {
                trust_keys: Some(vec![PUB_HEX.to_uppercase()]),
                ..Default::default()
            };
            assert!(verify_envelope(&json, &tk_ok).is_ok());
        }

        #[test]
        fn requires_explicit_trust_and_right_payload_type() {
            let (_td, key, st) = fixture();
            let mut env = sign_statement(&st, &key).unwrap();
            let json = serde_json::to_string(&env).unwrap();
            assert!(verify_envelope(&json, &VerifyOptions::default()).is_err());
            env.payload_type = "text/plain".into();
            let json = serde_json::to_string(&env).unwrap();
            assert!(verify_envelope(&json, &opts(PUB_HEX)).is_err());
        }
    }
}
