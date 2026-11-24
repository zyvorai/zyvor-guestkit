// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! `guestkit image-audit` and `guestkit image-audit-diff`: offline hardening
//! and boot-readiness audit of a disk image, and a machine-readable diff of
//! two such audits. Read-only: the image is opened and mounted read-only.

use super::{init_guestfs_ro, mount_all_ro};
use crate::offline::{
    apply_baseline, diff_reports, run_audit, to_sarif, AuditDiff, AuditReport, AuditScope,
    AuditVerdict, GuestRoot, KnownFilesystem, RuleSet, Severity, WaiverSet, AUDIT_SCHEMA,
};
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;

/// Inventory of the image's filesystems (UUID/label) for fstab/root= resolution.
fn known_filesystems(g: &mut crate::Guestfs, root_dev: &str) -> Vec<KnownFilesystem> {
    let mut devices: Vec<String> = Vec::new();
    if let Ok(fs) = g.list_filesystems() {
        devices.extend(fs.keys().cloned());
    }
    if let Ok(mp) = g.inspect_get_mountpoints(root_dev) {
        devices.extend(mp.values().cloned());
    }
    devices.sort();
    devices.dedup();
    devices
        .into_iter()
        .map(|d| {
            let uuid = g.vfs_uuid(&d).ok().filter(|s| !s.is_empty());
            let label = g.vfs_label(&d).ok().filter(|s| !s.is_empty());
            KnownFilesystem {
                device: d,
                uuid,
                label,
                partuuid: None,
            }
        })
        .collect()
}

/// Extra inputs of `image-audit` beyond the image and scope.
#[derive(Debug, Default, Clone)]
pub struct ImageAuditExtras<'a> {
    /// YAML file with user rules (`--rules`).
    pub rules: Option<&'a Path>,
    /// YAML file with waivers (`--waivers`).
    pub waivers: Option<&'a Path>,
    /// Saved `image-audit -o json` report; findings already in it are suppressed.
    pub baseline: Option<&'a Path>,
}

/// Open `image` read-only, mount it, run the audit and return the report.
/// `image` may also be an already mounted guest root directory.
pub fn collect_image_audit(image: &Path, scope: AuditScope, verbose: bool) -> Result<AuditReport> {
    collect_image_audit_with(image, scope, &RuleSet::default(), verbose)
}

fn collect_image_audit_with(
    image: &Path,
    scope: AuditScope,
    rules: &RuleSet,
    verbose: bool,
) -> Result<AuditReport> {
    if image.is_dir() {
        // An already mounted (or extracted) guest root: same confined reader,
        // no libguestfs. Filesystem references cannot be resolved without an
        // image, which the boot rules report as unverifiable.
        let root = GuestRoot::new(image)
            .with_context(|| format!("guest root {} is not accessible", image.display()))?;
        let mut report = run_audit(&root, scope, &[], &image.display().to_string());
        report.note("audited a directory tree, not a disk image: filesystem UUID/label references could not be verified");
        rules.run(&root, &mut report);
        return Ok(report);
    }
    let resolved = crate::storage::resolve_to_local_path(
        image
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
    )
    .unwrap_or_else(|_| image.to_path_buf());
    let mut g = init_guestfs_ro(&resolved, verbose)?;
    let root_dev = mount_all_ro(&mut g).context("No operating system found in disk image")?;
    let known = known_filesystems(&mut g, &root_dev);
    let root_dir = g.guest_root_dir()?;
    let root = GuestRoot::new(&root_dir)
        .with_context(|| format!("guest root {} is not accessible", root_dir.display()))?;
    let mut report = run_audit(&root, scope, &known, &image.display().to_string());
    rules.run(&root, &mut report);
    let _ = g.shutdown();
    Ok(report)
}

