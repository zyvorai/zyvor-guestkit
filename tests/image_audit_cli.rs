// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! CLI tests for `guestkit image-audit` on an already mounted guest root
//! directory (no image, root, KVM or libguestfs needed): identity scope, SARIF
//! output, custom rules, waivers and baselines.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn guestkit() -> Command {
    Command::new(env!("CARGO_BIN_EXE_guestkit"))
}

fn put(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel.trim_start_matches('/'));
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

/// A guest root with an identity problem (history with a secret, machine-id)
/// and a hardening problem (root login).
fn dirty_guest() -> TempDir {
    let d = TempDir::new().unwrap();
    put(
        d.path(),
        "/etc/machine-id",
        "0123456789abcdef0123456789abcdef\n",
    );
    put(d.path(), "/root/.bash_history", "mysql -pHunter2Secret\n");
    put(d.path(), "/etc/ssh/sshd_config", "PermitRootLogin yes\n");
    d
}

fn audit_json(args: &[&str]) -> Value {
    let out = guestkit().arg("image-audit").args(args).output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn ids(v: &Value) -> Vec<String> {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn identity_scope_finds_baked_in_state_without_leaking_it() {
    let d = dirty_guest();
    let out = guestkit()
        .args(["image-audit", "--scope", "identity", "-o", "json"])
        .arg(d.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("Hunter2Secret"));
    assert!(!text.contains("0123456789abcdef0123456789abcdef"));
    let v: Value = serde_json::from_str(&text).unwrap();
    let got = ids(&v);
    assert!(got.contains(&"IDENT-001".to_string()), "{got:?}");
    assert!(got.contains(&"IDENT-003".to_string()), "{got:?}");
    // Identity scope must not run the hardening rules.
    assert!(!got.iter().any(|i| i.starts_with("SSH-")));
}

#[test]
fn all_scope_includes_identity_and_hardening() {
    let d = dirty_guest();
    let v = audit_json(&["-o", "json", d.path().to_str().unwrap()]);
    let got = ids(&v);
    assert!(got.iter().any(|i| i.starts_with("SSH-")), "{got:?}");
    assert!(got.iter().any(|i| i.starts_with("IDENT-")), "{got:?}");
    assert!(v["notes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n.as_str().unwrap().contains("directory tree")));
}

#[test]
fn fail_on_exits_2_and_unknown_inputs_are_rejected() {
    let d = dirty_guest();
    guestkit()
        .args(["image-audit", "--scope", "identity", "--fail-on", "medium"])
        .arg(d.path())
        .assert()
        .code(2);
    guestkit()
        .args(["image-audit", "--scope", "nonsense"])
        .arg(d.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("identity"));
    guestkit()
        .args(["image-audit", "-o", "jsno"])
        .arg(d.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown output format"));
}

#[test]
fn sarif_output_is_valid_and_maps_findings() {
    let d = dirty_guest();
    let out = guestkit()
        .args(["image-audit", "--scope", "all", "-o", "sarif"])
        .arg(d.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["version"], "2.1.0");
    let results = v["runs"][0]["results"].as_array().unwrap();
    let ssh = results
        .iter()
        .find(|r| r["ruleId"] == "SSH-001")
        .expect("SSH-001 result");
    assert_eq!(ssh["level"], "error");
    assert_eq!(
        ssh["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "etc/ssh/sshd_config"
    );
    assert!(!String::from_utf8_lossy(&out.stdout).contains("Hunter2Secret"));
}

#[test]
fn custom_rules_run_and_bad_rule_files_fail_before_scanning() {
    let d = dirty_guest();
    let rf = TempDir::new().unwrap();
    let good = rf.path().join("rules.yaml");
    fs::write(
        &good,
        "rules:\n  - id: ORG-001\n    title: Banner missing\n    severity: medium\n    kind: line-missing\n    path: /etc/issue\n    pattern: Authorized use only\n    remediation: Install banner.\n  - id: ORG-002\n    title: History present\n    severity: low\n    kind: file-present\n    path: /root/.bash_history\n",
    )
    .unwrap();
    let v = audit_json(&[
        "--scope",
        "boot",
        "-o",
        "json",
        "--rules",
        good.to_str().unwrap(),
        d.path().to_str().unwrap(),
    ]);
    let got = ids(&v);
    assert!(got.contains(&"ORG-001".to_string()), "{got:?}");
    assert!(got.contains(&"ORG-002".to_string()));

    let bad = rf.path().join("bad.yaml");
    fs::write(
        &bad,
        "rules:\n  - id: X\n    title: t\n    severity: loud\n    kind: file-present\n    path: /a\n",
    )
    .unwrap();
    guestkit()
        .args(["image-audit", "--rules"])
        .arg(&bad)
        .arg(d.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("severity"));
}

#[test]
fn waivers_suppress_expire_and_keep_an_audit_trail() {
    let d = dirty_guest();
    let wf = TempDir::new().unwrap();

    let active = wf.path().join("active.yaml");
    fs::write(
        &active,
        "waivers:\n  - id: IDENT-*\n    reason: lab image, reviewed\n    owner: sec\n    expires: 2999-12-31\n",
    )
    .unwrap();
    let v = audit_json(&[
        "--scope",
        "identity",
        "-o",
        "json",
        "--waivers",
        active.to_str().unwrap(),
        d.path().to_str().unwrap(),
    ]);
    assert!(ids(&v).is_empty(), "{:?}", ids(&v));
    let waived = v["waived"].as_array().unwrap();
    assert!(waived.len() >= 2);
    assert_eq!(waived[0]["reason"], "lab image, reviewed");
    // --fail-on is evaluated after waivers.
    guestkit()
        .args([
            "image-audit",
            "--scope",
            "identity",
            "--fail-on",
            "low",
            "--waivers",
        ])
        .arg(&active)
        .arg(d.path())
        .assert()
        .success();

    let expired = wf.path().join("expired.yaml");
    fs::write(
        &expired,
        "waivers:\n  - id: IDENT-*\n    reason: temporary\n    expires: 2000-01-01\n",
    )
    .unwrap();
    guestkit()
        .args([
            "image-audit",
            "--scope",
            "identity",
            "--fail-on",
            "low",
            "--waivers",
        ])
        .arg(&expired)
        .arg(d.path())
        .assert()
        .code(2);
    let v = audit_json(&[
        "--scope",
        "identity",
        "-o",
        "json",
        "--waivers",
        expired.to_str().unwrap(),
        d.path().to_str().unwrap(),
    ]);
    assert!(!ids(&v).is_empty());
    assert!(v["notes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n.as_str().unwrap().contains("expired on 2000-01-01")));

    let no_reason = wf.path().join("noreason.yaml");
    fs::write(&no_reason, "waivers:\n  - id: IDENT-001\n").unwrap();
    guestkit()
        .args(["image-audit", "--waivers"])
        .arg(&no_reason)
        .arg(d.path())
        .assert()
        .failure();
}

#[test]
fn baseline_hides_known_findings_and_shows_only_new_ones() {
    let d = dirty_guest();
    let work = TempDir::new().unwrap();
    let base = work.path().join("base.json");
    let out = guestkit()
        .args(["image-audit", "--scope", "identity", "-o", "json"])
        .arg(d.path())
        .output()
        .unwrap();
    fs::write(&base, &out.stdout).unwrap();

    let v = audit_json(&[
        "--scope",
        "identity",
        "-o",
        "json",
        "--baseline",
        base.to_str().unwrap(),
        d.path().to_str().unwrap(),
    ]);
    assert!(ids(&v).is_empty());
    assert!(!v["waived"].as_array().unwrap().is_empty());

    // A new problem appears after the baseline was taken.
    put(d.path(), "/var/mail/root", "mail\n");
    let v = audit_json(&[
        "--scope",
        "identity",
        "-o",
        "json",
        "--baseline",
        base.to_str().unwrap(),
        d.path().to_str().unwrap(),
    ]);
    assert_eq!(ids(&v), vec!["IDENT-007"]);

    // A baseline that is not an audit report is an error, not "no baseline".
    let junk = work.path().join("junk.json");
    fs::write(&junk, "{\"hello\": 1}").unwrap();
    guestkit()
        .args(["image-audit", "--baseline"])
        .arg(&junk)
        .arg(d.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a saved"));
}

#[test]
fn audit_diff_works_on_directories_with_identity_findings() {
    let old = TempDir::new().unwrap();
    put(old.path(), "/etc/machine-id", "");
    let new = dirty_guest();
    let out = guestkit()
        .args(["image-audit-diff", "--scope", "identity", "-o", "json"])
        .arg(old.path())
        .arg(new.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["verdict"], "regressed");
    assert!(!v["added"].as_array().unwrap().is_empty());
    guestkit()
        .args([
            "image-audit-diff",
            "--scope",
            "identity",
            "--fail-on-regression",
            "medium",
        ])
        .arg(old.path())
        .arg(new.path())
        .assert()
        .code(2);
}
