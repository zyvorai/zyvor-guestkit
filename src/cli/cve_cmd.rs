// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! `guestkit cve`: vulnerability report with VEX suppression, EPSS/KEV
//! enrichment, OpenVEX/CSAF output, CI gating and air-gapped DB bundles.

use crate::cli::inventory::vuln_report as vr;
use crate::cli::inventory::{self, cve, Inventory};
use anyhow::{bail, Context, Result};
use clap::Args;
use std::path::{Path, PathBuf};

/// Exit code used when `--fail-on` / `--fail-on-kev` trips.
pub const EXIT_GATE_FAILED: i32 = 2;

#[derive(Args, Debug, Clone)]
pub struct CveArgs {
    /// Disk image, or an inventory JSON file from `guestkit inventory -f json`
    pub target: Option<PathBuf>,

    /// Output format: text, json, openvex, csaf
    #[arg(short = 'f', long, default_value = "text", value_parser = ["text", "json", "openvex", "csaf"])]
    pub format: String,

    /// Write the report to FILE (stdout if omitted)
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// OpenVEX document; `not_affected` and `fixed` statements suppress findings
    #[arg(long, value_name = "FILE")]
    pub vex: Option<PathBuf>,

    /// Locally cached EPSS feed (FIRST CSV or JSON); no network is used
    #[arg(long, value_name = "FILE")]
    pub epss: Option<PathBuf>,

    /// Locally cached CISA KEV catalog (JSON or one CVE id per line)
    #[arg(long, value_name = "FILE")]
    pub kev: Option<PathBuf>,

    /// Exit with code 2 when an active finding at or above SEVERITY exists
    /// (critical, high, medium, low)
    #[arg(long, value_name = "SEVERITY")]
    pub fail_on: Option<String>,

    /// Also exit with code 2 when an active finding is in the CISA KEV catalog
    #[arg(long)]
    pub fail_on_kev: bool,

    /// Never query the network (same as GUESTKIT_OFFLINE=1); use cache and imported bundles only
    #[arg(long)]
    pub offline: bool,

    /// Import an offline vulnerability DB bundle into the local cache, then exit
    #[arg(long, value_name = "FILE", conflicts_with = "db_export")]
    pub db_import: Option<PathBuf>,

    /// Export the local cache (OSV lookups, EPSS, KEV) as an offline bundle, then exit
    #[arg(long, value_name = "FILE")]
    pub db_export: Option<PathBuf>,
}

fn load_inventory(target: &Path) -> Result<Inventory> {
    let head = std::fs::File::open(target)
        .ok()
        .and_then(|mut f| {
            use std::io::Read;
            let mut b = [0u8; 1];
            f.read_exact(&mut b).ok().map(|_| b[0])
        })
        .unwrap_or(0);
    if head == b'{' {
        let text = std::fs::read_to_string(target)
            .with_context(|| format!("read inventory {}", target.display()))?;
        return serde_json::from_str(&text)
            .with_context(|| format!("{} is not a guestkit inventory JSON", target.display()));
    }
    inventory::generate_inventory(target, false, true, false)
}

/// Build the finding list (collect, enrich, apply VEX). Pure given its inputs.
pub fn build_findings(
    inv: &Inventory,
    enrichment: &vr::Enrichment,
    vex: &[vr::VexStatement],
) -> Vec<vr::Finding> {
    let mut findings = vr::collect_findings(inv);
    enrichment.apply(&mut findings);
    vr::apply_vex(&mut findings, vex);
    findings
}

/// Decide the process exit code for a set of findings.
pub fn gate_exit_code(
    findings: &[vr::Finding],
    fail_on: Option<&str>,
    fail_on_kev: bool,
) -> Result<i32> {
    let mut code = 0;
    if let Some(sev) = fail_on {
        if vr::count_at_or_above(findings, sev)? > 0 {
            code = EXIT_GATE_FAILED;
        }
    }
    if fail_on_kev && findings.iter().any(|f| f.is_active() && f.kev) {
        code = EXIT_GATE_FAILED;
    }
    Ok(code)
}

/// Render in the requested format.
pub fn render(findings: &[vr::Finding], format: &str, now: &str) -> Result<String> {
    Ok(match format {
        "text" => vr::to_text(findings),
        "json" => serde_json::to_string_pretty(findings)?,
        "openvex" => {
            serde_json::to_string_pretty(&vr::to_openvex(findings, "Zyvor AI Labs guestkit", now))?
        }
        "csaf" => serde_json::to_string_pretty(&vr::to_csaf(
            findings,
            "guestkit vulnerability report",
            now,
        ))?,
        other => bail!("unknown format `{}`", other),
    })
}

