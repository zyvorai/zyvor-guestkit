// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! User-supplied audit rules ("policy as data"), loaded from a YAML file with
//! `guestkit image-audit --rules FILE`.
//!
//! ```yaml
//! rules:
//!   - id: ORG-001
//!     title: Corporate login banner is missing
//!     severity: medium
//!     kind: line-missing          # see RuleKind
//!     path: /etc/issue
//!     pattern: "Authorized use only"
//!     remediation: Install the standard banner.
//! ```
//!
//! Rules run through the same confined reader as the built-in rules (symlinks
//! stay in the guest root, size and fan-out caps apply). A finding never
//! contains the matching line, only its number, so a rule pointed at a
//! credentials file cannot leak the value into a report. Rule files are
//! validated strictly: unknown keys, duplicate ids, bad paths and bad regexes
//! are errors, not silently ignored rules.

use super::finding::{AuditReport, Confidence, Finding, Severity};
use super::hardening::{read_or_note, wild_match};
use super::root::{join_guest, GuestRoot};
use anyhow::{bail, Context, Result};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

const MAX_RULE_FILE: u64 = 1024 * 1024;
const MAX_RULES: usize = 500;
const MAX_PATTERN_LEN: usize = 1024;
const MAX_MATCHES_PER_FILE: usize = 20;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    rules: Vec<RuleSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleSpec {
    id: String,
    title: String,
    severity: String,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    category: Option<String>,
    /// `file-present`, `file-missing`, `line-matches`, `line-missing`, `max-mode`.
    kind: String,
    /// Absolute guest path; a `*`/`?` wildcard is allowed in the last component.
    path: String,
    /// Regular expression for the `line-*` kinds.
    #[serde(default)]
    pattern: Option<String>,
    /// Octal permission ceiling for `max-mode`, for example `0640`.
    #[serde(default)]
    max_mode: Option<String>,
    /// `line-missing` only: a missing file is not a finding.
    #[serde(default)]
    missing_ok: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    remediation: Option<String>,
}

#[derive(Debug, Clone)]
enum RuleKind {
    FilePresent,
    FileMissing,
    LineMatches(Regex),
    LineMissing(Regex, bool),
    MaxMode(u32),
}

#[derive(Debug, Clone)]
pub struct CustomRule {
    id: String,
    title: String,
    severity: Severity,
    confidence: Confidence,
    category: String,
    kind: RuleKind,
    pattern_src: String,
    path: String,
    description: String,
    remediation: String,
}

#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    pub rules: Vec<CustomRule>,
}

fn validate_path(id: &str, p: &str) -> Result<()> {
    if !p.starts_with('/') {
        bail!("rule {id}: `path` must be absolute, got `{p}`");
    }
    if p.contains('\0') || p.split('/').any(|c| c == "..") {
        bail!("rule {id}: `path` must not contain `..` or NUL: `{p}`");
    }
    let (dir, leaf) = p.rsplit_once('/').unwrap_or(("", p));
    if dir.contains(['*', '?']) {
        bail!("rule {id}: wildcards are only allowed in the last path component: `{p}`");
    }
    if leaf.is_empty() {
        bail!("rule {id}: `path` must name a file, got `{p}`");
    }
    Ok(())
}

fn compile(id: &str, pattern: &Option<String>) -> Result<(Regex, String)> {
    let Some(p) = pattern else {
        bail!("rule {id}: this kind needs a `pattern`");
    };
    if p.len() > MAX_PATTERN_LEN {
        bail!("rule {id}: `pattern` is longer than {MAX_PATTERN_LEN} bytes");
    }
    let re = RegexBuilder::new(p)
        .size_limit(1 << 20)
        .build()
        .with_context(|| format!("rule {id}: invalid `pattern`"))?;
    Ok((re, p.clone()))
}

