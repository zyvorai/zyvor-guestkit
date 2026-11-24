// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Waivers (accepted risks) and baselines for audit reports.
//!
//! A waiver is a reviewed decision, so it needs a reason and may carry an
//! expiry date. An expired waiver stops suppressing and is reported, and a
//! waiver that matches nothing is reported as stale, so the file cannot
//! silently rot. Suppressed findings stay in the report under `waived`; they
//! are never dropped without a trace.

use super::finding::{AuditReport, Finding};
use super::hardening::wild_match;
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const MAX_WAIVER_FILE: u64 = 1024 * 1024;
const MAX_WAIVERS: usize = 2000;

/// A finding that was suppressed, with the reason it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaivedFinding {
    pub finding: Finding,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaiverFile {
    waivers: Vec<WaiverSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaiverSpec {
    /// Rule id; `*` and `?` wildcards allowed (`SSH-*`).
    id: String,
    /// Optional wildcard on the finding's file path (line number ignored).
    #[serde(default)]
    path: Option<String>,
    /// Optional wildcard on the finding's subject.
    #[serde(default)]
    subject: Option<String>,
    reason: String,
    #[serde(default)]
    owner: Option<String>,
    /// `YYYY-MM-DD`; the waiver is valid through that day.
    #[serde(default)]
    expires: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Waiver {
    pub id: String,
    pub path: Option<String>,
    pub subject: Option<String>,
    pub reason: String,
    pub owner: Option<String>,
    pub expires: Option<NaiveDate>,
}

impl Waiver {
    fn label(&self) -> String {
        let mut s = self.id.clone();
        if let Some(p) = &self.path {
            s.push_str(&format!(" path={p}"));
        }
        if let Some(p) = &self.subject {
            s.push_str(&format!(" subject={p}"));
        }
        s
    }

    fn matches(&self, f: &Finding) -> bool {
        if !wild_match(&self.id.to_ascii_uppercase(), &f.id.to_ascii_uppercase()) {
            return false;
        }
        if let Some(p) = &self.path {
            let loc = f
                .location
                .as_deref()
                .map(|l| l.split(':').next().unwrap_or(l))
                .unwrap_or("");
            if !wild_match(p, loc) {
                return false;
            }
        }
        if let Some(s) = &self.subject {
            if !wild_match(s, f.subject.as_deref().unwrap_or("")) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Default)]
pub struct WaiverSet {
    pub waivers: Vec<Waiver>,
}

/// What applying a waiver file did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WaiverSummary {
    pub applied: usize,
    pub expired: usize,
    pub stale: usize,
}

impl WaiverSet {
    pub fn parse(text: &str) -> Result<Self> {
        let file: WaiverFile = serde_yaml::from_str(text).context("invalid waiver file")?;
        if file.waivers.len() > MAX_WAIVERS {
            bail!("waiver file has more than {MAX_WAIVERS} waivers");
        }
        let mut waivers = Vec::new();
        for (i, w) in file.waivers.into_iter().enumerate() {
            let n = i + 1;
            if w.id.trim().is_empty() {
                bail!("waiver #{n}: `id` is empty");
            }
            if w.reason.trim().is_empty() {
                bail!(
                    "waiver #{n} ({}): `reason` is required; a waiver is a documented decision",
                    w.id
                );
            }
            let expires = match &w.expires {
                None => None,
                Some(d) => Some(
                    NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").map_err(|_| {
                        anyhow::anyhow!(
                            "waiver #{n} ({}): `expires` must be YYYY-MM-DD, got `{d}`",
                            w.id
                        )
                    })?,
                ),
            };
            waivers.push(Waiver {
                id: w.id.trim().to_string(),
                path: w.path,
                subject: w.subject,
                reason: w.reason.trim().to_string(),
                owner: w.owner,
                expires,
            });
        }
        Ok(Self { waivers })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let meta = std::fs::metadata(path)
            .with_context(|| format!("cannot read waiver file {}", path.display()))?;
        if !meta.is_file() || meta.len() > MAX_WAIVER_FILE {
            bail!(
                "waiver file {} is not a regular file under {MAX_WAIVER_FILE} bytes",
                path.display()
            );
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read waiver file {}", path.display()))?;
        Self::parse(&text).with_context(|| path.display().to_string())
    }

    /// Move waived findings out of `report.findings` into `report.waived`.
    /// `today` is a parameter so behaviour is deterministic in tests.
    pub fn apply(&self, report: &mut AuditReport, today: NaiveDate) -> WaiverSummary {
        let mut summary = WaiverSummary::default();
        let mut used = vec![false; self.waivers.len()];
        let mut expired_noted: HashSet<usize> = HashSet::new();
        let mut kept = Vec::new();
        for f in std::mem::take(&mut report.findings) {
            let mut waived_by: Option<usize> = None;
            for (i, w) in self.waivers.iter().enumerate() {
                if !w.matches(&f) {
                    continue;
                }
                if w.expires.map(|e| today > e).unwrap_or(false) {
                    // Expired: does not suppress, but is reported once.
                    if expired_noted.insert(i) {
                        report.notes.push(format!(
                            "waiver `{}` expired on {} and no longer suppresses ({})",
                            w.label(),
                            w.expires.map(|d| d.to_string()).unwrap_or_default(),
                            w.reason
                        ));
                        summary.expired += 1;
                    }
                    used[i] = true;
                    continue;
                }
                used[i] = true;
                waived_by = Some(i);
                break;
            }
            match waived_by {
                Some(i) => {
                    let w = &self.waivers[i];
                    summary.applied += 1;
                    report.waived.push(WaivedFinding {
                        finding: f,
                        reason: w.reason.clone(),
                        owner: w.owner.clone(),
                        expires: w.expires.map(|d| d.to_string()),
                    });
                }
                None => kept.push(f),
            }
        }
        report.findings = kept;
        for (i, w) in self.waivers.iter().enumerate() {
            if !used[i] {
                summary.stale += 1;
                report.notes.push(format!(
                    "waiver `{}` matched no finding (stale?)",
                    w.label()
                ));
            }
        }
        report.finish();
        summary
    }
}

/// Move every finding that also appears in `baseline` (same rule, file and
/// subject) into `report.waived`. Returns how many were moved.
pub fn apply_baseline(report: &mut AuditReport, baseline: &AuditReport) -> usize {
    let known: HashSet<String> = baseline.findings.iter().map(|f| f.key()).collect();
    let mut kept = Vec::new();
    let mut moved = 0;
    for f in std::mem::take(&mut report.findings) {
        if known.contains(&f.key()) {
            moved += 1;
            report.waived.push(WaivedFinding {
                finding: f,
                reason: format!("present in baseline {}", baseline.image),
                owner: None,
                expires: None,
            });
        } else {
            kept.push(f);
        }
    }
    report.findings = kept;
    report.finish();
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::finding::{Confidence, Severity};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn report() -> AuditReport {
        let mut r = AuditReport::new("img");
        r.push(
            Finding::new(
                "SSH-001",
                "hardening",
                Severity::High,
                Confidence::Confirmed,
                "root",
                "fix",
            )
            .at("/etc/ssh/sshd_config:12")
            .subject("PermitRootLogin"),
        );
        r.push(
            Finding::new(
                "SUDO-001",
                "hardening",
                Severity::High,
                Confidence::Confirmed,
                "nopasswd",
                "fix",
            )
            .at("/etc/sudoers.d/ops:1")
            .subject("ops"),
        );
        r.finish();
        r
    }

    #[test]
    fn waiver_requires_reason_and_valid_date() {
        assert!(WaiverSet::parse("waivers:\n  - id: SSH-001\n    reason: ''\n").is_err());
        let e =
            WaiverSet::parse("waivers:\n  - id: SSH-001\n    reason: ok\n    expires: tomorrow\n")
                .unwrap_err()
                .to_string();
        assert!(e.contains("YYYY-MM-DD"), "{e}");
        assert!(WaiverSet::parse("waivers:\n  - id: X\n    reason: r\n    bogus: 1\n").is_err());
        assert!(WaiverSet::parse("not: waivers\n").is_err());
    }

    #[test]
    fn applies_by_id_path_and_subject_and_keeps_audit_trail() {
        let set = WaiverSet::parse(
            "waivers:\n  - id: SSH-*\n    path: /etc/ssh/*\n    subject: PermitRootLogin\n    reason: bastion needs it\n    owner: sec\n    expires: 2030-01-01\n",
        )
        .unwrap();
        let mut r = report();
        let s = set.apply(&mut r, d("2026-11-24"));
        assert_eq!(s.applied, 1);
        assert_eq!(r.findings.len(), 1);
        assert_eq!(r.findings[0].id, "SUDO-001");
        assert_eq!(r.counts.high, 1);
        assert_eq!(r.waived.len(), 1);
        assert_eq!(r.waived[0].reason, "bastion needs it");
        assert_eq!(r.waived[0].owner.as_deref(), Some("sec"));
        assert!(r.notes.is_empty());
    }

    #[test]
    fn expired_waiver_does_not_suppress_and_is_reported() {
        let set = WaiverSet::parse(
            "waivers:\n  - id: SUDO-001\n    reason: migration\n    expires: 2026-11-23\n",
        )
        .unwrap();
        let mut r = report();
        let s = set.apply(&mut r, d("2026-11-24"));
        assert_eq!((s.applied, s.expired), (0, 1));
        assert_eq!(r.findings.len(), 2);
        assert!(r.notes[0].contains("expired on 2026-11-23"));
        // Valid through the expiry day itself.
        let mut r2 = report();
        assert_eq!(set.apply(&mut r2, d("2026-11-23")).applied, 1);
    }

    #[test]
    fn stale_waiver_is_reported_and_path_mismatch_does_not_match() {
        let set = WaiverSet::parse(
            "waivers:\n  - id: SSH-001\n    path: /etc/other\n    reason: r\n  - id: NOPE-1\n    reason: r\n",
        )
        .unwrap();
        let mut r = report();
        let s = set.apply(&mut r, d("2026-11-24"));
        assert_eq!((s.applied, s.stale), (0, 2));
        assert_eq!(r.findings.len(), 2);
        assert!(r.notes.iter().all(|n| n.contains("matched no finding")));
    }

    #[test]
    fn baseline_moves_known_findings_only() {
        let base = report();
        let mut r = report();
        r.push(
            Finding::new(
                "SSH-002",
                "hardening",
                Severity::Medium,
                Confidence::Confirmed,
                "new",
                "fix",
            )
            .at("/etc/ssh/sshd_config:30")
            .subject("PasswordAuthentication"),
        );
        // The same finding on a different line is still the baseline finding.
        r.findings[0].location = Some("/etc/ssh/sshd_config:99".into());
        r.finish();
        let moved = apply_baseline(&mut r, &base);
        assert_eq!(moved, 2);
        assert_eq!(r.findings.len(), 1);
        assert_eq!(r.findings[0].id, "SSH-002");
        assert!(r.waived[0].reason.contains("baseline img"));
    }
}