/// Audit plus the optional rules, waivers and baseline. Inputs are validated
/// before the image is opened so a typo fails fast.
pub fn build_image_audit(
    image: &Path,
    scope: AuditScope,
    extras: &ImageAuditExtras,
    verbose: bool,
) -> Result<AuditReport> {
    let rules = match extras.rules {
        Some(p) => RuleSet::load(p)?,
        None => RuleSet::default(),
    };
    let waivers = extras.waivers.map(WaiverSet::load).transpose()?;
    let baseline = match extras.baseline {
        Some(p) => Some(load_saved_report(p).ok_or_else(|| {
            anyhow::anyhow!(
                "baseline {} is not a saved `image-audit -o json` report (schema {AUDIT_SCHEMA})",
                p.display()
            )
        })?),
        None => None,
    };
    let mut report = collect_image_audit_with(image, scope, &rules, verbose)?;
    if let Some(b) = &baseline {
        apply_baseline(&mut report, b);
    }
    if let Some(w) = &waivers {
        w.apply(&mut report, chrono::Utc::now().date_naive());
    }
    Ok(report)
}

/// Render a report as `text` (None), json, yaml or sarif.
pub fn render_audit(format: &str, report: &AuditReport) -> Result<Option<String>> {
    if format.eq_ignore_ascii_case("sarif") {
        return Ok(Some(format!(
            "{}\n",
            serde_json::to_string_pretty(&to_sarif(report))?
        )));
    }
    crate::cli::output::render_structured(format, report)
}

fn print_finding(f: &crate::offline::Finding) {
    let sev = format!("{:?}", f.severity).to_uppercase();
    let sev = match f.severity {
        Severity::Critical | Severity::High => sev.red().bold().to_string(),
        Severity::Medium => sev.yellow().to_string(),
        _ => sev.dimmed().to_string(),
    };
    println!("  [{sev}] {} {} ({:?})", f.id, f.title, f.confidence);
    if let Some(l) = &f.location {
        println!("      at {l}");
    }
    if !f.detail.is_empty() {
        println!("      {}", f.detail);
    }
    println!("      fix: {}", f.remediation);
}

fn print_report(r: &AuditReport) {
    println!("{}", "Offline Image Audit".bold().cyan());
    println!("  image: {}", r.image);
    println!(
        "  {} finding(s): {} critical, {} high, {} medium, {} low, {} info ({} rule groups)",
        r.findings.len(),
        r.counts.critical,
        r.counts.high,
        r.counts.medium,
        r.counts.low,
        r.counts.info,
        r.checks_run
    );
    for f in &r.findings {
        print_finding(f);
    }
    if !r.waived.is_empty() {
        println!("\n{} finding(s) waived or in baseline:", r.waived.len());
        for w in &r.waived {
            println!("  - {} {} ({})", w.finding.id, w.finding.title, w.reason);
        }
    }
    if !r.notes.is_empty() {
        println!("\n{}", "Limits of this audit:".yellow());
        for n in &r.notes {
            println!("  - {n}");
        }
    }
}

pub fn image_audit_command(
    image: &Path,
    scope: &str,
    output_format: &str,
    fail_on: Option<&str>,
    extras: &ImageAuditExtras,
    verbose: bool,
) -> Result<()> {
    if !output_format.eq_ignore_ascii_case("sarif") {
        crate::cli::output::StructuredFormat::parse(output_format)?;
    }
    let scope = AuditScope::parse(scope)?;
    let threshold = fail_on.map(Severity::parse).transpose()?;
    let report = build_image_audit(image, scope, extras, verbose)?;
    match render_audit(output_format, &report)? {
        Some(s) => print!("{s}"),
        None => print_report(&report),
    }
    if let Some(t) = threshold {
        let n = report.at_least(t);
        if n > 0 {
            eprintln!("image-audit: {n} finding(s) at or above {t:?}");
            std::process::exit(2);
        }
    }
    Ok(())
}

/// Load a saved audit report if `path` is JSON with the audit schema.
fn load_saved_report(path: &Path) -> Option<AuditReport> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let meta = f.metadata().ok()?;
    if !meta.is_file() || meta.len() > 32 * 1024 * 1024 {
        return None;
    }
    let mut buf = Vec::new();
    f.by_ref()
        .take(32 * 1024 * 1024)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
        return None;
    }
    let r: AuditReport = serde_json::from_slice(&buf).ok()?;
    (r.schema == AUDIT_SCHEMA).then_some(r)
}

