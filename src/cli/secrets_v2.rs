// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Secrets scanner v2 engine used by `guestkit secrets`.
//!
//! * built-in rules plus gitleaks-style TOML rule files (`--rules`)
//! * Shannon-entropy detection of high-entropy assigned values
//! * allowlists (paths, regexes, stopwords, inline `guestkit:allow`)
//! * baseline files of finding fingerprints (known findings are suppressed)
//! * SARIF 2.1.0 and JSON output; secrets are redacted unless `--show-content`
//!
//! The engine is pure (text in, findings out); the guest filesystem walk lives
//! in `commands::security::secrets_command`.

use anyhow::{bail, Context, Result};
use clap::Args;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const BASELINE_SCHEMA: &str = "guestkit-secrets-baseline/1";
pub const DEFAULT_ENTROPY_THRESHOLD: f64 = 4.5;
pub const ENTROPY_RULE_ID: &str = "generic-high-entropy";

/// Paths scanned when `--scan-paths` is not given.
pub const DEFAULT_SCAN_PATHS: &[&str] = &[
    "/etc",
    "/home",
    "/root",
    "/var/www",
    "/opt",
    "/srv",
    "/var/lib/cloud",
    "/usr/local/etc",
];

/// Substrings excluded by default (public material / vendored trees).
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "/node_modules/",
    "/.git/objects/",
    "/etc/ssl/certs/",
    "/etc/pki/ca-trust/",
    "/usr/share/",
];

#[derive(Args, Debug, Clone, Default)]
pub struct SecretsV2Args {
    /// gitleaks-style TOML rule file ([[rules]], [allowlist]); added to the built-in rules
    #[arg(long, value_name = "FILE")]
    pub rules: Option<PathBuf>,

    /// Do not load the built-in rules (use only --rules and --patterns)
    #[arg(long)]
    pub no_default_rules: bool,

    /// Allowlist: skip files whose guest path matches this regex (repeatable)
    #[arg(long, value_name = "REGEX")]
    pub allow_path: Vec<String>,

    /// Allowlist: skip findings whose secret matches this regex (repeatable)
    #[arg(long, value_name = "REGEX")]
    pub allow_regex: Vec<String>,

    /// Shannon entropy (bits/char) above which an assigned value is reported
    #[arg(long, value_name = "BITS", default_value_t = DEFAULT_ENTROPY_THRESHOLD)]
    pub entropy_threshold: f64,

    /// Disable the generic high-entropy detector
    #[arg(long)]
    pub no_entropy: bool,

    /// Baseline file: findings whose fingerprint is listed are suppressed
    #[arg(long, value_name = "FILE")]
    pub baseline: Option<PathBuf>,

    /// Write (or extend) a baseline with all current findings, then continue
    #[arg(long, value_name = "FILE")]
    pub write_baseline: Option<PathBuf>,

    /// Report format: text, json, sarif
    #[arg(long, default_value = "text", value_parser = ["text", "json", "sarif"])]
    pub format: String,

    /// Exit with code 2 when findings remain after allowlists and baseline
    #[arg(long)]
    pub fail_on_findings: bool,
}

