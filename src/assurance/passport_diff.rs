// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cutover Passport drift: a machine-readable diff between two passports.
//!
//! Answers "did this VM get safer or riskier to cut over since last time?"
//! from two passport JSON files alone (no image access), so it is cheap to
//! run in CI against a stored baseline.

use super::passport::{CutoverPassport, PassportFinding};
use crate::migration::ReadinessLevel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Schema tag of [`PassportDiff`] documents.
pub const PASSPORT_DIFF_SCHEMA: &str = "guestkit.passport_diff/1";

/// Overall direction of change from `old` to `new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftVerdict {
    Unchanged,
    Improved,
    Regressed,
    /// Some things got better and some got worse.
    Mixed,
}

/// Old/new pair for a scalar that changed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Delta<T> {
    pub old: T,
    pub new: T,
}

/// Score movement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreDrift {
    pub old: f64,
    pub new: f64,
    pub delta: f64,
}

/// Machine-readable diff between two Cutover Passports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassportDiff {
    pub schema: String,
    pub old_generated_at: String,
    pub new_generated_at: String,
    /// Same image path, size and (when both present) content hash.
    pub same_image: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Delta<String>>,
    pub boot: ScoreDrift,
    pub migration: ScoreDrift,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Delta<ReadinessLevel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hard_blocked: Option<Delta<bool>>,
    pub blockers_added: Vec<PassportFinding>,
    pub blockers_resolved: Vec<PassportFinding>,
    pub actions_added: Vec<String>,
    pub actions_removed: Vec<String>,
    pub plan_ops_added: Vec<String>,
    pub plan_ops_removed: Vec<String>,
    /// Changed Windows readiness flags, keyed by flag name.
    pub windows_changes: BTreeMap<String, Delta<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_passed: Option<Delta<Option<bool>>>,
    /// Human-readable reasons the change counts as a regression.
    pub regressions: Vec<String>,
    /// Human-readable reasons the change counts as an improvement.
    pub improvements: Vec<String>,
    pub verdict: DriftVerdict,
}

/// Options for [`diff_passports`].
#[derive(Debug, Clone, Default)]
pub struct PassportDiffOptions {
    /// Score drops no larger than this many points are not regressions.
    pub score_tolerance: f64,
}

fn readiness_rank(r: ReadinessLevel) -> u8 {
    match r {
        ReadinessLevel::Ready => 2,
        ReadinessLevel::ReadyWithWarnings => 1,
        ReadinessLevel::Blocked => 0,
    }
}

fn score(old: f64, new: f64) -> ScoreDrift {
    ScoreDrift {
        old,
        new,
        delta: ((new - old) * 100.0).round() / 100.0,
    }
}

fn set_diff<T: Clone + PartialEq>(a: &[T], b: &[T]) -> Vec<T> {
    a.iter().filter(|x| !b.contains(x)).cloned().collect()
}

