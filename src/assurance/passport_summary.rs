// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! One-screen summary of a Cutover Passport (`guestkit passport show`).
//!
//! Pure and offline: it reads the passport JSON only. It reports whether a
//! signature is *present*; it does not verify it (use `passport verify`).

use super::passport::{CutoverPassport, PassportFinding};
use crate::migration::ReadinessLevel;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Schema tag of [`PassportSummary`] documents.
pub const PASSPORT_SUMMARY_SCHEMA: &str = "guestkit.passport_summary/1";

/// Condensed view of a passport.
#[derive(Debug, Clone, Serialize)]
pub struct PassportSummary {
    pub schema: String,
    pub image: String,
    pub image_size_bytes: u64,
    pub target: String,
    pub tool_version: String,
    pub generated_at: String,
    /// Seconds since `generated_at` (None when it does not parse).
    pub age_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// `Some(true)` when `expires_at` is in the past.
    pub expired: Option<bool>,
    pub boot_score: f64,
    pub migration_score: f64,
    /// min(boot, migration), the value `passport verify --fail-below` gates on.
    pub gate_score: f64,
    pub readiness: ReadinessLevel,
    pub hard_blocked: bool,
    pub signed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_public_key: Option<String>,
    pub blockers: Vec<PassportFinding>,
    pub recommended_actions: usize,
    pub fix_plan_operations: usize,
    pub is_windows: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_passed: Option<bool>,
}

fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Summarize `p` as of `now`.
pub fn summarize_passport(p: &CutoverPassport, now: DateTime<Utc>) -> PassportSummary {
    PassportSummary {
        schema: PASSPORT_SUMMARY_SCHEMA.to_string(),
        image: p.image.path.clone(),
        image_size_bytes: p.image.size_bytes,
        target: p.target.clone(),
        tool_version: p.tool_version.clone(),
        generated_at: p.generated_at.clone(),
        age_seconds: parse_utc(&p.generated_at).map(|g| (now - g).num_seconds()),
        issuer: p.issuer.clone(),
        expires_at: p.expires_at.clone(),
        expired: p.expires_at.as_deref().and_then(parse_utc).map(|e| now > e),
        boot_score: p.scores.boot,
        migration_score: p.scores.migration,
        gate_score: p.scores.boot.min(p.scores.migration),
        readiness: p.scores.readiness,
        hard_blocked: p.hard_blocked,
        signed: p.signature.is_some(),
        signer_public_key: p.signature.as_ref().map(|s| s.public_key_hex.clone()),
        blockers: p.critical_blockers.clone(),
        recommended_actions: p.recommended_actions.len(),
        fix_plan_operations: p.fix_plan.operation_count,
        is_windows: p.windows.is_windows,
        policy_passed: p.policy.as_ref().map(|v| v.passed),
    }
}

