// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Vulnerability reporting: VEX suppression, EPSS / CISA-KEV enrichment from
//! locally cached feed files, OpenVEX and CSAF output, `--fail-on` gating, and
//! an offline vulnerability DB bundle (export / import) for air-gapped hosts.
//!
//! Nothing here touches the network. Feeds are plain files supplied by the
//! operator (or cached under `<cache-root>/feeds/`).

use super::{cve, Inventory, VulnerabilityInfo};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

/// A VEX statement that matched a finding.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VexMatch {
    pub status: String,
    pub justification: Option<String>,
}

/// One (package, vulnerability) pair with enrichment and VEX disposition.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub package: String,
    pub version: String,
    pub package_type: String,
    pub id: String,
    pub severity: String,
    pub score: Option<f64>,
    pub description: String,
    pub fixed_version: Option<String>,
    pub epss: Option<f64>,
    pub epss_percentile: Option<f64>,
    pub kev: bool,
    /// Set when a supplied VEX statement suppresses this finding
    /// (`not_affected` or `fixed`).
    pub suppressed_by: Option<VexMatch>,
}

impl Finding {
    pub fn purl(&self) -> String {
        purl_for(&self.package_type, &self.package, &self.version)
    }
    pub fn is_active(&self) -> bool {
        self.suppressed_by.is_none()
    }
}

/// Package URL for an inventory package.
pub fn purl_for(package_type: &str, name: &str, version: &str) -> String {
    let ty = match package_type.to_ascii_lowercase().as_str() {
        "deb" | "dpkg" => "deb",
        "rpm" => "rpm",
        "apk" => "apk",
        "pip" | "pypi" | "python" => "pypi",
        "npm" => "npm",
        "gem" => "gem",
        _ => "generic",
    };
    format!("pkg:{}/{}@{}", ty, name, version)
}

/// Severity to use for gating: the recorded severity, else derived from CVSS.
pub fn effective_severity(v: &VulnerabilityInfo) -> String {
    let s = v.severity.to_ascii_lowercase();
    if cve::severity_rank(&s) > 0 {
        return s;
    }
    match v.score {
        Some(x) if x >= 9.0 => "critical",
        Some(x) if x >= 7.0 => "high",
        Some(x) if x >= 4.0 => "medium",
        Some(x) if x > 0.0 => "low",
        _ => "unknown",
    }
    .to_string()
}