/// Compare two passports. Pure function: no I/O, deterministic.
pub fn diff_passports(
    old: &CutoverPassport,
    new: &CutoverPassport,
    opts: &PassportDiffOptions,
) -> PassportDiff {
    let same_image = old.image.path == new.image.path
        && old.image.size_bytes == new.image.size_bytes
        && match (&old.image.content_sha256, &new.image.content_sha256) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        };

    // Blockers are identified by check id.
    let ids = |p: &CutoverPassport| -> Vec<String> {
        p.critical_blockers.iter().map(|b| b.id.clone()).collect()
    };
    let (old_ids, new_ids) = (ids(old), ids(new));
    let blockers_added: Vec<PassportFinding> = new
        .critical_blockers
        .iter()
        .filter(|b| !old_ids.contains(&b.id))
        .cloned()
        .collect();
    let blockers_resolved: Vec<PassportFinding> = old
        .critical_blockers
        .iter()
        .filter(|b| !new_ids.contains(&b.id))
        .cloned()
        .collect();

    let mut windows_changes = BTreeMap::new();
    if let (Ok(serde_json::Value::Object(a)), Ok(serde_json::Value::Object(b))) = (
        serde_json::to_value(&old.windows),
        serde_json::to_value(&new.windows),
    ) {
        let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
        for k in keys {
            let (x, y) = (
                a.get(&k).cloned().unwrap_or(serde_json::Value::Null),
                b.get(&k).cloned().unwrap_or(serde_json::Value::Null),
            );
            if x != y {
                windows_changes.insert(k, Delta { old: x, new: y });
            }
        }
    }

    let boot = score(old.scores.boot, new.scores.boot);
    let migration = score(old.scores.migration, new.scores.migration);
    let readiness = (old.scores.readiness != new.scores.readiness).then_some(Delta {
        old: old.scores.readiness,
        new: new.scores.readiness,
    });
    let hard_blocked = (old.hard_blocked != new.hard_blocked).then_some(Delta {
        old: old.hard_blocked,
        new: new.hard_blocked,
    });
    let old_pol = old.policy.as_ref().map(|p| p.passed);
    let new_pol = new.policy.as_ref().map(|p| p.passed);
    let policy_passed = (old_pol != new_pol).then_some(Delta {
        old: old_pol,
        new: new_pol,
    });

    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    for (name, s) in [("boot", &boot), ("migration", &migration)] {
        if s.delta < -opts.score_tolerance && s.delta < 0.0 {
            regressions.push(format!("{name} score fell {:.0} -> {:.0}", s.old, s.new));
        } else if s.delta > 0.0 {
            improvements.push(format!("{name} score rose {:.0} -> {:.0}", s.old, s.new));
        }
    }
    if let Some(r) = &readiness {
        let line = format!("readiness {:?} -> {:?}", r.old, r.new);
        if readiness_rank(r.new) < readiness_rank(r.old) {
            regressions.push(line);
        } else {
            improvements.push(line);
        }
    }
    if let Some(h) = &hard_blocked {
        if h.new {
            regressions.push("image became hard-blocked".into());
        } else {
            improvements.push("image is no longer hard-blocked".into());
        }
    }
    for b in &blockers_added {
        regressions.push(format!("new blocker {}: {}", b.id, b.title));
    }
    for b in &blockers_resolved {
        improvements.push(format!("resolved blocker {}: {}", b.id, b.title));
    }
    if let Some(p) = &policy_passed {
        match (p.old, p.new) {
            (Some(true), Some(false)) => regressions.push("policy verdict now fails".into()),
            (Some(false), Some(true)) => improvements.push("policy verdict now passes".into()),
            _ => {}
        }
    }

    let verdict = match (regressions.is_empty(), improvements.is_empty()) {
        (true, true) => DriftVerdict::Unchanged,
        (true, false) => DriftVerdict::Improved,
        (false, true) => DriftVerdict::Regressed,
        (false, false) => DriftVerdict::Mixed,
    };

    PassportDiff {
        schema: PASSPORT_DIFF_SCHEMA.into(),
        old_generated_at: old.generated_at.clone(),
        new_generated_at: new.generated_at.clone(),
        same_image,
        target: (old.target != new.target).then(|| Delta {
            old: old.target.clone(),
            new: new.target.clone(),
        }),
        boot,
        migration,
        readiness,
        hard_blocked,
        blockers_added,
        blockers_resolved,
        actions_added: set_diff(&new.recommended_actions, &old.recommended_actions),
        actions_removed: set_diff(&old.recommended_actions, &new.recommended_actions),
        plan_ops_added: set_diff(&new.fix_plan.operation_ids, &old.fix_plan.operation_ids),
        plan_ops_removed: set_diff(&old.fix_plan.operation_ids, &new.fix_plan.operation_ids),
        windows_changes,
        policy_passed,
        regressions,
        improvements,
        verdict,
    }
}

