// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! SARIF 2.1.0 export of an audit report, for code-scanning dashboards and
//! other SARIF consumers.
//!
//! Locations are guest paths relative to the guest root (`uriBaseId`
//! `GUESTROOT`). Waived findings are included with a SARIF `suppressions`
//! entry so a dashboard shows them as suppressed instead of losing them.

use super::finding::{AuditReport, Confidence, Finding, Severity};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const SARIF_SCHEMA_URI: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";

fn level(s: Severity) -> &'static str {
    match s {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

/// SARIF `security-severity` is a 0-10 score used by GitHub to rank alerts.
fn security_severity(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "9.5",
        Severity::High => "8.0",
        Severity::Medium => "5.5",
        Severity::Low => "3.0",
        Severity::Info => "0.5",
    }
}

fn conf(c: Confidence) -> &'static str {
    match c {
        Confidence::Confirmed => "confirmed",
        Confidence::Likely => "likely",
        Confidence::Heuristic => "heuristic",
    }
}

fn result(f: &Finding, suppression: Option<&str>) -> Value {
    let mut r = Map::new();
    r.insert("ruleId".into(), json!(f.id));
    r.insert("level".into(), json!(level(f.severity)));
    let text = if f.detail.is_empty() {
        f.title.clone()
    } else {
        format!("{} {}", f.title, f.detail)
    };
    r.insert("message".into(), json!({ "text": text }));
    if let Some(loc) = &f.location {
        let (path, line) = match loc.rsplit_once(':') {
            Some((p, l)) if l.chars().all(|c| c.is_ascii_digit()) && !l.is_empty() => {
                (p, l.parse::<u64>().ok())
            }
            _ => (loc.as_str(), None),
        };
        let mut phys = json!({
            "artifactLocation": {
                "uri": path.trim_start_matches('/'),
                "uriBaseId": "GUESTROOT"
            }
        });
        if let Some(n) = line.filter(|n| *n > 0) {
            phys["region"] = json!({ "startLine": n });
        }
        r.insert("locations".into(), json!([{ "physicalLocation": phys }]));
    }
    r.insert(
        "partialFingerprints".into(),
        json!({ "guestkitFindingKey/v1": f.key() }),
    );
    r.insert(
        "properties".into(),
        json!({
            "confidence": conf(f.confidence),
            "category": f.category,
            "severity": format!("{:?}", f.severity).to_lowercase(),
            "subject": f.subject,
        }),
    );
    if let Some(why) = suppression {
        r.insert(
            "suppressions".into(),
            json!([{ "kind": "external", "justification": why }]),
        );
    }
    Value::Object(r)
}

/// Build the SARIF document.
pub fn to_sarif(report: &AuditReport) -> Value {
    // One rule entry per distinct id, taken from the first finding that uses it.
    let mut rules: BTreeMap<&str, &Finding> = BTreeMap::new();
    for f in report
        .findings
        .iter()
        .chain(report.waived.iter().map(|w| &w.finding))
    {
        rules.entry(f.id.as_str()).or_insert(f);
    }
    let rule_values: Vec<Value> = rules
        .values()
        .map(|f| {
            json!({
                "id": f.id,
                "name": f.id,
                "shortDescription": { "text": f.title },
                "help": { "text": f.remediation },
                "defaultConfiguration": { "level": level(f.severity) },
                "properties": {
                    "category": f.category,
                    "security-severity": security_severity(f.severity),
                    "tags": ["offline-image-audit", f.category],
                }
            })
        })
        .collect();

    let mut results: Vec<Value> = report.findings.iter().map(|f| result(f, None)).collect();
    results.extend(report.waived.iter().map(|w| {
        let mut why = w.reason.clone();
        if let Some(o) = &w.owner {
            why.push_str(&format!(" (owner: {o})"));
        }
        result(&w.finding, Some(&why))
    }));

    let notifications: Vec<Value> = report
        .notes
        .iter()
        .map(|n| json!({ "level": "note", "message": { "text": n } }))
        .collect();

    json!({
        "$schema": SARIF_SCHEMA_URI,
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "guestkit image-audit",
                "version": env!("CARGO_PKG_VERSION"),
                "informationUri": "https://github.com/zyvorai/zyvor-guestkit",
                "rules": rule_values,
            }},
            "originalUriBaseIds": {
                "GUESTROOT": { "description": { "text": format!("Root of the audited guest image {}", report.image) } }
            },
            "invocations": [{
                "executionSuccessful": true,
                "toolExecutionNotifications": notifications,
            }],
            "results": results,
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::waivers::WaivedFinding;

    fn sample() -> AuditReport {
        let mut r = AuditReport::new("disk.qcow2");
        r.push(
            Finding::new(
                "SSH-001",
                "hardening",
                Severity::High,
                Confidence::Confirmed,
                "Root login",
                "Set no.",
            )
            .detail("PermitRootLogin yes.")
            .at("/etc/ssh/sshd_config:12")
            .subject("PermitRootLogin"),
        );
        r.push(Finding::new(
            "BOOT-001",
            "boot",
            Severity::Low,
            Confidence::Heuristic,
            "No kernel",
            "Install one.",
        ));
        r.waived.push(WaivedFinding {
            finding: Finding::new(
                "SUDO-001",
                "hardening",
                Severity::Medium,
                Confidence::Likely,
                "NOPASSWD",
                "Remove.",
            )
            .at("/etc/sudoers"),
            reason: "ops tooling".into(),
            owner: Some("sec".into()),
            expires: None,
        });
        r.note("Match blocks not evaluated");
        r.finish();
        r
    }

    #[test]
    fn document_shape_and_levels() {
        let v = to_sarif(&sample());
        assert_eq!(v["version"], "2.1.0");
        let run = &v["runs"][0];
        assert_eq!(run["tool"]["driver"]["rules"].as_array().unwrap().len(), 3);
        let results = run["results"].as_array().unwrap();
        assert_eq!(results.len(), 3);
        let ssh = results.iter().find(|r| r["ruleId"] == "SSH-001").unwrap();
        assert_eq!(ssh["level"], "error");
        let loc = &ssh["locations"][0]["physicalLocation"];
        assert_eq!(loc["artifactLocation"]["uri"], "etc/ssh/sshd_config");
        assert_eq!(loc["artifactLocation"]["uriBaseId"], "GUESTROOT");
        assert_eq!(loc["region"]["startLine"], 12);
        assert_eq!(ssh["properties"]["confidence"], "confirmed");
        let boot = results.iter().find(|r| r["ruleId"] == "BOOT-001").unwrap();
        assert_eq!(boot["level"], "note");
        assert!(boot.get("locations").is_none());
        assert_eq!(
            run["invocations"][0]["toolExecutionNotifications"][0]["message"]["text"],
            "Match blocks not evaluated"
        );
    }

    #[test]
    fn waived_findings_carry_suppressions() {
        let v = to_sarif(&sample());
        let results = v["runs"][0]["results"].as_array().unwrap();
        let w = results.iter().find(|r| r["ruleId"] == "SUDO-001").unwrap();
        assert_eq!(w["suppressions"][0]["kind"], "external");
        assert_eq!(
            w["suppressions"][0]["justification"],
            "ops tooling (owner: sec)"
        );
        assert!(w["locations"][0]["physicalLocation"]
            .get("region")
            .is_none());
        let ssh = results.iter().find(|r| r["ruleId"] == "SSH-001").unwrap();
        assert!(ssh.get("suppressions").is_none());
    }

    #[test]
    fn empty_report_is_valid() {
        let mut r = AuditReport::new("x");
        r.finish();
        let v = to_sarif(&r);
        assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 0);
    }
}
