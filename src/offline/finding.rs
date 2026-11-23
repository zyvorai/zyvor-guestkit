// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Finding and report types shared by the offline audit rule sets.

use serde::{Deserialize, Serialize};

pub const AUDIT_SCHEMA: &str = "guestkit.offline-audit/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "info" => Ok(Self::Info),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            other => {
                anyhow::bail!("unknown severity `{other}` (info, low, medium, high, critical)")
            }
        }
    }
}

/// How sure the check is. Offline analysis cannot prove everything, so this is
/// part of every finding rather than an afterthought.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Heuristic: depends on state this offline view cannot see.
    Heuristic,
    /// Likely: directly observed but the effect depends on defaults/versions.
    Likely,
    /// Confirmed: the exact setting or missing file was read from the image.
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Stable rule id, for example `SSH-001`.
    pub id: String,
    pub category: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub title: String,
    pub detail: String,
    /// Guest path (optionally `path:line`). Never contains secret values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// What the finding is about (account name, option, kernel version...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub remediation: String,
}

impl Finding {
    pub fn new(
        id: &str,
        category: &str,
        severity: Severity,
        confidence: Confidence,
        title: impl Into<String>,
        remediation: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            category: category.to_string(),
            severity,
            confidence,
            title: title.into(),
            detail: String::new(),
            location: None,
            subject: None,
            remediation: remediation.to_string(),
        }
    }

    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }

    pub fn at(mut self, loc: impl Into<String>) -> Self {
        self.location = Some(loc.into());
        self
    }

    pub fn subject(mut self, s: impl Into<String>) -> Self {
        self.subject = Some(s.into());
        self
    }

    /// Identity used by the diff: rule + file (without line number) + subject.
    pub fn key(&self) -> String {
        let path = self
            .location
            .as_deref()
            .map(|l| l.split(':').next().unwrap_or(l))
            .unwrap_or("");
        format!(
            "{}|{}|{}",
            self.id,
            path,
            self.subject.as_deref().unwrap_or("")
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeverityCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub info: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub schema: String,
    pub image: String,
    /// Number of rule groups that ran.
    pub checks_run: usize,
    pub findings: Vec<Finding>,
    /// Honest limits: files too large, unreadable, caps hit, unverifiable items.
    pub notes: Vec<String>,
    pub counts: SeverityCounts,
}

impl AuditReport {
    pub fn new(image: &str) -> Self {
        Self {
            schema: AUDIT_SCHEMA.to_string(),
            image: image.to_string(),
            checks_run: 0,
            findings: Vec::new(),
            notes: Vec::new(),
            counts: SeverityCounts::default(),
        }
    }

    pub fn push(&mut self, f: Finding) {
        self.findings.push(f);
    }

    pub fn note(&mut self, n: impl Into<String>) {
        let n = n.into();
        if !self.notes.contains(&n) {
            self.notes.push(n);
        }
    }

    /// Sort deterministically and recompute counts.
    pub fn finish(&mut self) {
        self.findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.id.cmp(&b.id))
                .then_with(|| a.location.cmp(&b.location))
                .then_with(|| a.subject.cmp(&b.subject))
        });
        self.findings.dedup();
        let mut c = SeverityCounts::default();
        for f in &self.findings {
            match f.severity {
                Severity::Critical => c.critical += 1,
                Severity::High => c.high += 1,
                Severity::Medium => c.medium += 1,
                Severity::Low => c.low += 1,
                Severity::Info => c.info += 1,
            }
        }
        self.counts = c;
    }

    pub fn max_severity(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }

    /// Findings at or above `min`.
    pub fn at_least(&self, min: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity >= min).count()
    }
}