impl RuleSet {
    pub fn parse(text: &str) -> Result<Self> {
        let file: RuleFile = serde_yaml::from_str(text).context("invalid rule file")?;
        if file.rules.len() > MAX_RULES {
            bail!("rule file has more than {MAX_RULES} rules");
        }
        let mut seen = HashSet::new();
        let mut rules = Vec::new();
        for s in file.rules {
            let id = s.id.trim().to_string();
            if id.is_empty()
                || id.len() > 64
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                bail!(
                    "rule id `{}` must be 1-64 characters of [A-Za-z0-9._-]",
                    s.id
                );
            }
            if !seen.insert(id.to_ascii_uppercase()) {
                bail!("duplicate rule id `{id}`");
            }
            if s.title.trim().is_empty() {
                bail!("rule {id}: `title` is empty");
            }
            validate_path(&id, &s.path)?;
            let severity = Severity::parse(&s.severity).with_context(|| format!("rule {id}"))?;
            let confidence = match s.confidence.as_deref().map(str::to_ascii_lowercase) {
                None => Confidence::Confirmed,
                Some(c) => match c.as_str() {
                    "confirmed" => Confidence::Confirmed,
                    "likely" => Confidence::Likely,
                    "heuristic" => Confidence::Heuristic,
                    other => bail!("rule {id}: unknown confidence `{other}`"),
                },
            };
            let mut pattern_src = String::new();
            let kind = match s.kind.as_str() {
                "file-present" => RuleKind::FilePresent,
                "file-missing" => RuleKind::FileMissing,
                "line-matches" => {
                    let (re, src) = compile(&id, &s.pattern)?;
                    pattern_src = src;
                    RuleKind::LineMatches(re)
                }
                "line-missing" => {
                    let (re, src) = compile(&id, &s.pattern)?;
                    pattern_src = src;
                    RuleKind::LineMissing(re, s.missing_ok)
                }
                "max-mode" => {
                    let m = s
                        .max_mode
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("rule {id}: `max-mode` needs `max_mode`"))?;
                    let n = u32::from_str_radix(m.trim_start_matches("0o"), 8)
                        .ok()
                        .filter(|n| *n <= 0o7777)
                        .ok_or_else(|| {
                            anyhow::anyhow!("rule {id}: `max_mode` must be octal like 0640, got `{m}`")
                        })?;
                    RuleKind::MaxMode(n)
                }
                other => bail!(
                    "rule {id}: unknown kind `{other}` (file-present, file-missing, line-matches, line-missing, max-mode)"
                ),
            };
            rules.push(CustomRule {
                id,
                title: s.title.trim().to_string(),
                severity,
                confidence,
                category: s.category.unwrap_or_else(|| "custom".into()),
                kind,
                pattern_src,
                path: s.path,
                description: s.description.unwrap_or_default(),
                remediation: s
                    .remediation
                    .unwrap_or_else(|| "See your organisation's policy for this rule.".into()),
            });
        }
        Ok(Self { rules })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let meta = std::fs::metadata(path)
            .with_context(|| format!("cannot read rule file {}", path.display()))?;
        if !meta.is_file() || meta.len() > MAX_RULE_FILE {
            bail!(
                "rule file {} is not a regular file under {MAX_RULE_FILE} bytes",
                path.display()
            );
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read rule file {}", path.display()))?;
        Self::parse(&text).with_context(|| path.display().to_string())
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Run every rule and push findings; each rule counts as one check.
    pub fn run(&self, root: &GuestRoot, report: &mut AuditReport) {
        for rule in &self.rules {
            report.checks_run += 1;
            rule.run(root, report);
        }
        report.finish();
    }
}

impl CustomRule {
    fn finding(&self, loc: &str, subject: &str, detail: String) -> Finding {
        let mut f = Finding::new(
            &self.id,
            &self.category,
            self.severity,
            self.confidence,
            self.title.clone(),
            &self.remediation,
        )
        .at(loc)
        .subject(subject);
        f.detail = if self.description.is_empty() {
            detail
        } else {
            format!("{} {}", self.description, detail)
        };
        f
    }

    /// Existing files this rule's path selects.
    fn targets(&self, root: &GuestRoot, report: &mut AuditReport) -> Vec<String> {
        let (dir, leaf) = self.path.rsplit_once('/').unwrap_or(("", &self.path));
        if !leaf.contains(['*', '?']) {
            return if root.exists(&self.path) {
                vec![self.path.clone()]
            } else {
                Vec::new()
            };
        }
        let dir = if dir.is_empty() { "/" } else { dir };
        let (names, truncated) = root.list_dir(dir);
        if truncated {
            report.note(format!(
                "{dir}: listing truncated while matching rule {}",
                self.id
            ));
        }
        let cap = root.limits().max_files_per_group;
        let mut out: Vec<String> = names
            .iter()
            .filter(|n| wild_match(leaf, n))
            .filter_map(|n| join_guest(dir, n))
            .filter(|p| root.size(p).is_some())
            .collect();
        if out.len() > cap {
            out.truncate(cap);
            report.note(format!(
                "rule {}: more than {cap} files matched `{}`, extra files skipped",
                self.id, self.path
            ));
        }
        out
    }