// ---------------------------------------------------------------------------
// Rule files
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawAllowlist {
    paths: Vec<String>,
    regexes: Vec<String>,
    stopwords: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawRule {
    id: String,
    description: String,
    regex: String,
    path: Option<String>,
    keywords: Vec<String>,
    entropy: Option<f64>,
    secret_group: usize,
    severity: Option<String>,
    allowlist: Option<RawAllowlist>,
    allowlists: Vec<RawAllowlist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawExtend {
    use_default: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawConfig {
    extend: Option<RawExtend>,
    rules: Vec<RawRule>,
    allowlist: Option<RawAllowlist>,
}

#[derive(Debug, Clone, Default)]
struct Allow {
    paths: Vec<Regex>,
    regexes: Vec<Regex>,
    stopwords: Vec<String>,
}

fn compile_all(list: &[String], what: &str) -> Result<Vec<Regex>> {
    list.iter()
        .map(|r| Regex::new(r).with_context(|| format!("invalid {} regex `{}`", what, r)))
        .collect()
}

impl Allow {
    fn from_raw(raw: &RawAllowlist) -> Result<Self> {
        Ok(Allow {
            paths: compile_all(&raw.paths, "allowlist path")?,
            regexes: compile_all(&raw.regexes, "allowlist")?,
            stopwords: raw.stopwords.iter().map(|s| s.to_lowercase()).collect(),
        })
    }

    fn path_allowed(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p.is_match(path))
    }

    fn secret_allowed(&self, secret: &str, matched: &str) -> bool {
        let low = secret.to_lowercase();
        self.stopwords.iter().any(|w| low.contains(w))
            || self
                .regexes
                .iter()
                .any(|r| r.is_match(secret) || r.is_match(matched))
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub description: String,
    pub severity: String,
    regex: Regex,
    path: Option<Regex>,
    keywords: Vec<String>,
    entropy: Option<f64>,
    secret_group: usize,
    allow: Vec<Allow>,
}

impl Rule {
    fn new(
        id: &str,
        description: &str,
        severity: &str,
        regex: &str,
        path: Option<&str>,
        keywords: &[&str],
    ) -> Rule {
        Rule {
            id: id.into(),
            description: description.into(),
            severity: severity.into(),
            regex: Regex::new(regex).expect("built-in secret rule regex"),
            path: path.map(|p| Regex::new(p).expect("built-in secret rule path regex")),
            keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
            entropy: None,
            secret_group: 0,
            allow: Vec::new(),
        }
    }

    fn with_group(mut self, g: usize) -> Rule {
        self.secret_group = g;
        self
    }
}

/// Built-in rules. The first block keeps the v1 patterns; the rest add cloud
/// credentials, kube/docker configs, shell history, netrc/pgpass and more.
pub fn default_rules() -> Vec<Rule> {
    vec![
        Rule::new("password", "Password", "medium", r"(?i)(password|passwd|pwd)\s*[:=]\s*\S{8,}", None, &["pass", "pwd"]),
        Rule::new("api-key", "API Key", "high", r"(?i)(api[_-]?key|apikey)\s*[:=]\s*[A-Za-z0-9_\-]{20,}", None, &["api"]),
        Rule::new("secret-key", "Secret Key", "high", r"(?i)(secret[_-]?key|secretkey)\s*[:=]\s*[A-Za-z0-9_\-]{20,}", None, &["secret"]),
        Rule::new("private-key-assignment", "Private Key", "high", r"(?i)(private[_-]?key|privatekey)\s*[:=]\s*[A-Za-z0-9_\-]{20,}", None, &["private"]),
        Rule::new("private-key", "Private Key (PEM/OpenSSH)", "critical", r"-----BEGIN (?:RSA |DSA |EC |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?-----", None, &["private key"]),
        Rule::new("bearer-token", "Bearer Token", "high", r"(?i)(bearer|token)\s*[:=]\s*[A-Za-z0-9_\-\.]{20,}", None, &["bearer", "token"]),
        Rule::new("aws-credential", "AWS Credential", "critical", r"(?i)(aws_access_key_id|aws_secret_access_key)\s*[:=]\s*[A-Za-z0-9/+=]{20,}", None, &["aws_"]),
        Rule::new("aws-access-key-id", "AWS Access Key ID", "critical", r"\b((?:AKIA|ASIA|ABIA|ACCA)[A-Z0-9]{16})\b", None, &["akia", "asia", "abia", "acca"]).with_group(1),
        Rule::new("mongodb-uri", "MongoDB Connection String", "high", r"(?i)mongodb(\+srv)?://[^:]+:[^@]+@", None, &["mongodb"]),
        Rule::new("database-uri", "Database Connection String", "high", r"(?i)(mysql|postgresql|postgres)://[^:]+:[^@]+@", None, &["mysql", "postgres"]),
        Rule::new("github-pat", "GitHub Token", "critical", r"\b(gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})\b", None, &["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"]),
        Rule::new("gitlab-pat", "GitLab Personal Access Token", "critical", r"glpat-[A-Za-z0-9_\-]{20,}", None, &["glpat-"]),
        Rule::new("stripe-live-key", "Stripe Live Key", "critical", r"sk_live_[A-Za-z0-9]{24,}", None, &["sk_live_"]),
        Rule::new("google-api-key", "Google API Key", "high", r"AIza[A-Za-z0-9_\-]{35}", None, &["aiza"]),
        Rule::new("slack-token", "Slack Token", "high", r"xox[baprs]-[A-Za-z0-9\-]{10,}", None, &["xox"]),
        Rule::new("jwt", "JSON Web Token", "medium", r"eyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}", None, &["eyj"]),
        Rule::new("azure-storage-key", "Azure Storage Account Key", "critical", r"(?i)AccountKey=([A-Za-z0-9+/=]{40,})", None, &["accountkey"]).with_group(1),
        Rule::new("azure-client-secret", "Azure Client Secret", "high", r#"(?i)(?:client_?secret|AZURE_CLIENT_SECRET)["']?\s*[:=]\s*["']?([A-Za-z0-9_.~\-]{30,})"#, None, &["client_secret", "clientsecret"]).with_group(1),
        Rule::new("kube-client-key", "Kubernetes client key data", "critical", r"(?i)client-key-data:\s*([A-Za-z0-9+/=]{40,})", Some(r"(?i)(kube|\.conf$|config$)"), &["client-key-data"]).with_group(1),
        Rule::new("kube-token", "Kubernetes bearer token", "high", r"(?im)^\s*token:\s*([A-Za-z0-9._\-]{20,})", Some(r"(?i)(kube|\.conf$|config$)"), &["token"]).with_group(1),
        Rule::new("docker-auth", "Docker registry auth", "high", r#""auth"\s*:\s*"([A-Za-z0-9+/=]{16,})""#, Some(r"(?i)docker"), &["\"auth\""]).with_group(1),
        Rule::new("http-authorization", "HTTP Authorization header", "high", r"(?i)authorization:\s*(?:bearer|basic)\s+([A-Za-z0-9._~+/=\-]{16,})", None, &["authorization"]).with_group(1),
        Rule::new("netrc-password", "netrc password", "high", r"(?i)\bpassword\s+(\S{4,})", Some(r"\.netrc$"), &["password"]).with_group(1),
        Rule::new("pgpass-password", "PostgreSQL pgpass entry", "high", r"^[^:#\s]+:[^:\s]+:[^:\s]+:[^:\s]+:(\S+)$", Some(r"\.pgpass$"), &[":"]).with_group(1),
        Rule::new("npmrc-token", "npm auth token", "high", r"_authToken=(\S{10,})", Some(r"\.npmrc$"), &["_authtoken"]).with_group(1),
        Rule::new("git-credentials", "Git stored credential", "high", r"https?://[^:\s/@]+:([^@\s]+)@", Some(r"\.git-credentials$"), &["http"]).with_group(1),
        Rule::new("history-password-flag", "Password on a command line (shell history)", "medium", r#"(?i)(?:\s-p|--password[= ]|--pass[= ])\s*['"]?([^\s'"]{6,})"#, Some(r"_history$|\.history$"), &["-p", "--pass"]).with_group(1),
        Rule::new("history-secret-export", "Secret exported in shell history", "high", r#"(?i)\bexport\s+[A-Z0-9_]*(?:SECRET|TOKEN|PASSWORD|PASSWD|API_?KEY|ACCESS_?KEY)[A-Z0-9_]*=['"]?([^\s'"]{8,})"#, Some(r"_history$|\.history$|\.(bash|zsh)rc$|\.profile$"), &["export"]).with_group(1),
    ]
}

fn default_stopwords() -> Vec<String> {
    [
        "example",
        "changeme",
        "change_me",
        "placeholder",
        "your_",
        "your-",
        "xxxxxxxx",
        "<redacted>",
        "${",
        "{{",
        "redacted",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SecretFinding {
    pub rule_id: String,
    pub description: String,
    pub severity: String,
    pub file: String,
    pub line: usize,
    /// Stable id (sha256 of rule, file and secret); the secret is not recoverable from it.
    pub fingerprint: String,
    pub entropy: f64,
    pub redacted: String,
    /// Only set with `--show-content`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

/// Shannon entropy in bits per character.
pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut counts = std::collections::HashMap::new();
    let mut n = 0f64;
    for c in s.chars() {
        *counts.entry(c).or_insert(0f64) += 1.0;
        n += 1.0;
    }
    -counts
        .values()
        .map(|c| {
            let p = c / n;
            p * p.log2()
        })
        .sum::<f64>()
}

/// Redact a secret for display: never more than the length is revealed.
pub fn redact(secret: &str) -> String {
    let n = secret.chars().count();
    if n >= 12 {
        let head: String = secret.chars().take(2).collect();
        format!("{}***[REDACTED {} chars]", head, n)
    } else {
        format!("***[REDACTED {} chars]", n)
    }
}

fn fingerprint(rule_id: &str, file: &str, secret: &str) -> String {
    let mut h = Sha256::new();
    h.update(rule_id.as_bytes());
    h.update([0]);
    h.update(file.as_bytes());
    h.update([0]);
    h.update(secret.as_bytes());
    h.finalize()
        .iter()
        .take(12)
        .map(|b| format!("{:02x}", b))
        .collect()
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

pub struct Scanner {
    rules: Vec<Rule>,
    global: Allow,
    entropy_threshold: Option<f64>,
    entropy_re: Regex,
    show_content: bool,
}

impl Scanner {
    /// Build a scanner from CLI options plus custom `--patterns` regexes.
    pub fn from_args(
        args: &SecretsV2Args,
        custom_patterns: &[String],
        show_content: bool,
    ) -> Result<Scanner> {
        let mut rules = Vec::new();
        let mut global = Allow {
            stopwords: default_stopwords(),
            ..Default::default()
        };
        let mut use_default = !args.no_default_rules;

        let mut cfg: Option<RawConfig> = None;
        if let Some(p) = &args.rules {
            let text = std::fs::read_to_string(p)
                .with_context(|| format!("read rules file {}", p.display()))?;
            let c: RawConfig =
                toml::from_str(&text).with_context(|| format!("parse rules {}", p.display()))?;
            if let Some(false) = c.extend.as_ref().and_then(|e| e.use_default) {
                use_default = false;
            }
            cfg = Some(c);
        }
        if use_default {
            rules.extend(default_rules());
        }
        if let Some(c) = cfg {
            for r in c.rules {
                rules.push(compile_rule(r)?);
            }
            if let Some(al) = &c.allowlist {
                let a = Allow::from_raw(al)?;
                global.paths.extend(a.paths);
                global.regexes.extend(a.regexes);
                global.stopwords.extend(a.stopwords);
            }
        }
        for (i, pat) in custom_patterns.iter().enumerate() {
            rules.push(Rule {
                id: format!("custom-pattern-{}", i + 1),
                description: "Custom Pattern".into(),
                severity: "medium".into(),
                regex: Regex::new(pat).with_context(|| format!("invalid pattern `{}`", pat))?,
                path: None,
                keywords: vec![],
                entropy: None,
                secret_group: 0,
                allow: vec![],
            });
        }
        global
            .paths
            .extend(compile_all(&args.allow_path, "--allow-path")?);
        global
            .regexes
            .extend(compile_all(&args.allow_regex, "--allow-regex")?);

        Ok(Scanner {
            rules,
            global,
            entropy_threshold: if args.no_entropy {
                None
            } else {
                Some(args.entropy_threshold)
            },
            entropy_re: Regex::new(r#"[:=]\s*["']?([A-Za-z0-9+/=_\-.~]{20,})["']?"#)?,
            show_content,
        })
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// True when the whole file is allowlisted.
    pub fn path_skipped(&self, path: &str) -> bool {
        self.global.path_allowed(path)
    }

    fn make(
        &self,
        id: &str,
        desc: &str,
        sev: &str,
        file: &str,
        line: usize,
        secret: &str,
    ) -> SecretFinding {
        SecretFinding {
            rule_id: id.into(),
            description: desc.into(),
            severity: sev.into(),
            file: file.into(),
            line,
            fingerprint: fingerprint(id, file, secret),
            entropy: (shannon_entropy(secret) * 100.0).round() / 100.0,
            redacted: redact(secret),
            secret: self.show_content.then(|| secret.to_string()),
        }
    }

    /// Scan one text file.
    pub fn scan_text(&self, path: &str, text: &str) -> Vec<SecretFinding> {
        let mut out = Vec::new();
        if self.path_skipped(path) {
            return out;
        }
        let rules: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| r.path.as_ref().map(|p| p.is_match(path)).unwrap_or(true))
            .filter(|r| !r.allow.iter().any(|a| a.path_allowed(path)))
            .collect();

        for (idx, line) in text.lines().enumerate() {
            let lineno = idx + 1;
            if line.contains("guestkit:allow") || line.contains("gitleaks:allow") {
                continue;
            }
            let lower = line.to_lowercase();
            let mut hit_spans: Vec<(usize, usize)> = Vec::new();

            for r in &rules {
                if !r.keywords.is_empty() && !r.keywords.iter().any(|k| lower.contains(k)) {
                    continue;
                }
                for cap in r.regex.captures_iter(line) {
                    let whole = cap.get(0).unwrap();
                    let sec = cap.get(r.secret_group).unwrap_or(whole);
                    let secret = sec.as_str();
                    if let Some(min) = r.entropy {
                        if shannon_entropy(secret) < min {
                            continue;
                        }
                    }
                    if self.global.secret_allowed(secret, whole.as_str())
                        || r.allow
                            .iter()
                            .any(|a| a.secret_allowed(secret, whole.as_str()))
                    {
                        continue;
                    }
                    hit_spans.push((sec.start(), sec.end()));
                    out.push(self.make(&r.id, &r.description, &r.severity, path, lineno, secret));
                }
            }

            if let Some(th) = self.entropy_threshold {
                for cap in self.entropy_re.captures_iter(line) {
                    let m = cap.get(1).unwrap();
                    let v = m.as_str();
                    // already reported by a specific rule
                    if hit_spans
                        .iter()
                        .any(|(s, e)| m.start() < *e && *s < m.end())
                    {
                        continue;
                    }
                    if v.starts_with('/') || v.starts_with("./") || v.starts_with("../") {
                        continue;
                    }
                    if shannon_entropy(v) < th || self.global.secret_allowed(v, v) {
                        continue;
                    }
                    out.push(self.make(
                        ENTROPY_RULE_ID,
                        "High-entropy value",
                        "medium",
                        path,
                        lineno,
                        v,
                    ));
                }
            }
        }
        out
    }

    /// Scan a directory tree on the host (used for tests and extracted images).
    pub fn scan_dir(&self, root: &Path) -> Result<Vec<SecretFinding>> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let p = e.path();
                let ft = e.file_type()?;
                if ft.is_dir() {
                    stack.push(p);
                } else if ft.is_file() {
                    if let Ok(text) = std::fs::read_to_string(&p) {
                        let guest =
                            format!("/{}", p.strip_prefix(root).unwrap_or(&p).to_string_lossy());
                        out.extend(self.scan_text(&guest, &text));
                    }
                }
            }
        }
        Ok(out)
    }
}

fn compile_rule(r: RawRule) -> Result<Rule> {
    if r.id.is_empty() {
        bail!("rule is missing `id`");
    }
    if r.regex.is_empty() {
        bail!("rule `{}` is missing `regex`", r.id);
    }
    let regex = Regex::new(&r.regex).with_context(|| format!("rule `{}`: invalid regex", r.id))?;
    if r.secret_group >= regex.captures_len() {
        bail!(
            "rule `{}`: secretGroup {} exceeds the number of capture groups",
            r.id,
            r.secret_group
        );
    }
    let mut allow = Vec::new();
    for raw in r.allowlist.iter().chain(r.allowlists.iter()) {
        allow.push(Allow::from_raw(raw)?);
    }
    Ok(Rule {
        description: if r.description.is_empty() {
            r.id.clone()
        } else {
            r.description
        },
        severity: r.severity.unwrap_or_else(|| "high".into()),
        path: match r.path {
            Some(p) => Some(Regex::new(&p).with_context(|| format!("rule `{}`: bad path", r.id))?),
            None => None,
        },
        keywords: r.keywords.iter().map(|k| k.to_lowercase()).collect(),
        entropy: r.entropy,
        secret_group: r.secret_group,
        allow,
        regex,
        id: r.id,
    })
}

// ---------------------------------------------------------------------------
// Baseline
// ---------------------------------------------------------------------------

pub fn load_baseline(path: &Path) -> Result<BTreeSet<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read baseline {}", path.display()))?;
    let v: Value = serde_json::from_str(&text).context("baseline is not JSON")?;
    if v.get("schema").and_then(|s| s.as_str()) != Some(BASELINE_SCHEMA) {
        bail!("{} is not a guestkit secrets baseline", path.display());
    }
    Ok(v.get("fingerprints")
        .and_then(|f| f.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

/// Write a baseline, extending an existing one at the same path.
pub fn write_baseline(path: &Path, findings: &[SecretFinding]) -> Result<usize> {
    let mut set = if path.exists() {
        load_baseline(path)?
    } else {
        BTreeSet::new()
    };
    set.extend(findings.iter().map(|f| f.fingerprint.clone()));
    let doc = json!({"schema": BASELINE_SCHEMA, "fingerprints": set});
    std::fs::write(path, serde_json::to_string_pretty(&doc)?)
        .with_context(|| format!("write baseline {}", path.display()))?;
    Ok(set.len())
}

/// Drop findings present in the baseline; returns the number suppressed.
pub fn apply_baseline(findings: &mut Vec<SecretFinding>, baseline: &BTreeSet<String>) -> usize {
    let before = findings.len();
    findings.retain(|f| !baseline.contains(&f.fingerprint));
    before - findings.len()
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn sarif_level(sev: &str) -> &'static str {
    match sev {
        "critical" | "high" => "error",
        "medium" => "warning",
        _ => "note",
    }
}

/// SARIF 2.1.0. Never contains the secret itself, only the redacted form.
pub fn to_sarif(findings: &[SecretFinding], image: &str) -> Value {
    let mut rule_ids: BTreeSet<(&str, &str)> = BTreeSet::new();
    for f in findings {
        rule_ids.insert((f.rule_id.as_str(), f.description.as_str()));
    }
    let rules: Vec<Value> = rule_ids
        .iter()
        .map(|(id, d)| json!({"id": id, "name": id, "shortDescription": {"text": d}}))
        .collect();
    let results: Vec<Value> = findings
        .iter()
        .map(|f| {
            json!({
                "ruleId": f.rule_id,
                "level": sarif_level(&f.severity),
                "message": {"text": format!("{} ({})", f.description, f.redacted)},
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {"uri": f.file.trim_start_matches('/')},
                        "region": {"startLine": f.line},
                    }
                }],
                "partialFingerprints": {"guestkit/v1": f.fingerprint},
                "properties": {"severity": f.severity, "entropy": f.entropy},
            })
        })
        .collect();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {
                "name": "guestkit secrets",
                "informationUri": "https://zyvor.dev",
                "rules": rules,
            }},
            "originalUriBaseIds": {},
            "properties": {"image": image},
            "results": results,
        }]
    })
}

pub fn to_json(findings: &[SecretFinding]) -> Result<String> {
    Ok(serde_json::to_string_pretty(findings)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanner(args: SecretsV2Args) -> Scanner {
        Scanner::from_args(&args, &[], false).unwrap()
    }

    fn args() -> SecretsV2Args {
        SecretsV2Args {
            entropy_threshold: DEFAULT_ENTROPY_THRESHOLD,
            format: "text".into(),
            ..Default::default()
        }
    }

    // Test-only fake credentials assembled at runtime so no literal secret
    // pattern sits in the source.
    fn fake_aws_key() -> String {
        format!("{}{}", "AKIA", "QWERTYUIOPASDFGH")
    }

    fn random_token() -> String {
        // 40 distinct-ish chars: high entropy
        "q9Zr7LkV3xT0pWb2NfY8sHcJ6mGd4aEu1oIe5RvX".to_string()
    }

    #[test]
    fn entropy_values() {
        assert_eq!(shannon_entropy(""), 0.0);
        assert_eq!(shannon_entropy("aaaa"), 0.0);
        assert!((shannon_entropy("abcd") - 2.0).abs() < 1e-9);
        assert!(shannon_entropy(&random_token()) > 4.5);
    }

    #[test]
    fn detects_builtin_rule_and_redacts() {
        let s = scanner(args());
        let text = format!("[default]\naws_access_key_id = {}\n", fake_aws_key());
        let f = s.scan_text("/root/.aws/credentials", &text);
        assert!(f
            .iter()
            .any(|x| x.rule_id == "aws-credential" || x.rule_id == "aws-access-key-id"));
        let key = fake_aws_key();
        for x in &f {
            assert!(!x.redacted.contains(&key));
            assert!(x.secret.is_none());
            assert_eq!(x.line, 2);
        }
        let sarif = to_sarif(&f, "img").to_string();
        assert!(!sarif.contains(&key));
        assert!(!to_json(&f).unwrap().contains(&key));
    }

    #[test]
    fn show_content_includes_secret() {
        let s = Scanner::from_args(&args(), &[], true).unwrap();
        let f = s.scan_text("/etc/x", &format!("k={}\n", fake_aws_key()));
        assert!(f
            .iter()
            .any(|x| x.secret.as_deref() == Some(fake_aws_key().as_str())));
    }

    #[test]
    fn entropy_detector_flags_random_but_not_words_or_paths() {
        let s = scanner(args());
        let text = format!(
            "session = \"{}\"\nname = thisisaverylongconfigurationname\npath = /var/lib/some/long/directory/path/here\nuuid = 123e4567-e89b-12d3-a456-426614174000\n",
            random_token()
        );
        let f = s.scan_text("/etc/app.conf", &text);
        let ent: Vec<_> = f.iter().filter(|x| x.rule_id == ENTROPY_RULE_ID).collect();
        assert_eq!(ent.len(), 1, "{:?}", f);
        assert_eq!(ent[0].line, 1);

        let mut a = args();
        a.no_entropy = true;
        assert!(scanner(a).scan_text("/etc/app.conf", &text).is_empty());
        let mut a = args();
        a.entropy_threshold = 6.0;
        assert!(scanner(a).scan_text("/etc/app.conf", &text).is_empty());
    }

    #[test]
    fn specific_rule_suppresses_duplicate_entropy_hit() {
        let s = scanner(args());
        let f = s.scan_text(
            "/etc/x",
            &format!("aws_secret_access_key = {}\n", random_token()),
        );
        assert_eq!(f.len(), 1, "{:?}", f);
    }

    #[test]
    fn toml_rules_allowlists_and_stopwords() {
        let td = tempfile::tempdir().unwrap();
        let rules = td.path().join("rules.toml");
        std::fs::write(
            &rules,
            r#"
title = "custom"
[extend]
useDefault = false

[[rules]]
id = "acme-token"
description = "ACME token"
regex = '''acme_([a-z0-9]{16})'''
secretGroup = 1
keywords = ["acme_"]
entropy = 3.0
severity = "critical"
path = '''\.env$'''
[rules.allowlist]
stopwords = ["abcabcabc"]

[allowlist]
paths = ['''/vendor/''']
regexes = ['''^deadbeef''']
"#,
        )
        .unwrap();
        let mut a = args();
        a.rules = Some(rules);
        a.no_entropy = true;
        let s = scanner(a);
        assert_eq!(s.rule_count(), 1);

        let good = "acme_k3j9x2m7q1w8z5v4";
        let f = s.scan_text("/app/.env", &format!("T={}\n", good));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule_id, "acme-token");
        assert_eq!(f[0].severity, "critical");
        // wrong path
        assert!(s
            .scan_text("/app/config", &format!("T={}\n", good))
            .is_empty());
        // low entropy
        assert!(s
            .scan_text("/app/.env", "T=acme_aaaaaaaaaaaaaaaa\n")
            .is_empty());
        // rule stopword
        assert!(s
            .scan_text("/app/.env", "T=acme_abcabcabcabcabca\n")
            .is_empty());
        // global path allowlist
        assert!(s
            .scan_text("/vendor/.env", &format!("T={}\n", good))
            .is_empty());
        // inline allow
        assert!(s
            .scan_text("/app/.env", &format!("T={} # guestkit:allow\n", good))
            .is_empty());
    }

    #[test]
    fn bad_rule_files_are_rejected() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("r.toml");
        for body in [
            "[[rules]]\nid='x'\nregex='('\n",
            "[[rules]]\nregex='a'\n",
            "[[rules]]\nid='x'\nregex='a'\nsecretGroup=3\n",
            "not toml [",
        ] {
            std::fs::write(&p, body).unwrap();
            let mut a = args();
            a.rules = Some(p.clone());
            assert!(Scanner::from_args(&a, &[], false).is_err(), "{}", body);
        }
    }

    #[test]
    fn allow_flags_and_custom_patterns() {
        let mut a = args();
        a.allow_regex = vec!["AKIAQWERTY".into()];
        let s = scanner(a);
        assert!(s
            .scan_text("/etc/x", &format!("aws_access_key_id={}\n", fake_aws_key()))
            .is_empty());

        let mut a = args();
        a.allow_path = vec!["^/etc/skip".into()];
        let s = Scanner::from_args(&a, &["corp-[0-9]{6}".to_string()], false).unwrap();
        assert!(s.scan_text("/etc/skip/a", "corp-123456").is_empty());
        let f = s.scan_text("/etc/a", "x corp-123456");
        assert_eq!(f[0].rule_id, "custom-pattern-1");
    }

    #[test]
    fn cloud_kube_and_history_paths() {
        let s = scanner(args());
        let kube = format!(
            "users:\n- user:\n    client-key-data: {}\n",
            "A".repeat(10) + &random_token()
        );
        assert!(s
            .scan_text("/root/.kube/config", &kube)
            .iter()
            .any(|f| f.rule_id == "kube-client-key"));
        assert!(s
            .scan_text("/etc/hosts", &kube)
            .iter()
            .all(|f| f.rule_id != "kube-client-key"));

        let hist = "mysql -u root -pHunter2Hunter2\nexport DB_PASSWORD=s3cr3tValue99\n";
        let f = s.scan_text("/home/bob/.bash_history", hist);
        assert!(
            f.iter().any(|x| x.rule_id == "history-password-flag"),
            "{:?}",
            f
        );
        assert!(
            f.iter().any(|x| x.rule_id == "history-secret-export"),
            "{:?}",
            f
        );
        assert!(s
            .scan_text("/home/bob/notes.txt", hist)
            .iter()
            .all(|x| !x.rule_id.starts_with("history-")));

        let docker = format!(
            "{{\"auths\":{{\"r.io\":{{\"auth\":\"{}\"}}}}}}",
            "dXNlcjpwYXNzd29yZDEyMzQ1Njc4"
        );
        assert!(s
            .scan_text("/root/.docker/config.json", &docker)
            .iter()
            .any(|f| f.rule_id == "docker-auth"));
    }

    #[test]
    fn baseline_roundtrip_and_suppression() {
        let s = scanner(args());
        let f = s.scan_text("/etc/x", &format!("aws_access_key_id={}\n", fake_aws_key()));
        assert!(!f.is_empty());
        let td = tempfile::tempdir().unwrap();
        let bp = td.path().join("base.json");
        let n = write_baseline(&bp, &f).unwrap();
        assert_eq!(n, f.len());
        let b = load_baseline(&bp).unwrap();
        // same secret on a different line: fingerprint is line-independent
        let mut again = s.scan_text(
            "/etc/x",
            &format!("# moved\naws_access_key_id={}\n", fake_aws_key()),
        );
        let sup = apply_baseline(&mut again, &b);
        assert_eq!(sup, f.len());
        assert!(again.is_empty());
        // a different secret is still reported
        let mut other = s.scan_text("/etc/x", "aws_access_key_id=AKIAZZZZZZZZZZZZZZZZ\n");
        apply_baseline(&mut other, &b);
        assert!(!other.is_empty());
        // wrong schema rejected
        std::fs::write(&bp, r#"{"schema":"nope"}"#).unwrap();
        assert!(load_baseline(&bp).is_err());
    }

    #[test]
    fn sarif_shape() {
        let s = scanner(args());
        let f = s.scan_text(
            "/root/.aws/credentials",
            &format!("aws_access_key_id={}\n", fake_aws_key()),
        );
        let v = to_sarif(&f, "disk.img");
        assert_eq!(v["version"], "2.1.0");
        let r = &v["runs"][0]["results"][0];
        assert_eq!(r["level"], "error");
        assert_eq!(
            r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "root/.aws/credentials"
        );
        assert_eq!(
            r["locations"][0]["physicalLocation"]["region"]["startLine"],
            1
        );
        assert!(r["partialFingerprints"]["guestkit/v1"].is_string());
        assert!(!v["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn scan_dir_walks_fixture_tree() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        std::fs::create_dir_all(root.join("root/.aws")).unwrap();
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(
            root.join("root/.aws/credentials"),
            format!("aws_access_key_id = {}\n", fake_aws_key()),
        )
        .unwrap();
        std::fs::write(root.join("etc/hostname"), "box\n").unwrap();
        std::fs::write(root.join("etc/bin"), [0xff, 0xfe, 0x00]).unwrap();
        let f = scanner(args()).scan_dir(root).unwrap();
        assert!(!f.is_empty());
        assert!(f.iter().all(|x| x.file == "/root/.aws/credentials"));
    }

    #[test]
    fn redaction_shapes() {
        assert_eq!(redact("short"), "***[REDACTED 5 chars]");
        assert_eq!(redact("abcdefghijkl"), "ab***[REDACTED 12 chars]");
    }
}
