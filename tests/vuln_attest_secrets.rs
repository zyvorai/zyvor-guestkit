// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Hermetic CLI tests for `guestkit cve` and `guestkit attest`: fixture
//! inventories and feeds, no network, no guest images.

use assert_cmd::Command;
use serde_json::{json, Value};
use std::path::Path;

fn guestkit(cache: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_guestkit"));
    c.env("GUESTKIT_CACHE_DIR", cache)
        .env("GUESTKIT_OFFLINE", "1");
    c
}

fn write_inventory(dir: &Path) -> std::path::PathBuf {
    let pkg = |n: &str, v: &str, id: &str, sev: &str, fix: Option<&str>| {
        json!({
            "name": n, "version": v, "package_type": "rpm", "license": null,
            "size": null, "installed_date": null, "files": [], "dependencies": [],
            "vulnerabilities": [{
                "cve": id, "severity": sev, "score": null,
                "description": "fixture", "fixed_version": fix
            }],
            "checksum": null
        })
    };
    let inv = json!({
        "image_path": "fixture.img", "scanned_at": "2026-01-01T00:00:00Z",
        "os_name": "fixture", "os_version": "1", "architecture": "x86_64",
        "packages": [
            pkg("alpha", "1.0", "CVE-2025-1111", "critical", Some("1.1")),
            pkg("beta", "2.0", "CVE-2025-2222", "medium", None),
        ],
        "statistics": {"total_packages": 2, "total_size": 0,
                       "vulnerabilities": {}, "licenses": {}}
    });
    let p = dir.join("inv.json");
    std::fs::write(&p, serde_json::to_string_pretty(&inv).unwrap()).unwrap();
    p
}

fn stdout_json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).expect("stdout is JSON")
}

#[test]
fn cve_fail_on_exit_codes() {
    let td = tempfile::tempdir().unwrap();
    let inv = write_inventory(td.path());
    let cache = td.path().join("cache");
    guestkit(&cache).arg("cve").arg(&inv).assert().code(0);
    guestkit(&cache)
        .args(["cve", "--fail-on", "critical"])
        .arg(&inv)
        .assert()
        .code(2);
    guestkit(&cache)
        .args(["cve", "--fail-on", "bogus"])
        .arg(&inv)
        .assert()
        .failure();
}

#[test]
fn cve_vex_suppression_changes_gate_and_openvex_output() {
    let td = tempfile::tempdir().unwrap();
    let inv = write_inventory(td.path());
    let vex = td.path().join("vex.json");
    std::fs::write(
        &vex,
        r#"{"@context":"https://openvex.dev/ns/v0.2.0","statements":[
            {"vulnerability":{"name":"CVE-2025-1111"},"status":"not_affected",
             "justification":"vulnerable_code_not_in_execute_path"}]}"#,
    )
    .unwrap();
    let cache = td.path().join("cache");
    guestkit(&cache)
        .args(["cve", "--fail-on", "high", "--vex"])
        .arg(&vex)
        .arg(&inv)
        .assert()
        .code(0);
    let out = guestkit(&cache)
        .args(["cve", "-f", "openvex", "--vex"])
        .arg(&vex)
        .arg(&inv)
        .output()
        .unwrap();
    let doc = stdout_json(&out);
    let st = doc["statements"].as_array().unwrap();
    let by = |id: &str| {
        st.iter()
            .find(|s| s["vulnerability"]["name"] == id)
            .unwrap()
    };
    assert_eq!(by("CVE-2025-1111")["status"], "not_affected");
    assert_eq!(by("CVE-2025-2222")["status"], "affected");
}