impl PassportDiff {
    /// True when the change is a regression or a mix containing one.
    pub fn has_regression(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Plain-text rendering for terminals and CI logs.
    pub fn render_text(&self) -> String {
        let mut s = format!(
            "Passport drift: {:?}  ({} -> {})\n",
            self.verdict, self.old_generated_at, self.new_generated_at
        );
        if !self.same_image {
            s.push_str("  note: passports describe different image files\n");
        }
        s.push_str(&format!(
            "  boot       {:.0} -> {:.0} ({:+})\n  migration  {:.0} -> {:.0} ({:+})\n",
            self.boot.old,
            self.boot.new,
            self.boot.delta,
            self.migration.old,
            self.migration.new,
            self.migration.delta
        ));
        for r in &self.regressions {
            s.push_str(&format!("  [-] {r}\n"));
        }
        for i in &self.improvements {
            s.push_str(&format!("  [+] {i}\n"));
        }
        for o in &self.plan_ops_added {
            s.push_str(&format!("  fix-plan op added: {o}\n"));
        }
        for o in &self.plan_ops_removed {
            s.push_str(&format!("  fix-plan op removed: {o}\n"));
        }
        for (k, d) in &self.windows_changes {
            s.push_str(&format!("  windows.{k}: {} -> {}\n", d.old, d.new));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn passport(extra: serde_json::Value) -> CutoverPassport {
        let mut v = json!({
            "schema_version": "1", "kind": "guestkit.cutover_passport",
            "generated_at": "2026-01-01T00:00:00Z", "tool_version": "1.2.11",
            "target": "kvm",
            "image": {"path": "a.qcow2", "size_bytes": 10},
            "evidence_schema": "1",
            "evidence_digest": {"os":"linux","architecture":"x86_64","bootloader":"grub",
                "root_filesystem":"ext4","kernel_count":1,"fstab_entries":2,
                "virtio_modules_loaded":true,"vm_tools":[],"selinux":"off"},
            "scores": {"boot": 90.0, "migration": 90.0, "readiness": "ready"},
            "critical_blockers": [], "recommended_actions": [],
            "fix_plan": {"profile":"p","operation_count":0,"sha256":"x","operation_ids":[]},
            "windows": {"is_windows": false, "bitlocker_blocker": false,
                "bitlocker_detected": false, "rdp_enabled": false,
                "virtio_driver_count": 0, "windows_offline_ready": false},
            "suite": {"export":"","convert_deploy":"","assurance":"","next_step":""},
            "hard_blocked": false
        });
        if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            for (k, x) in e {
                o.insert(k.clone(), x.clone());
            }
        }
        serde_json::from_value(v).expect("fixture passport")
    }

    fn blocker(id: &str) -> serde_json::Value {
        json!({"id": id, "title": format!("t-{id}"), "message": "m", "severity": "blocker"})
    }

    #[test]
    fn identical_is_unchanged() {
        let p = passport(json!({}));
        let d = diff_passports(&p, &p, &PassportDiffOptions::default());
        assert_eq!(d.verdict, DriftVerdict::Unchanged);
        assert!(d.same_image && !d.has_regression());
        assert!(d.windows_changes.is_empty());
        assert_eq!(d.schema, PASSPORT_DIFF_SCHEMA);
    }

    #[test]
    fn new_blocker_and_score_drop_regress() {
        let old = passport(json!({}));
        let new = passport(json!({
            "scores": {"boot": 60.0, "migration": 40.0, "readiness": "blocked"},
            "critical_blockers": [blocker("MIG-W-001")],
            "hard_blocked": true,
            "fix_plan": {"profile":"p","operation_count":1,"sha256":"y","operation_ids":["op1"]}
        }));
        let d = diff_passports(&old, &new, &PassportDiffOptions::default());
        assert_eq!(d.verdict, DriftVerdict::Regressed);
        assert_eq!(d.blockers_added.len(), 1);
        assert_eq!(d.boot.delta, -30.0);
        assert_eq!(d.plan_ops_added, vec!["op1"]);
        assert!(d.hard_blocked.as_ref().unwrap().new);
        assert!(d.regressions.len() >= 4);
        assert!(d.render_text().contains("[-] new blocker MIG-W-001"));
    }

    #[test]
    fn resolved_blocker_improves() {
        let old = passport(json!({
            "scores": {"boot": 50.0, "migration": 50.0, "readiness": "blocked"},
            "critical_blockers": [blocker("B1")], "hard_blocked": true}));
        let new = passport(json!({}));
        let d = diff_passports(&old, &new, &PassportDiffOptions::default());
        assert_eq!(d.verdict, DriftVerdict::Improved);
        assert_eq!(d.blockers_resolved[0].id, "B1");
        assert!(!d.has_regression());
    }

    #[test]
    fn tolerance_absorbs_small_drop_and_mixed_detected() {
        let old = passport(json!({}));
        let new = passport(json!({
            "scores": {"boot": 89.0, "migration": 95.0, "readiness": "ready"}}));
        let d = diff_passports(
            &old,
            &new,
            &PassportDiffOptions {
                score_tolerance: 2.0,
            },
        );
        assert_eq!(d.verdict, DriftVerdict::Improved);
        let d = diff_passports(&old, &new, &PassportDiffOptions::default());
        assert_eq!(d.verdict, DriftVerdict::Mixed);
    }

    #[test]
    fn windows_flag_and_image_changes_reported() {
        let old = passport(json!({}));
        let mut new = passport(json!({"target": "proxmox"}));
        new.windows.rdp_enabled = true;
        new.image.size_bytes = 11;
        let d = diff_passports(&old, &new, &PassportDiffOptions::default());
        assert!(!d.same_image);
        assert_eq!(d.target.as_ref().unwrap().new, "proxmox");
        assert!(d.windows_changes.contains_key("rdp_enabled"));
        assert_eq!(d.verdict, DriftVerdict::Unchanged);
    }
}