/// Run the command; returns the process exit code.
pub fn run(args: CveArgs) -> Result<i32> {
    if args.offline {
        std::env::set_var("GUESTKIT_OFFLINE", "1");
    }
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    if let Some(path) = &args.db_import {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read bundle {}", path.display()))?;
        let (e, p, k) = vr::import_bundle(&text, &cve::osv_cache_dir()?, &vr::feeds_dir())?;
        eprintln!(
            "Imported vulnerability DB bundle: {} package lookups, {} EPSS scores, {} KEV entries",
            e, p, k
        );
        return Ok(0);
    }
    if let Some(path) = &args.db_export {
        let b = vr::export_bundle(&cve::osv_cache_dir()?, &vr::feeds_dir(), &now)?;
        std::fs::write(path, serde_json::to_string_pretty(&b)?)
            .with_context(|| format!("write bundle {}", path.display()))?;
        eprintln!(
            "Exported vulnerability DB bundle to {} ({} package lookups, {} EPSS scores, {} KEV entries)",
            path.display(),
            b.payload.entries.len(),
            b.payload.epss.len(),
            b.payload.kev.len()
        );
        return Ok(0);
    }

    let target = args
        .target
        .as_deref()
        .context("a disk image or inventory JSON is required (or use --db-import / --db-export)")?;
    // Validate the threshold before doing any work.
    if let Some(sev) = &args.fail_on {
        if cve::severity_rank(sev) == 0 {
            bail!(
                "invalid --fail-on `{}` (expected critical, high, medium or low)",
                sev
            );
        }
    }

    let inv = load_inventory(target)?;
    let mut enrichment = vr::Enrichment::load(args.epss.as_deref(), args.kev.as_deref())?;
    enrichment.merge_missing(vr::Enrichment::from_cache_dir(&vr::feeds_dir()));
    let vex = match &args.vex {
        Some(p) => vr::load_vex(p)?,
        None => Vec::new(),
    };
    let findings = build_findings(&inv, &enrichment, &vex);

    let rendered = render(&findings, &args.format, &now)?;
    match &args.output {
        Some(p) => {
            std::fs::write(p, &rendered).with_context(|| format!("write report {}", p.display()))?
        }
        None => println!("{}", rendered),
    }

    let code = gate_exit_code(&findings, args.fail_on.as_deref(), args.fail_on_kev)?;
    if code != 0 {
        eprintln!("cve: gate failed (--fail-on / --fail-on-kev)");
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::inventory::{InventoryStatistics, PackageInfo, VulnerabilityInfo};
    use std::collections::HashMap;

    fn inv() -> Inventory {
        let pkg = |n: &str, v: &str, id: &str, sev: &str| PackageInfo {
            name: n.into(),
            version: v.into(),
            package_type: "rpm".into(),
            license: None,
            size: None,
            installed_date: None,
            files: vec![],
            dependencies: vec![],
            vulnerabilities: vec![VulnerabilityInfo::new(
                id.into(),
                sev.into(),
                None,
                "x".into(),
                None,
            )],
            checksum: None,
        };
        Inventory {
            image_path: "fixture.img".into(),
            scanned_at: "2026-01-01T00:00:00Z".into(),
            os_name: "t".into(),
            os_version: "1".into(),
            architecture: "x86_64".into(),
            packages: vec![
                pkg("alpha", "1.0", "CVE-2025-0001", "critical"),
                pkg("beta", "2.0", "CVE-2025-0002", "medium"),
            ],
            statistics: InventoryStatistics {
                total_packages: 2,
                total_size: 0,
                vulnerabilities: HashMap::new(),
                licenses: HashMap::new(),
            },
        }
    }

    #[test]
    fn gate_exit_codes() {
        let findings = build_findings(&inv(), &vr::Enrichment::default(), &[]);
        assert_eq!(
            gate_exit_code(&findings, Some("critical"), false).unwrap(),
            2
        );
        assert_eq!(gate_exit_code(&findings, None, false).unwrap(), 0);
        assert!(gate_exit_code(&findings, Some("nope"), false).is_err());

        let vex = vr::parse_openvex(
            r#"{"statements":[{"vulnerability":{"name":"CVE-2025-0001"},"status":"not_affected"}]}"#,
        )
        .unwrap();
        let findings = build_findings(&inv(), &vr::Enrichment::default(), &vex);
        assert_eq!(gate_exit_code(&findings, Some("high"), false).unwrap(), 0);
        assert_eq!(gate_exit_code(&findings, Some("medium"), false).unwrap(), 2);
    }

    #[test]
    fn kev_gate_and_enrichment() {
        let enr = vr::Enrichment {
            epss: vr::parse_epss("CVE-2025-0002,0.9,0.99\n").unwrap(),
            kev: vr::parse_kev("CVE-2025-0002\n").unwrap(),
        };
        let findings = build_findings(&inv(), &enr, &[]);
        assert_eq!(gate_exit_code(&findings, None, true).unwrap(), 2);
        let text = render(&findings, "text", "t").unwrap();
        assert!(text.contains("KEV") && text.contains("epss=0.900"));
    }

    #[test]
    fn render_formats() {
        let findings = build_findings(&inv(), &vr::Enrichment::default(), &[]);
        for f in ["text", "json", "openvex", "csaf"] {
            assert!(!render(&findings, f, "2026-01-01T00:00:00Z")
                .unwrap()
                .is_empty());
        }
        assert!(render(&findings, "xml", "t").is_err());
    }
}