#[test]
fn cve_csaf_and_epss_kev_enrichment() {
    let td = tempfile::tempdir().unwrap();
    let inv = write_inventory(td.path());
    let epss = td.path().join("epss.csv");
    std::fs::write(
        &epss,
        "#model_version:v1\ncve,epss,percentile\nCVE-2025-2222,0.80,0.97\n",
    )
    .unwrap();
    let kev = td.path().join("kev.json");
    std::fs::write(&kev, r#"{"vulnerabilities":[{"cveID":"CVE-2025-2222"}]}"#).unwrap();
    let cache = td.path().join("cache");

    let out = guestkit(&cache)
        .args(["cve", "-f", "csaf"])
        .arg(&inv)
        .output()
        .unwrap();
    let doc = stdout_json(&out);
    assert_eq!(doc["document"]["category"], "csaf_vex");
    assert_eq!(doc["vulnerabilities"].as_array().unwrap().len(), 2);

    let out = guestkit(&cache)
        .args(["cve", "-f", "json", "--epss"])
        .arg(&epss)
        .arg("--kev")
        .arg(&kev)
        .arg(&inv)
        .output()
        .unwrap();
    let v = stdout_json(&out);
    let beta = v
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "CVE-2025-2222")
        .unwrap();
    assert_eq!(beta["epss"], 0.8);
    assert_eq!(beta["kev"], true);

    guestkit(&cache)
        .args(["cve", "--fail-on-kev", "--kev"])
        .arg(&kev)
        .arg(&inv)
        .assert()
        .code(2);
}

#[test]
fn cve_db_export_import_roundtrip_offline() {
    let td = tempfile::tempdir().unwrap();
    let src_cache = td.path().join("src");
    let feeds = src_cache.join("feeds");
    let cve_dir = src_cache.join("cve");
    std::fs::create_dir_all(&feeds).unwrap();
    std::fs::create_dir_all(&cve_dir).unwrap();
    std::fs::write(
        cve_dir.join("zlib_1_2_11.json"),
        r#"[{"cve":"CVE-2018-25032","severity":"high","score":7.5,"description":"zlib","fixed_version":"1.2.12"}]"#,
    )
    .unwrap();
    std::fs::write(feeds.join("kev.json"), r#"["CVE-2018-25032"]"#).unwrap();
    let bundle = td.path().join("db.json");
    guestkit(&src_cache)
        .arg("cve")
        .arg("--db-export")
        .arg(&bundle)
        .assert()
        .success();

    let dst = td.path().join("dst");
    guestkit(&dst)
        .arg("cve")
        .arg("--db-import")
        .arg(&bundle)
        .assert()
        .success();
    assert!(dst.join("cve/zlib_1_2_11.json").exists());
    assert!(dst.join("feeds/kev.json").exists());

    // Tampered bundle is refused.
    let t = std::fs::read_to_string(&bundle)
        .unwrap()
        .replace("CVE-2018-25032", "CVE-2018-99999");
    let bad = td.path().join("bad.json");
    std::fs::write(&bad, t).unwrap();
    guestkit(&dst)
        .arg("cve")
        .arg("--db-import")
        .arg(&bad)
        .assert()
        .failure();
}

#[cfg(feature = "agent")]
mod attest {
    use super::*;

    const SEED_HEX: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
    const PUB_HEX: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

    #[test]
    fn create_verify_and_detect_image_tamper() {
        let td = tempfile::tempdir().unwrap();
        let cache = td.path().join("cache");
        let img = td.path().join("disk.img");
        std::fs::write(&img, b"fixture image bytes").unwrap();
        let sbom = td.path().join("sbom.json");
        std::fs::write(
            &sbom,
            r#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
        let key = td.path().join("k.key");
        std::fs::write(&key, SEED_HEX).unwrap();
        let env = td.path().join("att.json");

        guestkit(&cache)
            .args(["attest", "create"])
            .arg(&img)
            .arg("--sbom")
            .arg(&sbom)
            .arg("--sign-key")
            .arg(&key)
            .arg("-o")
            .arg(&env)
            .assert()
            .success();
        let e: Value = serde_json::from_str(&std::fs::read_to_string(&env).unwrap()).unwrap();
        assert_eq!(e["payloadType"], "application/vnd.in-toto+json");

        guestkit(&cache)
            .args(["attest", "verify"])
            .arg(&env)
            .arg("--image")
            .arg(&img)
            .args(["--public-key", PUB_HEX])
            .assert()
            .success();

        // no trusted key supplied: refuse
        guestkit(&cache)
            .args(["attest", "verify"])
            .arg(&env)
            .assert()
            .failure();

        // image changed after attestation
        std::fs::write(&img, b"different bytes").unwrap();
        guestkit(&cache)
            .args(["attest", "verify"])
            .arg(&env)
            .arg("--image")
            .arg(&img)
            .args(["--public-key", PUB_HEX])
            .assert()
            .failure();
    }
}