fn human_age(secs: i64) -> String {
    let s = secs.max(0);
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m", s / 60),
        3600..=86399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

impl PassportSummary {
    /// Plain-text rendering, one screen.
    pub fn render_text(&self) -> String {
        let mut o = String::new();
        o.push_str(&format!(
            "Cutover Passport: {} -> {}\n",
            self.image, self.target
        ));
        let age = match self.age_seconds {
            Some(a) if a >= 0 => format!("{} ago", human_age(a)),
            Some(_) => "in the future".to_string(),
            None => "unparseable timestamp".to_string(),
        };
        o.push_str(&format!(
            "  generated: {} ({age}) by guestkit {}\n",
            self.generated_at, self.tool_version
        ));
        if let Some(i) = &self.issuer {
            o.push_str(&format!("  issuer:    {i}\n"));
        }
        if let Some(e) = &self.expires_at {
            let state = match self.expired {
                Some(true) => "EXPIRED",
                Some(false) => "valid",
                None => "unparseable",
            };
            o.push_str(&format!("  expires:   {e} ({state})\n"));
        }
        o.push_str(&format!(
            "  scores:    boot {:.0}, migration {:.0}, gate {:.0}  ({:?}{})\n",
            self.boot_score,
            self.migration_score,
            self.gate_score,
            self.readiness,
            if self.hard_blocked {
                ", HARD BLOCKED"
            } else {
                ""
            }
        ));
        match &self.signer_public_key {
            Some(k) => {
                let short: String = k.chars().take(16).collect();
                o.push_str(&format!(
                    "  signature: present, key {short}... (not verified; run `passport verify`)\n"
                ));
            }
            None => o.push_str("  signature: none\n"),
        }
        if let Some(p) = self.policy_passed {
            o.push_str(&format!(
                "  policy:    {}\n",
                if p { "passed" } else { "FAILED" }
            ));
        }
        o.push_str(&format!(
            "  fix plan:  {} operation(s); {} recommended action(s){}\n",
            self.fix_plan_operations,
            self.recommended_actions,
            if self.is_windows {
                "; Windows guest"
            } else {
                ""
            }
        ));
        if self.blockers.is_empty() {
            o.push_str("  blockers:  none\n");
        } else {
            o.push_str(&format!("  blockers:  {}\n", self.blockers.len()));
            for b in &self.blockers {
                o.push_str(&format!("    - [{}] {}: {}\n", b.id, b.title, b.message));
            }
        }
        o
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
            "scores": {"boot": 90.0, "migration": 70.0, "readiness": "ready_with_warnings"},
            "critical_blockers": [], "recommended_actions": ["a", "b"],
            "fix_plan": {"profile":"p","operation_count":3,"sha256":"x","operation_ids":[]},
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

    fn at(s: &str) -> DateTime<Utc> {
        parse_utc(s).unwrap()
    }

    #[test]
    fn basic_fields_and_gate_score() {
        let s = summarize_passport(&passport(json!({})), at("2026-01-01T02:00:00Z"));
        assert_eq!(s.gate_score, 70.0);
        assert_eq!(s.age_seconds, Some(7200));
        assert_eq!(s.fix_plan_operations, 3);
        assert_eq!(s.recommended_actions, 2);
        assert!(!s.signed && s.expired.is_none());
        assert_eq!(s.schema, PASSPORT_SUMMARY_SCHEMA);
    }

    #[test]
    fn expiry_is_relative_to_now() {
        let p = passport(json!({"expires_at": "2026-01-02T00:00:00Z"}));
        assert_eq!(
            summarize_passport(&p, at("2026-01-01T12:00:00Z")).expired,
            Some(false)
        );
        let s = summarize_passport(&p, at("2026-01-03T00:00:00Z"));
        assert_eq!(s.expired, Some(true));
        assert!(s.render_text().contains("(EXPIRED)"));
    }

    #[test]
    fn signature_presence_is_reported_not_verified() {
        let p = passport(json!({"signature": {
            "algorithm": "ed25519",
            "public_key_hex": "abcdef0123456789abcdef0123456789",
            "signature_hex": "00"}}));
        let s = summarize_passport(&p, at("2026-01-01T00:00:00Z"));
        assert!(s.signed);
        let t = s.render_text();
        assert!(t.contains("key abcdef0123456789..."));
        assert!(t.contains("not verified"));
    }

    #[test]
    fn blockers_and_hard_block_render() {
        let p = passport(json!({
            "hard_blocked": true,
            "critical_blockers": [{"id":"MIG-W-001","title":"BitLocker","message":"on","severity":"blocker"}]
        }));
        let t = summarize_passport(&p, at("2026-01-01T00:00:00Z")).render_text();
        assert!(t.contains("HARD BLOCKED"));
        assert!(t.contains("- [MIG-W-001] BitLocker: on"));
    }

    #[test]
    fn bad_timestamp_does_not_panic() {
        let p = passport(json!({"generated_at": "yesterday"}));
        let s = summarize_passport(&p, at("2026-01-01T00:00:00Z"));
        assert_eq!(s.age_seconds, None);
        assert!(s.render_text().contains("unparseable timestamp"));
    }

    #[test]
    fn human_age_buckets() {
        assert_eq!(human_age(5), "5s");
        assert_eq!(human_age(120), "2m");
        assert_eq!(human_age(7200), "2h");
        assert_eq!(human_age(172800), "2d");
    }
}
