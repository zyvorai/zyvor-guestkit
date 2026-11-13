// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration assessment: categorized sub-scores over the check results.

use super::checks::run_all_checks;
use super::readiness::{AssessContext, MigrationCheckResult, ReadinessCategory};
use crate::boot::report::{BootabilityReport, CheckSeverity, Finding};
use crate::boot::BootTarget;
use crate::cli::migrate::plan::{compute_migration_score, MigrationScoreReport};
use crate::evidence::EvidenceSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessLevel {
    Ready,
    ReadyWithWarnings,
    Blocked,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationSubScores {
    pub boot: f64,
    pub storage: f64,
    pub network: f64,
    pub driver: f64,
    pub application: f64,
    pub security: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationAssessment {
    pub target: String,
    pub live: bool,
    pub assessed_at: String,
    pub overall_score: f64,
    pub sub_scores: MigrationSubScores,
    pub readiness: ReadinessLevel,
    pub critical_blockers: Vec<Finding>,
    pub recommended_actions: Vec<String>,
    pub checks: Vec<MigrationCheckResult>,
    /// Last-known *running* state read from the guest's on-disk inventory
    /// cache during offline assessment (§31 combined assessment): present
    /// only when a live agent previously wrote an integrity-valid cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_correlation: Option<serde_json::Value>,
    /// Backward-compatible flat report (what `MigrateScore` returns today).
    pub legacy: MigrationScoreReport,
}

/// Run the migration check engine over evidence + an existing boot report.
pub fn assess_migration(
    ev: &EvidenceSnapshot,
    boot_report: &BootabilityReport,
    target: &str,
    live: bool,
) -> MigrationAssessment {
    let ctx = AssessContext {
        target: BootTarget::parse(target),
        target_name: target.to_string(),
        live,
        boot_report,
    };
    let checks = run_all_checks(ev, &ctx);

    let (sub_scores, overall_opt) = aggregate_scores(&checks);
    let overall_score = overall_opt.unwrap_or(boot_report.score);

    let critical_blockers: Vec<Finding> = checks
        .iter()
        .filter(|c| !c.passed && c.severity == CheckSeverity::Blocker)
        .map(|c| Finding {
            check_id: c.id.clone(),
            title: c.name.clone(),
            message: c.message.clone(),
            remediation: c
                .remediation
                .as_ref()
                .map(|h| format!("{h:?}"))
                .or(Some("manual intervention required".into())),
        })
        .collect();

    let recommended_actions: Vec<String> = checks
        .iter()
        .filter(|c| !c.passed && c.remediation.is_some())
        .map(|c| format!("[{}] {}: {}", c.id, c.name, c.message))
        .collect();

    let readiness = readiness_of(&checks);

    MigrationAssessment {
        target: target.to_string(),
        live,
        assessed_at: chrono::Utc::now().to_rfc3339(),
        overall_score,
        sub_scores,
        readiness,
        critical_blockers,
        recommended_actions,
        checks,
        online_correlation: ev.online_cache.clone(),
        legacy: compute_migration_score(ev, boot_report, target),
    }
}

/// Category sub-scores and overall score over a set of check results.
///
/// The overall score is `None` when the checks carry no weight (the caller
/// falls back to the boot report score). Shared by [`assess_migration`] and
/// [`super::explain`] so both always agree.
pub(crate) fn aggregate_scores(
    checks: &[MigrationCheckResult],
) -> (MigrationSubScores, Option<f64>) {
    let mut per_cat: BTreeMap<ReadinessCategory, (f64, f64)> = BTreeMap::new(); // (passed_w, total_w)
    for check in checks {
        let entry = per_cat.entry(check.category).or_insert((0.0, 0.0));
        entry.1 += check.weight;
        if check.passed {
            entry.0 += check.weight;
        } else {
            // Partial credit for advisory findings.
            entry.0 += match check.severity {
                CheckSeverity::Blocker => 0.0,
                CheckSeverity::Warning => check.weight * 0.4,
                CheckSeverity::Info => check.weight * 0.8,
            };
        }
    }
    let cat_score = |cat: ReadinessCategory| -> f64 {
        per_cat
            .get(&cat)
            .map(|(p, t)| {
                if *t > 0.0 {
                    (p / t * 100.0).round()
                } else {
                    100.0
                }
            })
            .unwrap_or(100.0)
    };
    let sub_scores = MigrationSubScores {
        boot: cat_score(ReadinessCategory::Boot),
        storage: cat_score(ReadinessCategory::Storage),
        network: cat_score(ReadinessCategory::Network),
        driver: cat_score(ReadinessCategory::Driver),
        application: cat_score(ReadinessCategory::Application),
        security: cat_score(ReadinessCategory::Security),
    };

    // Overall: weight by each category's total check weight so unpopulated
    // categories don't dilute the score.
    let (weighted_sum, weight_total) = per_cat
        .values()
        .fold((0.0, 0.0), |(ws, wt), (p, t)| (ws + p, wt + t));
    let overall = if weight_total > 0.0 {
        Some((weighted_sum / weight_total * 100.0).round())
    } else {
        None
    };

    (sub_scores, overall)
}

/// Readiness level implied by a set of check results.
pub(crate) fn readiness_of(checks: &[MigrationCheckResult]) -> ReadinessLevel {
    if checks
        .iter()
        .any(|c| !c.passed && c.severity == CheckSeverity::Blocker)
    {
        ReadinessLevel::Blocked
    } else if checks
        .iter()
        .any(|c| !c.passed && c.severity == CheckSeverity::Warning)
    {
        ReadinessLevel::ReadyWithWarnings
    } else {
        ReadinessLevel::Ready
    }
}
