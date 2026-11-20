// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! `guestkit attest create|verify`: signed in-toto/DSSE SBOM attestations.

use crate::assurance::attest as at;
use crate::cli::inventory;
use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum AttestCommand {
    /// Create a signed in-toto/DSSE attestation of an image's SBOM
    Create(CreateArgs),
    /// Verify an attestation (signature, payload, optional image digest)
    Verify(VerifyArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PredicateFormat {
    Cyclonedx,
    Spdx,
}

#[derive(Args, Debug, Clone)]
pub struct CreateArgs {
    /// Image file; its SHA-256 becomes the statement subject
    pub image: PathBuf,

    /// Existing CycloneDX or SPDX JSON to attest (otherwise generated from the image)
    #[arg(long, value_name = "FILE")]
    pub sbom: Option<PathBuf>,

    /// Predicate format when generating the SBOM from the image
    #[arg(long, value_enum, default_value = "cyclonedx")]
    pub predicate: PredicateFormat,

    /// Ed25519 signing key (32-byte seed file or 64 hex chars; same as `passport keygen`)
    #[arg(long, value_name = "FILE")]
    pub sign_key: PathBuf,

    /// Subject name (default: the image file name)
    #[arg(long)]
    pub subject_name: Option<String>,

    /// Write the DSSE envelope to FILE (stdout if omitted)
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct VerifyArgs {
    /// DSSE envelope JSON produced by `attest create`
    pub envelope: PathBuf,

    /// Image to compare against the statement subject digest
    #[arg(long, value_name = "FILE")]
    pub image: Option<PathBuf>,

    /// Expected Ed25519 public key (64 hex chars)
    #[arg(long, value_name = "HEX")]
    pub public_key: Option<String>,

    /// Allowlist file: one Ed25519 public key (hex) per line
    #[arg(long, value_name = "FILE")]
    pub trust_keys: Option<PathBuf>,
}

pub fn run(cmd: AttestCommand) -> Result<()> {
    match cmd {
        AttestCommand::Create(a) => create(a),
        AttestCommand::Verify(a) => verify(a),
    }
}

fn create(a: CreateArgs) -> Result<()> {
    let digest = at::sha256_file(&a.image)?;
    let predicate: serde_json::Value = match &a.sbom {
        Some(p) => serde_json::from_str(
            &std::fs::read_to_string(p).with_context(|| format!("read SBOM {}", p.display()))?,
        )
        .with_context(|| format!("{} is not JSON", p.display()))?,
        None => {
            let inv = inventory::generate_inventory(&a.image, true, false, false)?;
            match a.predicate {
                PredicateFormat::Cyclonedx => {
                    serde_json::to_value(inventory::formats::to_cyclonedx(&inv)?)?
                }
                PredicateFormat::Spdx => serde_json::to_value(inventory::formats::to_spdx(&inv)?)?,
            }
        }
    };
    let ptype = at::detect_predicate_type(&predicate)?;
    let name = a.subject_name.clone().unwrap_or_else(|| {
        a.image
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".into())
    });
    let statement = at::build_statement(&name, &digest, ptype, predicate);
    let env = at::sign_statement(&statement, &a.sign_key)?;
    let text = serde_json::to_string_pretty(&env)?;
    match &a.output {
        Some(p) => {
            std::fs::write(p, &text).with_context(|| format!("write {}", p.display()))?;
            eprintln!(
                "Attestation written to {} (subject sha256:{}, key {})",
                p.display(),
                digest,
                env.signatures[0].keyid
            );
        }
        None => println!("{}", text),
    }
    Ok(())
}

fn verify(a: VerifyArgs) -> Result<()> {
    let text = std::fs::read_to_string(&a.envelope)
        .with_context(|| format!("read {}", a.envelope.display()))?;
    let trust_keys = match &a.trust_keys {
        Some(p) => Some(crate::assurance::load_trust_keys(p)?),
        None => None,
    };
    let expect_sha256 = match &a.image {
        Some(p) => Some(at::sha256_file(p)?),
        None => None,
    };
    let rep = at::verify_envelope(
        &text,
        &at::VerifyOptions {
            public_key: a.public_key.clone(),
            trust_keys,
            expect_sha256,
        },
    )?;
    println!("Attestation OK");
    println!("  signer:    {}", rep.keyid);
    println!("  predicate: {}", rep.predicate_type);
    for (n, d) in &rep.subjects {
        println!("  subject:   {} sha256:{}", n, d);
    }
    if a.image.is_some() {
        println!("  image digest matches a subject");
    }
    Ok(())
}
