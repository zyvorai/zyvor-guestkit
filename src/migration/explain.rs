// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! "Path to ready": explain which failed checks to fix, in what order, and
//! what each fix is worth.
//!
//! Works from the check list of a [`MigrationAssessment`] alone: it replays
//! the scoring with failed checks flipped to passed, so it needs no image,
//! is deterministic, and cannot disagree with the real score (it shares the
//! aggregation in [`super::score`]).

use super::readiness::{MigrationCheckResult, ReadinessCategory};
use super::score::{aggregate_scores, readiness_of, MigrationAssessment, ReadinessLevel};
use crate::boot::report::CheckSeverity;
use serde::{Deserialize, Serialize};

/// Schema tag of [`AssessmentExplain`] documents.
pub const ASSESSMENT_EXPLAIN_SCHEMA: &str = "guestkit.assessment_explain/1";

/// One recommended fix in the ordered plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExplainStep {
    pub order: usize,
    pub id: String,
    pub name: String,
    pub category: ReadinessCategory,
    pub severity: CheckSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    /// Overall-score gain if only this check were fixed.
    pub standalone_gain: f64,
    /// Overall score after applying this and every earlier step.
    pub cumulative_score: f64,
    pub cumulative_readiness: ReadinessLevel,
}

/// Ordered fix plan with projected outcomes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentExplain {
    pub schema: String,
    pub target: String,
    pub current_score: f64,
    pub current_readiness: ReadinessLevel,
    /// Blockers first (they gate cutover), then by standalone gain, then id.
    pub steps: Vec<ExplainStep>,
    /// Score and readiness once every blocker is fixed.
    pub score_if_blockers_fixed: f64,
    pub readiness_if_blockers_fixed: ReadinessLevel,
    /// Score and readiness once every failed check is fixed.
    pub score_if_all_fixed: f64,
    pub readiness_if_all_fixed: ReadinessLevel,
}

fn overall(checks: &[MigrationCheckResult], fallback: f64) -> f64 {
    aggregate_scores(checks).1.unwrap_or(fallback)
}

fn fixed(c: &MigrationCheckResult) -> MigrationCheckResult {
    let mut c = c.clone();
    c.passed = true;
    c.severity = CheckSeverity::Info;
    c.remediation = None;
    c
}

fn sev_rank(s: CheckSeverity) -> u8 {
    match s {
        CheckSeverity::Blocker => 0,
        CheckSeverity::Warning => 1,
        CheckSeverity::Info => 2,
    }
}

/// Build the ordered "path to ready" for an assessment.
pub fn explain_assessment(a: &MigrationAssessment) -> AssessmentExplain {
    let fallback = a.overall_score;
    let base = overall(&a.checks, fallback);

    let mut failed: Vec<(usize, f64)> = a
        .checks
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.passed)
        .map(|(i, c)| {
            let mut t = a.checks.clone();
            t[i] = fixed(c);
            (i, overall(&t, fallback) - base)
        })
        .collect();
    failed.sort_by(|(i, ga), (j, gb)| {
        let (ci, cj) = (&a.checks[*i], &a.checks[*j]);
        sev_rank(ci.severity)
            .cmp(&sev_rank(cj.severity))
            .then(gb.partial_cmp(ga).unwrap_or(std::cmp::Ordering::Equal))
            .then(ci.id.cmp(&cj.id))
    });

    let n_blockers = failed
        .iter()
        .filter(|(i, _)| a.checks[*i].severity == CheckSeverity::Blocker)
        .count();
    let mut work = a.checks.clone();
    let mut steps = Vec::new();
    let mut blockers_fixed = (n_blockers == 0).then(|| (base, readiness_of(&work)));
    for (n, (i, gain)) in failed.iter().enumerate() {
        let c = &a.checks[*i];
        work[*i] = fixed(c);
        let score = overall(&work, fallback);
        let readiness = readiness_of(&work);
        if n + 1 == n_blockers {
            blockers_fixed = Some((score, readiness));
        }
        steps.push(ExplainStep {
            order: n + 1,
            id: c.id.clone(),
            name: c.name.clone(),
            category: c.category,
            severity: c.severity,
            message: c.message.clone(),
            remediation: c.remediation.as_ref().map(|h| format!("{h:?}")),
            standalone_gain: (gain * 100.0).round() / 100.0,
            cumulative_score: score,
            cumulative_readiness: readiness,
        });
    }
    let (bs, br) = blockers_fixed.unwrap_or((base, readiness_of(&a.checks)));
    AssessmentExplain {
        schema: ASSESSMENT_EXPLAIN_SCHEMA.into(),
        target: a.target.clone(),
        current_score: a.overall_score,
        current_readiness: a.readiness,
        steps,
        score_if_blockers_fixed: bs,
        readiness_if_blockers_fixed: br,
        score_if_all_fixed: overall(&work, fallback),
        readiness_if_all_fixed: readiness_of(&work),
    }
}

