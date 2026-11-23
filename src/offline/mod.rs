// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Offline image audit: hardening rules and boot-readiness checks that run
//! over a *mounted, read-only* guest root without booting it and without any
//! agent.
//!
//! Design rules (see `docs/features/offline-audit.md`):
//!
//! * Read-only. Nothing in this module writes to the guest tree.
//! * Hostile input is expected: every read goes through [`GuestRoot`], which
//!   confines symlinks to the guest root, refuses non-regular files (FIFOs,
//!   devices) and enforces file-size, directory fan-out and include-depth caps.
//! * Findings carry a [`Confidence`]: a check that cannot be proven offline is
//!   reported as `Likely` or `Heuristic`, never as `Confirmed`.
//! * Secrets are never copied into a report: only names, locations and counts.

pub mod boot_ready;
pub mod diff;
pub mod finding;
pub mod hardening;
pub mod root;

pub use boot_ready::{audit_boot_readiness, KnownFilesystem};
pub use diff::{diff_reports, AuditDiff, AuditVerdict, AUDIT_DIFF_SCHEMA};
pub use finding::{AuditReport, Confidence, Finding, Severity, AUDIT_SCHEMA};
pub use hardening::audit_hardening;
pub use root::{GuestRoot, Limits, ReadOutcome};

/// Which rule sets to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditScope {
    Hardening,
    Boot,
    All,
}

impl AuditScope {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "hardening" => Ok(Self::Hardening),
            "boot" => Ok(Self::Boot),
            "all" => Ok(Self::All),
            other => anyhow::bail!("unknown audit scope `{other}` (hardening, boot, all)"),
        }
    }
}

/// Run the selected rule sets over `root` and build one report.
pub fn run_audit(
    root: &GuestRoot,
    scope: AuditScope,
    known_fs: &[KnownFilesystem],
    image: &str,
) -> AuditReport {
    let mut report = AuditReport::new(image);
    if matches!(scope, AuditScope::Hardening | AuditScope::All) {
        audit_hardening(root, &mut report);
    }
    if matches!(scope, AuditScope::Boot | AuditScope::All) {
        audit_boot_readiness(root, known_fs, &mut report);
    }
    report.finish();
    report
}