    fn run(&self, root: &GuestRoot, report: &mut AuditReport) {
        let targets = self.targets(root, report);
        match &self.kind {
            RuleKind::FilePresent => {
                for t in &targets {
                    report.push(self.finding(t, t, "The file exists.".into()));
                }
            }
            RuleKind::FileMissing => {
                if targets.is_empty() {
                    report.push(self.finding(
                        &self.path,
                        &self.path,
                        "The file does not exist.".into(),
                    ));
                }
            }
            RuleKind::LineMatches(re) => {
                for t in &targets {
                    let Some(text) = read_or_note(root, report, t) else {
                        continue;
                    };
                    let mut hits = 0;
                    for (i, line) in text.lines().enumerate() {
                        if !re.is_match(line) {
                            continue;
                        }
                        hits += 1;
                        if hits > MAX_MATCHES_PER_FILE {
                            report.note(format!(
                                "rule {}: {t} has more than {MAX_MATCHES_PER_FILE} matching lines, extra matches not listed",
                                self.id
                            ));
                            break;
                        }
                        report.push(self.finding(
                            &format!("{t}:{}", i + 1),
                            t,
                            format!(
                                "Line {} matches /{}/ (line content not copied).",
                                i + 1,
                                self.pattern_src
                            ),
                        ));
                    }
                }
            }
            RuleKind::LineMissing(re, missing_ok) => {
                if targets.is_empty() {
                    if !missing_ok {
                        report.push(self.finding(
                            &self.path,
                            &self.path,
                            format!(
                                "The file does not exist, so /{}/ cannot match.",
                                self.pattern_src
                            ),
                        ));
                    }
                    return;
                }
                for t in &targets {
                    let Some(text) = read_or_note(root, report, t) else {
                        continue;
                    };
                    if !text.lines().any(|l| re.is_match(l)) {
                        report.push(self.finding(
                            t,
                            t,
                            format!("No line matches /{}/.", self.pattern_src),
                        ));
                    }
                }
            }
            RuleKind::MaxMode(max) => {
                for t in &targets {
                    let Some(mode) = root.mode(t) else { continue };
                    if mode & !max != 0 {
                        report.push(self.finding(
                            t,
                            t,
                            format!("Mode is {mode:04o}; at most {max:04o} is allowed."),
                        ));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::root::fixture::Tree;
    use super::*;

    fn run(rules: &str, t: &Tree) -> AuditReport {
        let set = RuleSet::parse(rules).unwrap();
        let mut r = AuditReport::new("t");
        set.run(&t.root(), &mut r);
        r
    }

    #[test]
    fn rejects_bad_rule_files() {
        let bad = [
            "rules:\n  - id: A\n    title: t\n    severity: high\n    kind: nope\n    path: /etc/x\n",
            "rules:\n  - id: A\n    title: t\n    severity: loud\n    kind: file-present\n    path: /etc/x\n",
            "rules:\n  - id: A\n    title: t\n    severity: low\n    kind: file-present\n    path: etc/x\n",
            "rules:\n  - id: A\n    title: t\n    severity: low\n    kind: file-present\n    path: /etc/../x\n",
            "rules:\n  - id: A\n    title: t\n    severity: low\n    kind: file-present\n    path: /e*/x\n",
            "rules:\n  - id: A\n    title: t\n    severity: low\n    kind: line-matches\n    path: /etc/x\n",
            "rules:\n  - id: A\n    title: t\n    severity: low\n    kind: line-matches\n    path: /etc/x\n    pattern: '('\n",
            "rules:\n  - id: A\n    title: t\n    severity: low\n    kind: max-mode\n    path: /etc/x\n    max_mode: '999'\n",
            "rules:\n  - id: 'a b'\n    title: t\n    severity: low\n    kind: file-present\n    path: /etc/x\n",
            "rules:\n  - id: A\n    title: t\n    severity: low\n    kind: file-present\n    path: /etc/x\n    typo: 1\n",
        ];
        for b in bad {
            assert!(RuleSet::parse(b).is_err(), "should reject: {b}");
        }
        let dup = "rules:\n  - {id: A, title: t, severity: low, kind: file-present, path: /a}\n  - {id: a, title: t, severity: low, kind: file-present, path: /b}\n";
        assert!(RuleSet::parse(dup)
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
    }

    #[test]
    fn file_present_and_missing() {
        let t = Tree::new();
        t.file("/etc/hosts.equiv", "+ +\n");
        let r = run(
            "rules:\n  - {id: ORG-1, title: no hosts.equiv, severity: high, kind: file-present, path: /etc/hosts.equiv}\n  - {id: ORG-2, title: need banner, severity: low, kind: file-missing, path: /etc/issue}\n",
            &t,
        );
        let ids: Vec<_> = r.findings.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["ORG-1", "ORG-2"]);
        assert_eq!(r.findings[0].category, "custom");
        assert_eq!(r.checks_run, 2);
    }

    #[test]
    fn line_matches_reports_line_numbers_not_content() {
        let t = Tree::new();
        t.file(
            "/etc/app.conf",
            "a=1\npassword=hunter2\nb=2\npassword = other\n",
        );
        let r = run(
            "rules:\n  - id: ORG-3\n    title: plaintext password\n    severity: high\n    kind: line-matches\n    path: /etc/app.conf\n    pattern: '^password\\s*='\n",
            &t,
        );
        assert_eq!(r.findings.len(), 2);
        let locs: Vec<_> = r
            .findings
            .iter()
            .filter_map(|f| f.location.clone())
            .collect();
        assert!(locs.contains(&"/etc/app.conf:2".to_string()));
        assert!(locs.contains(&"/etc/app.conf:4".to_string()));
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("hunter2"));
    }

    #[test]
    fn line_missing_and_missing_ok() {
        let t = Tree::new();
        t.file("/etc/issue", "Welcome\n");
        let rules = "rules:\n  - {id: B1, title: banner, severity: low, kind: line-missing, path: /etc/issue, pattern: Authorized}\n  - {id: B2, title: motd, severity: low, kind: line-missing, path: /etc/motd, pattern: x}\n  - {id: B3, title: motd ok, severity: low, kind: line-missing, path: /etc/motd, pattern: x, missing_ok: true}\n";
        let r = run(rules, &t);
        let ids: Vec<_> = r.findings.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["B1", "B2"]);
        t.file("/etc/issue", "Authorized use only\n");
        let r2 = run(rules, &t);
        assert_eq!(r2.findings.len(), 1);
    }

    #[test]
    fn wildcard_targets_and_max_mode() {
        let t = Tree::new();
        t.file("/etc/ssl/a.key", "k");
        t.file("/etc/ssl/b.key", "k");
        t.file("/etc/ssl/c.pem", "k");
        t.mode("/etc/ssl/a.key", 0o644);
        t.mode("/etc/ssl/b.key", 0o600);
        t.mode("/etc/ssl/c.pem", 0o666);
        let r = run(
            "rules:\n  - {id: KEY-1, title: key perms, severity: medium, kind: max-mode, path: '/etc/ssl/*.key', max_mode: '0600'}\n",
            &t,
        );
        assert_eq!(r.findings.len(), 1);
        assert_eq!(r.findings[0].location.as_deref(), Some("/etc/ssl/a.key"));
        assert!(r.findings[0].detail.contains("0644"));
    }

    #[test]
    fn hostile_symlinks_stay_in_guest() {
        let t = Tree::new();
        // Would match host /etc/passwd if the link escaped the guest root.
        t.link("/etc/app.conf", "/etc/passwd");
        let r = run(
            "rules:\n  - {id: H1, title: root, severity: high, kind: line-matches, path: /etc/app.conf, pattern: 'root'}\n",
            &t,
        );
        assert!(r.findings.is_empty());
    }

    #[test]
    fn description_and_remediation_are_used() {
        let t = Tree::new();
        t.file("/etc/x", "");
        let r = run(
            "rules:\n  - {id: D1, title: t, severity: info, confidence: likely, category: policy, kind: file-present, path: /etc/x, description: Policy 4.2., remediation: Delete it.}\n",
            &t,
        );
        let f = &r.findings[0];
        assert_eq!(f.category, "policy");
        assert_eq!(f.confidence, Confidence::Likely);
        assert!(f.detail.starts_with("Policy 4.2."));
        assert_eq!(f.remediation, "Delete it.");
    }
}