impl AssessmentExplain {
    /// Plain-text rendering for terminals.
    pub fn render_text(&self) -> String {
        if self.steps.is_empty() {
            return format!(
                "Path to ready: nothing to fix ({:.0}/100, {:?})\n",
                self.current_score, self.current_readiness
            );
        }
        let mut s = format!(
            "Path to ready ({:.0} now, {:.0} if blockers fixed, {:.0} if all fixed):\n",
            self.current_score, self.score_if_blockers_fixed, self.score_if_all_fixed
        );
        for st in &self.steps {
            s.push_str(&format!(
                "  {}. [{}] {:?} {}: {:+.1} alone, then {:.0} ({:?})\n",
                st.order,
                st.id,
                st.severity,
                st.name,
                st.standalone_gain,
                st.cumulative_score,
                st.cumulative_readiness
            ));
            if let Some(r) = &st.remediation {
                s.push_str(&format!("       fix: {r}\n"));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::migrate::plan::MigrationScoreReport;
    use crate::migration::readiness::RemediationHint;

    fn assessment(checks: Vec<MigrationCheckResult>) -> MigrationAssessment {
        let (sub_scores, o) = aggregate_scores(&checks);
        let overall_score = o.unwrap_or(0.0);
        let readiness = readiness_of(&checks);
        MigrationAssessment {
            target: "kvm".into(),
            live: false,
            assessed_at: "t".into(),
            overall_score,
            sub_scores,
            readiness,
            critical_blockers: vec![],
            recommended_actions: vec![],
            checks,
            online_correlation: None,
            legacy: MigrationScoreReport {
                score: overall_score,
                driver_injections: vec![],
                required_changes: vec![],
                licensing_warnings: vec![],
                estimated_downtime_minutes: 0,
            },
        }
    }

    use ReadinessCategory as C;
    fn ok(id: &str, w: f64) -> MigrationCheckResult {
        MigrationCheckResult::pass(id, id, C::Boot, w, "ok")
    }
    fn bad(id: &str, w: f64, sev: CheckSeverity) -> MigrationCheckResult {
        MigrationCheckResult::fail(
            id,
            id,
            C::Driver,
            w,
            sev,
            "bad",
            Some(RemediationHint::AddVirtioToInitramfs),
        )
    }

    #[test]
    fn all_passing_has_no_steps() {
        let e = explain_assessment(&assessment(vec![ok("A", 1.0), ok("B", 2.0)]));
        assert!(e.steps.is_empty());
        assert_eq!(e.score_if_all_fixed, e.current_score);
        assert_eq!(e.readiness_if_blockers_fixed, ReadinessLevel::Ready);
        assert!(e.render_text().contains("nothing to fix"));
    }

    #[test]
    fn blockers_ordered_first_and_unlock_readiness() {
        let a = assessment(vec![
            ok("OK", 4.0),
            bad("W", 4.0, CheckSeverity::Warning),
            bad("B", 1.0, CheckSeverity::Blocker),
        ]);
        assert_eq!(a.readiness, ReadinessLevel::Blocked);
        let e = explain_assessment(&a);
        assert_eq!(e.steps[0].id, "B");
        assert_eq!(e.steps[1].id, "W");
        assert_eq!(
            e.readiness_if_blockers_fixed,
            ReadinessLevel::ReadyWithWarnings
        );
        assert_eq!(
            e.steps[0].cumulative_readiness,
            ReadinessLevel::ReadyWithWarnings
        );
        assert_eq!(e.readiness_if_all_fixed, ReadinessLevel::Ready);
        assert_eq!(e.score_if_all_fixed, 100.0);
        assert!(e.score_if_blockers_fixed > e.current_score);
        assert!(e.steps[0]
            .remediation
            .as_deref()
            .unwrap()
            .contains("AddVirtio"));
    }

    #[test]
    fn same_severity_sorted_by_gain_then_id() {
        let a = assessment(vec![
            ok("OK", 1.0),
            bad("SMALL", 1.0, CheckSeverity::Blocker),
            bad("BIG", 6.0, CheckSeverity::Blocker),
        ]);
        let e = explain_assessment(&a);
        assert_eq!(e.steps[0].id, "BIG");
        assert!(e.steps[0].standalone_gain > e.steps[1].standalone_gain);
        assert!(e.steps[1].cumulative_score >= e.steps[0].cumulative_score);
        assert_eq!(e.steps[1].cumulative_score, e.score_if_all_fixed);
    }

    #[test]
    fn aggregation_gives_partial_credit_to_warnings() {
        let checks = vec![ok("A", 1.0), bad("B", 1.0, CheckSeverity::Warning)];
        let (sub, overall) = aggregate_scores(&checks);
        // Warning earns 40% partial credit: (1 + 0.4) / 2 = 70.
        assert_eq!(overall, Some(70.0));
        assert_eq!(sub.driver, 40.0);
        assert_eq!(sub.boot, 100.0);
    }
}
