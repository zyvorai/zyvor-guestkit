// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Machine-readable difference between two offline audit reports.
//!
//! Findings are matched by `(rule id, file, subject)`; line numbers are
//! ignored so that unrelated edits that shift a line do not look like drift.

use super::finding::{AuditReport, Finding, Severity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const AUDIT_DIFF_SCHEMA: &str = "guestkit.offline-audit-diff/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditVerdict {
    Unchanged,
    Improved,
    Regressed,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeverityChange {
    pub id: String,
    pub subject: Option<String>,
    pub location: Option<String>,
    pub from: Severity,
    pub to: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDiff {
    pub schema: String,
    pub old_image: String,
    pub new_image: String,
    pub added: Vec<Finding>,
    pub resolved: Vec<Finding>,
    pub severity_changed: Vec<SeverityChange>,
    pub unchanged: usize,
    pub verdict: AuditVerdict,
}

impl AuditDiff {
    /// Number of regressions at or above `min`: new findings plus findings
    /// whose severity rose to `min` or higher.
    pub fn regressions_at_least(&self, min: Severity) -> usize {
        self.added.iter().filter(|f| f.severity >= min).count()
            + self
                .severity_changed
                .iter()
                .filter(|c| c.to > c.from && c.to >= min)
                .count()
    }
}

fn index(r: &AuditReport) -> BTreeMap<String, &Finding> {
    let mut m: BTreeMap<String, &Finding> = BTreeMap::new();
    for f in &r.findings {
        // Several findings can share a key (same user, several sudoers lines):
        // keep the most severe one.
        m.entry(f.key())
            .and_modify(|cur| {
                if f.severity > cur.severity {
                    *cur = f;
                }
            })
            .or_insert(f);
    }
    m
}

pub fn diff_reports(old: &AuditReport, new: &AuditReport) -> AuditDiff {
    let (a, b) = (index(old), index(new));
    let mut added = Vec::new();
    let mut resolved = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = 0usize;
    for (k, f) in &b {
        match a.get(k) {
            None => added.push((*f).clone()),
            Some(o) if o.severity != f.severity => changed.push(SeverityChange {
                id: f.id.clone(),
                subject: f.subject.clone(),
                location: f.location.clone(),
                from: o.severity,
                to: f.severity,
            }),
            Some(_) => unchanged += 1,
        }
    }
    for (k, f) in &a {
        if !b.contains_key(k) {
            resolved.push((*f).clone());
        }
    }
    let sort = |v: &mut Vec<Finding>| {
        v.sort_by(|x, y| {
            y.severity
                .cmp(&x.severity)
                .then_with(|| x.key().cmp(&y.key()))
        })
    };
    sort(&mut added);
    sort(&mut resolved);
    let worse = !added.is_empty() || changed.iter().any(|c| c.to > c.from);
    let better = !resolved.is_empty() || changed.iter().any(|c| c.to < c.from);
    let verdict = match (worse, better) {
        (false, false) => AuditVerdict::Unchanged,
        (true, false) => AuditVerdict::Regressed,
        (false, true) => AuditVerdict::Improved,
        (true, true) => AuditVerdict::Mixed,
    };
    AuditDiff {
        schema: AUDIT_DIFF_SCHEMA.to_string(),
        old_image: old.image.clone(),
        new_image: new.image.clone(),
        added,
        resolved,
        severity_changed: changed,
        unchanged,
        verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::super::finding::Confidence;
    use super::*;

    fn f(id: &str, sev: Severity, loc: &str, subj: &str) -> Finding {
        Finding::new(id, "t", sev, Confidence::Confirmed, "t", "fix")
            .at(loc)
            .subject(subj)
    }
    fn rep(img: &str, fs: Vec<Finding>) -> AuditReport {
        let mut r = AuditReport::new(img);
        for x in fs {
            r.push(x);
        }
        r.finish();
        r
    }

    #[test]
    fn identical_reports_are_unchanged_even_if_lines_shift() {
        let a = rep(
            "a",
            vec![f("SSH-001", Severity::High, "/etc/ssh/sshd_config:3", "x")],
        );
        let b = rep(
            "b",
            vec![f("SSH-001", Severity::High, "/etc/ssh/sshd_config:9", "x")],
        );
        let d = diff_reports(&a, &b);
        assert_eq!(d.verdict, AuditVerdict::Unchanged);
        assert_eq!(d.unchanged, 1);
    }

    #[test]
    fn added_resolved_and_changed() {
        let a = rep(
            "a",
            vec![
                f("A", Severity::High, "/a:1", "s"),
                f("B", Severity::Low, "/b:1", "s"),
                f("C", Severity::Medium, "/c:1", "s"),
            ],
        );
        let b = rep(
            "b",
            vec![
                f("B", Severity::High, "/b:1", "s"),
                f("C", Severity::Medium, "/c:2", "s"),
                f("D", Severity::Critical, "/d:1", "s"),
            ],
        );
        let d = diff_reports(&a, &b);
        assert_eq!(d.verdict, AuditVerdict::Mixed);
        assert_eq!(d.added.len(), 1);
        assert_eq!(d.resolved.len(), 1);
        assert_eq!(d.severity_changed.len(), 1);
        assert_eq!(d.regressions_at_least(Severity::High), 2);
        assert_eq!(d.regressions_at_least(Severity::Critical), 1);
    }

    #[test]
    fn improved_and_regressed_verdicts() {
        let a = rep("a", vec![f("A", Severity::High, "/a", "s")]);
        let empty = rep("b", vec![]);
        assert_eq!(diff_reports(&a, &empty).verdict, AuditVerdict::Improved);
        assert_eq!(diff_reports(&empty, &a).verdict, AuditVerdict::Regressed);
    }

    #[test]
    fn duplicate_keys_collapse_to_most_severe() {
        let a = rep(
            "a",
            vec![
                f("S", Severity::Medium, "/s:1", "u"),
                f("S", Severity::High, "/s:2", "u"),
            ],
        );
        let b = rep("b", vec![f("S", Severity::High, "/s:7", "u")]);
        assert_eq!(diff_reports(&a, &b).verdict, AuditVerdict::Unchanged);
    }

    #[test]
    fn report_round_trips_through_json() {
        let a = rep("a", vec![f("A", Severity::High, "/a:1", "s")]);
        let j = serde_json::to_string(&a).unwrap();
        let back: AuditReport = serde_json::from_str(&j).unwrap();
        assert_eq!(diff_reports(&a, &back).verdict, AuditVerdict::Unchanged);
    }
}