fn report_for(path: &Path, scope: AuditScope, verbose: bool) -> Result<AuditReport> {
    match load_saved_report(path) {
        Some(r) => Ok(r),
        None => collect_image_audit(path, scope, verbose),
    }
}

fn print_diff(d: &AuditDiff) {
    println!("{}", "Offline Audit Diff".bold().cyan());
    println!("  old: {}\n  new: {}", d.old_image, d.new_image);
    println!(
        "  verdict: {:?}  (+{} new, -{} resolved, {} severity change(s), {} unchanged)",
        d.verdict,
        d.added.len(),
        d.resolved.len(),
        d.severity_changed.len(),
        d.unchanged
    );
    if !d.added.is_empty() {
        println!("\n{}", "New findings:".red());
        for f in &d.added {
            print_finding(f);
        }
    }
    for c in &d.severity_changed {
        println!(
            "  ~ {} {} {:?} -> {:?}",
            c.id,
            c.subject.as_deref().unwrap_or(""),
            c.from,
            c.to
        );
    }
    if !d.resolved.is_empty() {
        println!("\n{}", "Resolved:".green());
        for f in &d.resolved {
            println!(
                "  - {} {} {}",
                f.id,
                f.title,
                f.subject.as_deref().unwrap_or("")
            );
        }
    }
}

pub fn image_audit_diff_command(
    old: &Path,
    new: &Path,
    scope: &str,
    output_format: &str,
    fail_on_regression: Option<&str>,
    verbose: bool,
) -> Result<()> {
    crate::cli::output::StructuredFormat::parse(output_format)?;
    let scope = AuditScope::parse(scope)?;
    let threshold = fail_on_regression.map(Severity::parse).transpose()?;
    let a = report_for(old, scope, verbose)?;
    let b = report_for(new, scope, verbose)?;
    let diff = diff_reports(&a, &b);
    match crate::cli::output::render_structured(output_format, &diff)? {
        Some(s) => print!("{s}"),
        None => print_diff(&diff),
    }
    if let Some(t) = threshold {
        let n = diff.regressions_at_least(t);
        if n > 0 || (diff.verdict == AuditVerdict::Regressed && t == Severity::Info) {
            eprintln!("image-audit-diff: {n} regression(s) at or above {t:?}");
            std::process::exit(2);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_report_loading_is_strict() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("a.json");
        let mut r = AuditReport::new("img");
        r.finish();
        std::fs::write(&good, serde_json::to_string(&r).unwrap()).unwrap();
        assert!(load_saved_report(&good).is_some());

        // Wrong schema, not JSON, binary junk, a directory and a missing file
        // all fall through to "treat as a disk image" without panicking.
        let wrong = dir.path().join("b.json");
        std::fs::write(&wrong, r#"{"schema":"other","image":"x","checks_run":0,"findings":[],"notes":[],"counts":{"critical":0,"high":0,"medium":0,"low":0,"info":0}}"#).unwrap();
        assert!(load_saved_report(&wrong).is_none());
        let junk = dir.path().join("c.img");
        std::fs::write(&junk, [0u8, 1, 2, 0xff, b'{']).unwrap();
        assert!(load_saved_report(&junk).is_none());
        assert!(load_saved_report(dir.path()).is_none());
        assert!(load_saved_report(&dir.path().join("missing")).is_none());
    }

    #[test]
    fn unknown_formats_and_scopes_are_rejected_before_opening_the_image() {
        let missing = Path::new("/nonexistent/disk.img");
        assert!(image_audit_command(
            missing,
            "all",
            "jsno",
            None,
            &ImageAuditExtras::default(),
            false
        )
        .is_err());
        assert!(image_audit_command(
            missing,
            "bogus",
            "json",
            None,
            &ImageAuditExtras::default(),
            false
        )
        .is_err());
        assert!(image_audit_command(
            missing,
            "all",
            "json",
            Some("severe"),
            &ImageAuditExtras::default(),
            false
        )
        .is_err());
        assert!(image_audit_diff_command(missing, missing, "all", "table", None, false).is_err());
    }
}