/// Collect findings from an inventory. Packages that already carry
/// vulnerabilities (inventory generated with `--include-cves`) use those;
/// others are looked up (static table + OSV cache, honouring offline mode).
pub fn collect_findings(inv: &Inventory) -> Vec<Finding> {
    let mut out = Vec::new();
    for pkg in &inv.packages {
        let vulns = if pkg.vulnerabilities.is_empty() {
            cve::lookup_cves(&pkg.name, &pkg.version).unwrap_or_default()
        } else {
            pkg.vulnerabilities.clone()
        };
        for v in vulns {
            out.push(Finding {
                package: pkg.name.clone(),
                version: pkg.version.clone(),
                package_type: pkg.package_type.clone(),
                id: v.cve.clone(),
                severity: effective_severity(&v),
                score: v.score,
                description: v.description.clone(),
                fixed_version: v.fixed_version.clone(),
                epss: None,
                epss_percentile: None,
                kev: false,
                suppressed_by: None,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// VEX input (OpenVEX)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VexStatement {
    pub ids: Vec<String>,
    pub products: Vec<String>,
    pub status: String,
    pub justification: Option<String>,
}

/// Parse an OpenVEX document (v0.2 object form or the older string form).
pub fn parse_openvex(text: &str) -> Result<Vec<VexStatement>> {
    let doc: Value = serde_json::from_str(text).context("VEX file is not valid JSON")?;
    let stmts = doc
        .get("statements")
        .and_then(|s| s.as_array())
        .context("not an OpenVEX document: missing `statements` array")?;
    let mut out = Vec::new();
    for s in stmts {
        let mut ids = Vec::new();
        match s.get("vulnerability") {
            Some(Value::String(n)) => ids.push(n.clone()),
            Some(Value::Object(o)) => {
                for k in ["name", "@id"] {
                    if let Some(n) = o.get(k).and_then(|v| v.as_str()) {
                        ids.push(n.to_string());
                    }
                }
                if let Some(al) = o.get("aliases").and_then(|a| a.as_array()) {
                    ids.extend(al.iter().filter_map(|a| a.as_str().map(String::from)));
                }
            }
            _ => continue,
        }
        let mut products = Vec::new();
        if let Some(ps) = s.get("products").and_then(|p| p.as_array()) {
            for p in ps {
                match p {
                    Value::String(x) => products.push(x.clone()),
                    Value::Object(o) => {
                        if let Some(x) = o.get("@id").and_then(|v| v.as_str()) {
                            products.push(x.to_string());
                        }
                        if let Some(x) = o
                            .get("identifiers")
                            .and_then(|i| i.get("purl"))
                            .and_then(|v| v.as_str())
                        {
                            products.push(x.to_string());
                        }
                        if let Some(subs) = o.get("subcomponents").and_then(|v| v.as_array()) {
                            for sc in subs {
                                if let Some(x) = sc.get("@id").and_then(|v| v.as_str()) {
                                    products.push(x.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let status = s
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let justification = s
            .get("justification")
            .and_then(|v| v.as_str())
            .map(String::from);
        out.push(VexStatement {
            ids,
            products,
            status,
            justification,
        });
    }
    Ok(out)
}

pub fn load_vex(path: &Path) -> Result<Vec<VexStatement>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("read VEX {}", path.display()))?;
    parse_openvex(&text).with_context(|| format!("parse VEX {}", path.display()))
}

fn product_matches(product: &str, f: &Finding) -> bool {
    if product == "*" || product == f.package {
        return true;
    }
    if let Some(rest) = product.strip_prefix("pkg:") {
        // pkg:<type>/<name>[@version][?qualifiers]
        let after_type = rest.split_once('/').map(|x| x.1).unwrap_or(rest);
        let no_q = after_type.split('?').next().unwrap_or(after_type);
        let (name, ver) = match no_q.split_once('@') {
            Some((n, v)) => (n, Some(v)),
            None => (no_q, None),
        };
        if name != f.package {
            return false;
        }
        return ver.map(|v| v == f.version).unwrap_or(true);
    }
    false
}

/// Apply VEX statements. Later matching statements override earlier ones;
/// `not_affected` and `fixed` suppress, anything else leaves the finding active.
pub fn apply_vex(findings: &mut [Finding], statements: &[VexStatement]) {
    for f in findings.iter_mut() {
        let mut disposition: Option<VexMatch> = None;
        for st in statements {
            if !st.ids.iter().any(|i| i.eq_ignore_ascii_case(&f.id)) {
                continue;
            }
            if !st.products.is_empty() && !st.products.iter().any(|p| product_matches(p, f)) {
                continue;
            }
            disposition = match st.status.as_str() {
                "not_affected" | "fixed" => Some(VexMatch {
                    status: st.status.clone(),
                    justification: st.justification.clone(),
                }),
                _ => None,
            };
        }
        f.suppressed_by = disposition;
    }
}

// ---------------------------------------------------------------------------
// Enrichment feeds (EPSS, CISA KEV)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct Enrichment {
    /// CVE -> (epss probability, percentile)
    pub epss: BTreeMap<String, (f64, f64)>,
    pub kev: HashSet<String>,
}

/// Directory for cached feed files (`<cache-root>/feeds`).
pub fn feeds_dir() -> PathBuf {
    match crate::core::cache_root::guestkit_cache_root() {
        Some(root) => root.join("feeds"),
        None => PathBuf::from("/tmp").join("guestkit").join("feeds"),
    }
}

fn json_num(x: &Value) -> Option<f64> {
    x.as_f64()
        .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
}

/// Parse EPSS data: FIRST CSV (`cve,epss,percentile`, `#` comment lines),
/// FIRST API JSON (`{"data":[{"cve","epss","percentile"}]}`), or a JSON map
/// `{"CVE-..":{"epss":x,"percentile":y}}`.
pub fn parse_epss(text: &str) -> Result<BTreeMap<String, (f64, f64)>> {
    let mut out = BTreeMap::new();
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        let v: Value = serde_json::from_str(trimmed).context("EPSS JSON")?;
        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
            for e in arr {
                if let (Some(c), Some(s)) = (
                    e.get("cve").and_then(|c| c.as_str()),
                    e.get("epss").and_then(json_num),
                ) {
                    let p = e.get("percentile").and_then(json_num).unwrap_or(0.0);
                    out.insert(c.to_ascii_uppercase(), (s, p));
                }
            }
        } else if let Some(map) = v.as_object() {
            for (c, e) in map {
                if let Some(s) = e.get("epss").and_then(json_num) {
                    let p = e.get("percentile").and_then(json_num).unwrap_or(0.0);
                    out.insert(c.to_ascii_uppercase(), (s, p));
                }
            }
        }
        return Ok(out);
    }
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.to_ascii_lowercase().starts_with("cve,")
        {
            continue;
        }
        let mut it = line.split(',');
        let (Some(c), Some(s)) = (it.next(), it.next()) else {
            continue;
        };
        let Ok(score) = s.trim().parse::<f64>() else {
            continue;
        };
        let pct = it
            .next()
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        out.insert(c.trim().to_ascii_uppercase(), (score, pct));
    }
    Ok(out)
}

/// Parse a KEV list: CISA catalog JSON (`vulnerabilities[].cveID`), a JSON
/// array of CVE ids, or one CVE id per line.
pub fn parse_kev(text: &str) -> Result<HashSet<String>> {
    let trimmed = text.trim_start();
    let mut out = HashSet::new();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(trimmed).context("KEV JSON")?;
        if let Some(arr) = v.get("vulnerabilities").and_then(|a| a.as_array()) {
            for e in arr {
                if let Some(c) = e.get("cveID").and_then(|c| c.as_str()) {
                    out.insert(c.to_ascii_uppercase());
                }
            }
        } else if let Some(arr) = v.as_array() {
            for e in arr {
                if let Some(c) = e.as_str() {
                    out.insert(c.to_ascii_uppercase());
                }
            }
        }
        return Ok(out);
    }
    for line in trimmed.lines() {
        let l = line.trim();
        if l.to_ascii_uppercase().starts_with("CVE-") {
            out.insert(l.to_ascii_uppercase());
        }
    }
    Ok(out)
}

impl Enrichment {
    pub fn load(epss: Option<&Path>, kev: Option<&Path>) -> Result<Self> {
        let mut e = Enrichment::default();
        if let Some(p) = epss {
            let t = std::fs::read_to_string(p)
                .with_context(|| format!("read EPSS feed {}", p.display()))?;
            e.epss = parse_epss(&t)?;
        }
        if let Some(p) = kev {
            let t = std::fs::read_to_string(p)
                .with_context(|| format!("read KEV feed {}", p.display()))?;
            e.kev = parse_kev(&t)?;
        }
        Ok(e)
    }

    /// Load feeds imported into / cached under `dir`, when present.
    pub fn from_cache_dir(dir: &Path) -> Self {
        let mut e = Enrichment::default();
        for name in ["epss.json", "epss.csv"] {
            if let Ok(t) = std::fs::read_to_string(dir.join(name)) {
                if let Ok(m) = parse_epss(&t) {
                    e.epss = m;
                    break;
                }
            }
        }
        if let Ok(t) = std::fs::read_to_string(dir.join("kev.json")) {
            if let Ok(k) = parse_kev(&t) {
                e.kev = k;
            }
        }
        e
    }

    /// Fill in anything missing from `other` (explicit files win over cache).
    pub fn merge_missing(&mut self, other: Enrichment) {
        for (k, v) in other.epss {
            self.epss.entry(k).or_insert(v);
        }
        self.kev.extend(other.kev);
    }

    pub fn apply(&self, findings: &mut [Finding]) {
        for f in findings {
            let key = f.id.to_ascii_uppercase();
            if let Some((s, p)) = self.epss.get(&key) {
                f.epss = Some(*s);
                f.epss_percentile = Some(*p);
            }
            f.kev = self.kev.contains(&key);
        }
    }
}

// ---------------------------------------------------------------------------
// Gating
// ---------------------------------------------------------------------------

/// Number of active findings at or above `threshold` severity.
pub fn count_at_or_above(findings: &[Finding], threshold: &str) -> Result<usize> {
    let min = cve::severity_rank(threshold);
    if min == 0 {
        bail!(
            "invalid severity `{}` (expected critical, high, medium or low)",
            threshold
        );
    }
    Ok(findings
        .iter()
        .filter(|f| f.is_active() && cve::severity_rank(&f.severity) >= min)
        .count())
}

// ---------------------------------------------------------------------------
// OpenVEX output
// ---------------------------------------------------------------------------

fn short_hash(v: &Value) -> String {
    let d = Sha256::digest(serde_json::to_vec(v).unwrap_or_default());
    d.iter().take(16).map(|b| format!("{:02x}", b)).collect()
}

/// Render findings as an OpenVEX v0.2.0 document.
///
/// Active findings become `affected` (with an action statement); findings
/// suppressed by an input VEX keep that status and justification.
pub fn to_openvex(findings: &[Finding], author: &str, timestamp: &str) -> Value {
    let statements: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut st = json!({
                "vulnerability": {"name": f.id},
                "products": [{"@id": f.purl()}],
            });
            match &f.suppressed_by {
                Some(m) => {
                    st["status"] = json!(m.status);
                    if m.status == "not_affected" {
                        match &m.justification {
                            Some(j) => st["justification"] = json!(j),
                            None => {
                                st["impact_statement"] =
                                    json!("Marked not affected by supplied VEX")
                            }
                        }
                    }
                }
                None => {
                    st["status"] = json!("affected");
                    st["action_statement"] = json!(match &f.fixed_version {
                        Some(v) => format!("Upgrade {} to {} or later", f.package, v),
                        None => format!(
                            "No fixed version known for {}; apply mitigations",
                            f.package
                        ),
                    });
                }
            }
            st["timestamp"] = json!(timestamp);
            st
        })
        .collect();
    let id = format!(
        "https://zyvor.dev/guestkit/openvex/{}",
        short_hash(&json!(statements))
    );
    json!({
        "@context": "https://openvex.dev/ns/v0.2.0",
        "@id": id,
        "author": author,
        "timestamp": timestamp,
        "version": 1,
        "tooling": "guestkit cve",
        "statements": statements,
    })
}

// ---------------------------------------------------------------------------
// CSAF 2.0 (csaf_vex profile) output
// ---------------------------------------------------------------------------

/// Render findings as a CSAF 2.0 `csaf_vex` advisory.
pub fn to_csaf(findings: &[Finding], title: &str, timestamp: &str) -> Value {
    let mut products: BTreeMap<String, &Finding> = BTreeMap::new();
    for f in findings {
        products
            .entry(format!("{}@{}", f.package, f.version))
            .or_insert(f);
    }
    let product_nodes: Vec<Value> = products
        .iter()
        .map(|(pid, f)| {
            json!({
                "category": "product_version",
                "name": format!("{} {}", f.package, f.version),
                "product": {
                    "name": format!("{} {}", f.package, f.version),
                    "product_id": pid,
                    "product_identification_helper": {"purl": f.purl()},
                }
            })
        })
        .collect();

    let mut by_id: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for f in findings {
        by_id.entry(f.id.as_str()).or_default().push(f);
    }
    let vulns: Vec<Value> = by_id
        .iter()
        .map(|(id, fs)| {
            let pid = |f: &Finding| format!("{}@{}", f.package, f.version);
            let mut affected = Vec::new();
            let mut not_affected = Vec::new();
            let mut fixed = Vec::new();
            let mut flags = Vec::new();
            let mut remediations = Vec::new();
            for f in fs {
                match &f.suppressed_by {
                    None => {
                        affected.push(pid(f));
                        remediations.push(match &f.fixed_version {
                            Some(v) => json!({
                                "category": "vendor_fix",
                                "details": format!("Upgrade {} to {} or later", f.package, v),
                                "product_ids": [pid(f)],
                            }),
                            None => json!({
                                "category": "none_available",
                                "details": "No fix available",
                                "product_ids": [pid(f)],
                            }),
                        });
                    }
                    Some(m) if m.status == "fixed" => fixed.push(pid(f)),
                    Some(m) => {
                        not_affected.push(pid(f));
                        let label = m
                            .justification
                            .clone()
                            .filter(|j| !j.is_empty())
                            .unwrap_or_else(|| "vulnerable_code_not_present".to_string());
                        flags.push(json!({"label": label, "product_ids": [pid(f)]}));
                    }
                }
            }
            let first = fs[0];
            let desc = if first.description.is_empty() {
                "n/a"
            } else {
                first.description.as_str()
            };
            let mut details = format!("severity {}", first.severity);
            if let Some(e) = first.epss {
                details.push_str(&format!(", EPSS {:.5}", e));
            }
            if first.kev {
                details.push_str(", listed in CISA KEV");
            }
            let notes = vec![
                json!({"category": "description", "text": desc}),
                json!({"category": "details", "title": "guestkit", "text": details}),
            ];

            let mut status = serde_json::Map::new();
            if !affected.is_empty() {
                status.insert("known_affected".into(), json!(affected));
            }
            if !not_affected.is_empty() {
                status.insert("known_not_affected".into(), json!(not_affected));
            }
            if !fixed.is_empty() {
                status.insert("fixed".into(), json!(fixed));
            }
            let mut v = json!({"notes": notes, "product_status": status});
            if id.to_ascii_uppercase().starts_with("CVE-") {
                v["cve"] = json!(id);
            } else {
                v["ids"] = json!([{"system_name": "OSV", "text": id}]);
            }
            if !flags.is_empty() {
                v["flags"] = json!(flags);
            }
            if !remediations.is_empty() {
                v["remediations"] = json!(remediations);
            }
            v
        })
        .collect();

    json!({
        "document": {
            "category": "csaf_vex",
            "csaf_version": "2.0",
            "title": title,
            "publisher": {
                "category": "vendor",
                "name": "Zyvor AI Labs guestkit",
                "namespace": "https://zyvor.dev",
            },
            "tracking": {
                "id": format!("guestkit-vex-{}", short_hash(&json!(vulns))),
                "status": "final",
                "version": "1",
                "initial_release_date": timestamp,
                "current_release_date": timestamp,
                "revision_history": [
                    {"date": timestamp, "number": "1", "summary": "Initial version"}
                ],
            },
        },
        "product_tree": {
            "branches": [{
                "category": "vendor",
                "name": "scanned image",
                "branches": product_nodes,
            }]
        },
        "vulnerabilities": vulns,
    })
}

/// Plain-text table.
pub fn to_text(findings: &[Finding]) -> String {
    let mut s = String::new();
    let active = findings.iter().filter(|f| f.is_active()).count();
    s.push_str(&format!(
        "Vulnerabilities: {} active, {} suppressed by VEX\n",
        active,
        findings.len() - active
    ));
    let mut sorted: Vec<&Finding> = findings.iter().collect();
    sorted.sort_by(|a, b| {
        cve::severity_rank(&b.severity)
            .cmp(&cve::severity_rank(&a.severity))
            .then(a.id.cmp(&b.id))
    });
    for f in sorted {
        let mut extra = String::new();
        if let Some(e) = f.epss {
            extra.push_str(&format!(" epss={:.3}", e));
        }
        if f.kev {
            extra.push_str(" KEV");
        }
        if let Some(m) = &f.suppressed_by {
            extra.push_str(&format!(" [vex:{}]", m.status));
        }
        s.push_str(&format!(
            "  {:<9} {:<18} {} {}{}\n",
            f.severity, f.id, f.package, f.version, extra
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Offline vulnerability DB bundle
// ---------------------------------------------------------------------------

pub const BUNDLE_SCHEMA: &str = "guestkit-vulndb/1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BundlePayload {
    /// OSV cache entries keyed by sanitized `name@version` file stem.
    pub entries: BTreeMap<String, Vec<VulnerabilityInfo>>,
    pub epss: BTreeMap<String, (f64, f64)>,
    pub kev: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnDbBundle {
    pub schema: String,
    pub created: String,
    /// SHA-256 (hex) of the canonical JSON of `payload`.
    pub sha256: String,
    pub payload: BundlePayload,
}

fn payload_digest(p: &BundlePayload) -> Result<String> {
    let d = Sha256::digest(serde_json::to_vec(p)?);
    Ok(d.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Build a bundle from a CVE cache dir (`*.json` files holding
/// `Vec<VulnerabilityInfo>`) and a feeds dir (EPSS / KEV).
pub fn export_bundle(cve_dir: &Path, feeds: &Path, created: &str) -> Result<VulnDbBundle> {
    let mut payload = BundlePayload::default();
    if cve_dir.is_dir() {
        for ent in std::fs::read_dir(cve_dir)? {
            let p = ent?.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let text = std::fs::read_to_string(&p)?;
            if let Ok(v) = serde_json::from_str::<Vec<VulnerabilityInfo>>(&text) {
                if !v.is_empty() {
                    payload.entries.insert(stem.to_string(), v);
                }
            }
        }
    }
    let enr = Enrichment::from_cache_dir(feeds);
    payload.epss = enr.epss;
    let mut kev: Vec<String> = enr.kev.into_iter().collect();
    kev.sort();
    payload.kev = kev;
    Ok(VulnDbBundle {
        schema: BUNDLE_SCHEMA.to_string(),
        created: created.to_string(),
        sha256: payload_digest(&payload)?,
        payload,
    })
}

/// Verify and install a bundle. Returns (entries, epss, kev) counts.
pub fn import_bundle(
    bundle_json: &str,
    cve_dir: &Path,
    feeds: &Path,
) -> Result<(usize, usize, usize)> {
    let b: VulnDbBundle = serde_json::from_str(bundle_json).context("parse vuln DB bundle")?;
    if b.schema != BUNDLE_SCHEMA {
        bail!("unsupported bundle schema `{}`", b.schema);
    }
    if payload_digest(&b.payload)? != b.sha256 {
        bail!("bundle integrity check failed (sha256 mismatch); refusing to import");
    }
    std::fs::create_dir_all(cve_dir)?;
    std::fs::create_dir_all(feeds)?;
    for (key, vulns) in &b.payload.entries {
        // Same sanitisation as the OSV cache so lookups hit offline; this also
        // blocks path traversal through crafted keys.
        let stem = cve::sanitize_cache_key(key);
        std::fs::write(
            cve_dir.join(format!("{}.json", stem)),
            serde_json::to_string_pretty(vulns)?,
        )?;
    }
    if !b.payload.epss.is_empty() {
        let map: BTreeMap<&String, Value> = b
            .payload
            .epss
            .iter()
            .map(|(k, (s, p))| (k, json!({"epss": s, "percentile": p})))
            .collect();
        std::fs::write(feeds.join("epss.json"), serde_json::to_string(&map)?)?;
    }
    if !b.payload.kev.is_empty() {
        std::fs::write(
            feeds.join("kev.json"),
            serde_json::to_string(&b.payload.kev)?,
        )?;
    }
    Ok((
        b.payload.entries.len(),
        b.payload.epss.len(),
        b.payload.kev.len(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(pkg: &str, ver: &str, id: &str, sev: &str) -> Finding {
        Finding {
            package: pkg.into(),
            version: ver.into(),
            package_type: "deb".into(),
            id: id.into(),
            severity: sev.into(),
            score: None,
            description: "d".into(),
            fixed_version: None,
            epss: None,
            epss_percentile: None,
            kev: false,
            suppressed_by: None,
        }
    }

    const VEX: &str = r#"{"@context":"https://openvex.dev/ns/v0.2.0","statements":[
      {"vulnerability":{"name":"CVE-2024-0001"},"products":[{"@id":"pkg:deb/foo@1.0"}],"status":"not_affected","justification":"vulnerable_code_not_present"},
      {"vulnerability":{"name":"CVE-2024-0002","aliases":["GHSA-xxxx"]},"status":"fixed"},
      {"vulnerability":"CVE-2024-0003","status":"affected"}
    ]}"#;

    #[test]
    fn vex_suppresses_not_affected_and_fixed_only() {
        let mut fs = vec![
            finding("foo", "1.0", "CVE-2024-0001", "high"),
            finding("foo", "2.0", "CVE-2024-0001", "high"), // version differs
            finding("bar", "1", "GHSA-xxxx", "low"),        // alias of a fixed CVE
            finding("bar", "1", "CVE-2024-0003", "high"),   // affected stays active
        ];
        apply_vex(&mut fs, &parse_openvex(VEX).unwrap());
        assert_eq!(fs[0].suppressed_by.as_ref().unwrap().status, "not_affected");
        assert!(fs[1].is_active());
        assert_eq!(fs[2].suppressed_by.as_ref().unwrap().status, "fixed");
        assert!(fs[3].is_active());
    }

    #[test]
    fn later_vex_statement_overrides() {
        let vex = r#"{"statements":[
          {"vulnerability":{"name":"CVE-1"},"status":"not_affected"},
          {"vulnerability":{"name":"CVE-1"},"status":"under_investigation"}]}"#;
        let mut fs = vec![finding("a", "1", "CVE-1", "high")];
        apply_vex(&mut fs, &parse_openvex(vex).unwrap());
        assert!(fs[0].is_active());
    }

    #[test]
    fn rejects_non_openvex() {
        assert!(parse_openvex("{}").is_err());
        assert!(parse_openvex("nope").is_err());
    }

    #[test]
    fn epss_csv_and_json_and_kev() {
        let csv = "#model_version:v2025,score_date:2026-01-01\ncve,epss,percentile\nCVE-2024-0001,0.97000,0.99900\nCVE-2024-0002,0.00100,0.10000\n";
        let m = parse_epss(csv).unwrap();
        assert_eq!(m["CVE-2024-0001"], (0.97, 0.999));
        let js = r#"{"data":[{"cve":"CVE-2024-0003","epss":"0.5","percentile":"0.8"}]}"#;
        assert_eq!(parse_epss(js).unwrap()["CVE-2024-0003"], (0.5, 0.8));
        let kev = r#"{"vulnerabilities":[{"cveID":"CVE-2024-0001"}]}"#;
        assert!(parse_kev(kev).unwrap().contains("CVE-2024-0001"));
        assert!(parse_kev("cve-2024-0009\n# c\n")
            .unwrap()
            .contains("CVE-2024-0009"));
    }

    #[test]
    fn enrichment_applies() {
        let e = Enrichment {
            epss: parse_epss("CVE-1,0.4,0.9\n").unwrap(),
            kev: parse_kev("CVE-1\n").unwrap(),
        };
        let mut fs = vec![
            finding("a", "1", "CVE-1", "high"),
            finding("a", "1", "CVE-2", "low"),
        ];
        e.apply(&mut fs);
        assert_eq!(fs[0].epss, Some(0.4));
        assert!(fs[0].kev);
        assert!(fs[1].epss.is_none() && !fs[1].kev);
    }

    #[test]
    fn fail_on_counts_active_only() {
        let mut fs = vec![
            finding("a", "1", "CVE-1", "critical"),
            finding("a", "1", "CVE-2", "medium"),
        ];
        assert_eq!(count_at_or_above(&fs, "high").unwrap(), 1);
        assert_eq!(count_at_or_above(&fs, "medium").unwrap(), 2);
        fs[0].suppressed_by = Some(VexMatch {
            status: "fixed".into(),
            justification: None,
        });
        assert_eq!(count_at_or_above(&fs, "high").unwrap(), 0);
        assert!(count_at_or_above(&fs, "bogus").is_err());
    }

    #[test]
    fn severity_derived_from_score() {
        let v = VulnerabilityInfo::new("X".into(), "unknown".into(), Some(9.8), "".into(), None);
        assert_eq!(effective_severity(&v), "critical");
        let v = VulnerabilityInfo::new("X".into(), "unknown".into(), None, "".into(), None);
        assert_eq!(effective_severity(&v), "unknown");
    }

    #[test]
    fn openvex_output_shape() {
        let mut a = finding("foo", "1.0", "CVE-1", "high");
        a.fixed_version = Some("1.1".into());
        let mut b = finding("bar", "2", "CVE-2", "low");
        b.suppressed_by = Some(VexMatch {
            status: "not_affected".into(),
            justification: Some("component_not_present".into()),
        });
        let doc = to_openvex(&[a, b], "me", "2026-01-01T00:00:00Z");
        assert_eq!(doc["@context"], "https://openvex.dev/ns/v0.2.0");
        let st = doc["statements"].as_array().unwrap();
        assert_eq!(st[0]["status"], "affected");
        assert_eq!(st[0]["products"][0]["@id"], "pkg:deb/foo@1.0");
        assert!(st[0]["action_statement"].as_str().unwrap().contains("1.1"));
        assert_eq!(st[1]["status"], "not_affected");
        assert_eq!(st[1]["justification"], "component_not_present");
        // our own output is consumable as VEX input
        assert_eq!(parse_openvex(&doc.to_string()).unwrap().len(), 2);
    }

    #[test]
    fn csaf_output_shape() {
        let a = finding("foo", "1.0", "CVE-2024-1", "high");
        let mut b = finding("bar", "2", "GHSA-abcd", "low");
        b.suppressed_by = Some(VexMatch {
            status: "not_affected".into(),
            justification: None,
        });
        let mut c = finding("baz", "3", "CVE-2024-2", "low");
        c.suppressed_by = Some(VexMatch {
            status: "fixed".into(),
            justification: None,
        });
        let doc = to_csaf(&[a, b, c], "t", "2026-01-01T00:00:00Z");
        assert_eq!(doc["document"]["category"], "csaf_vex");
        assert_eq!(doc["document"]["csaf_version"], "2.0");
        let v = doc["vulnerabilities"].as_array().unwrap();
        assert_eq!(v.len(), 3);
        let by = |id: &str| {
            v.iter()
                .find(|x| x["cve"] == id || x["ids"][0]["text"] == id)
                .unwrap()
                .clone()
        };
        assert_eq!(
            by("CVE-2024-1")["product_status"]["known_affected"][0],
            "foo@1.0"
        );
        assert!(by("CVE-2024-1")["remediations"].is_array());
        assert_eq!(
            by("GHSA-abcd")["product_status"]["known_not_affected"][0],
            "bar@2"
        );
        assert_eq!(
            by("GHSA-abcd")["flags"][0]["label"],
            "vulnerable_code_not_present"
        );
        assert_eq!(by("CVE-2024-2")["product_status"]["fixed"][0], "baz@3");
        assert_eq!(
            doc["product_tree"]["branches"][0]["branches"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn bundle_roundtrip_and_tamper_detection() {
        let td = tempfile::tempdir().unwrap();
        let cve_dir = td.path().join("cve");
        let feeds = td.path().join("feeds");
        std::fs::create_dir_all(&cve_dir).unwrap();
        std::fs::create_dir_all(&feeds).unwrap();
        let v = vec![VulnerabilityInfo::new(
            "CVE-9".into(),
            "high".into(),
            Some(7.5),
            "x".into(),
            Some("1.2".into()),
        )];
        std::fs::write(
            cve_dir.join("zlib_1_2_11.json"),
            serde_json::to_string(&v).unwrap(),
        )
        .unwrap();
        std::fs::write(feeds.join("epss.csv"), "CVE-9,0.25,0.7\n").unwrap();
        std::fs::write(feeds.join("kev.json"), r#"["CVE-9"]"#).unwrap();

        let b = export_bundle(&cve_dir, &feeds, "2026-01-01T00:00:00Z").unwrap();
        let text = serde_json::to_string(&b).unwrap();

        let td2 = tempfile::tempdir().unwrap();
        let (c2, f2) = (td2.path().join("cve"), td2.path().join("feeds"));
        assert_eq!(import_bundle(&text, &c2, &f2).unwrap(), (1, 1, 1));
        let back: Vec<VulnerabilityInfo> =
            serde_json::from_str(&std::fs::read_to_string(c2.join("zlib_1_2_11.json")).unwrap())
                .unwrap();
        assert_eq!(back[0].cve, "CVE-9");
        let e = Enrichment::from_cache_dir(&f2);
        assert_eq!(e.epss["CVE-9"], (0.25, 0.7));
        assert!(e.kev.contains("CVE-9"));

        let tampered = text.replace("CVE-9", "CVE-8");
        assert!(import_bundle(&tampered, &c2, &f2).is_err());
    }

    #[test]
    fn bundle_key_cannot_escape_dir() {
        let mut payload = BundlePayload::default();
        payload.entries.insert(
            "../../evil".into(),
            vec![VulnerabilityInfo::new(
                "C".into(),
                "low".into(),
                None,
                "".into(),
                None,
            )],
        );
        let b = VulnDbBundle {
            schema: BUNDLE_SCHEMA.into(),
            created: "t".into(),
            sha256: payload_digest(&payload).unwrap(),
            payload,
        };
        let td = tempfile::tempdir().unwrap();
        let cd = td.path().join("a/b");
        import_bundle(
            &serde_json::to_string(&b).unwrap(),
            &cd,
            &td.path().join("f"),
        )
        .unwrap();
        assert!(cd.join("______evil.json").exists());
        assert!(!td.path().join("evil.json").exists());
    }
}
