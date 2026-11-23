// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Offline hardening audit: SSH, sudo, PAM, login.defs and local accounts.
//!
//! Rules are data (`SSH_RULES`) where they are simple "directive has a bad
//! value" checks, and small functions where the logic needs context. Findings
//! never contain password hashes, keys or other secret material: only rule
//! ids, file locations, account names and counts.

use super::finding::{AuditReport, Confidence, Finding, Severity};
use super::root::{join_guest, GuestRoot, ReadOutcome};
use std::collections::HashSet;

const CAT: &str = "hardening";

/// Run every hardening rule group.
pub fn audit_hardening(root: &GuestRoot, report: &mut AuditReport) {
    audit_ssh(root, report);
    audit_sudo(root, report);
    audit_pam(root, report);
    audit_login_defs(root, report);
    audit_accounts(root, report);
}

/// Read a file; record a note for anything other than "missing".
pub(crate) fn read_or_note(
    root: &GuestRoot,
    report: &mut AuditReport,
    path: &str,
) -> Option<String> {
    match root.read_text(path) {
        ReadOutcome::Text(t) => Some(t),
        ReadOutcome::Missing => None,
        ReadOutcome::TooLarge(n) => {
            report.note(format!(
                "{path}: skipped, {n} bytes exceeds the {} byte read cap",
                root.limits().max_file_bytes
            ));
            None
        }
        ReadOutcome::Refused(why) => {
            report.note(format!("{path}: skipped, {why}"));
            None
        }
        ReadOutcome::Io(e) => {
            report.note(format!("{path}: unreadable ({e})"));
            None
        }
    }
}

/// Shell-style wildcard match (`*`, `?`) without recursion or backtracking
/// blowup.
pub(crate) fn wild_match(pat: &str, s: &str) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = (pat.chars().collect(), s.chars().collect());
    let (mut pi, mut si) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Expand an include pattern whose wildcards are only in the last component.
/// Returns guest paths in lexical order, capped by `max`.
fn expand_pattern(root: &GuestRoot, pattern: &str, base_dir: &str, max: usize) -> Vec<String> {
    let abs = if pattern.starts_with('/') {
        pattern.to_string()
    } else {
        format!("{}/{}", base_dir.trim_end_matches('/'), pattern)
    };
    let (dir, leaf) = match abs.rsplit_once('/') {
        Some((d, l)) => (if d.is_empty() { "/" } else { d }, l),
        None => return Vec::new(),
    };
    if dir.contains('*') || dir.contains('?') {
        return Vec::new(); // wildcards in directories are not supported
    }
    if !leaf.contains('*') && !leaf.contains('?') {
        return vec![abs];
    }
    let (names, _) = root.list_dir(dir);
    names
        .into_iter()
        .filter(|n| wild_match(leaf, n))
        .filter_map(|n| join_guest(dir, &n))
        .take(max)
        .collect()
}

// ------------------------------------------------------------------ SSH

#[derive(Debug, Clone)]
pub(crate) struct SshDirective {
    pub key: String,
    pub value: String,
    pub loc: String,
    pub in_match: bool,
}

struct SshWalk<'a> {
    root: &'a GuestRoot,
    out: Vec<SshDirective>,
    visited: HashSet<String>,
    files: usize,
    saw_match: bool,
}

fn ssh_walk(
    w: &mut SshWalk,
    report: &mut AuditReport,
    path: &str,
    depth: usize,
    in_match: &mut bool,
) {
    let lim = w.root.limits();
    if depth > lim.max_include_depth {
        report.note(format!(
            "{path}: Include nesting deeper than {} ignored",
            lim.max_include_depth
        ));
        return;
    }
    if w.files >= lim.max_files_per_group {
        report.note(format!(
            "sshd_config: more than {} files, rest ignored",
            lim.max_files_per_group
        ));
        return;
    }
    if !w.visited.insert(path.to_string()) {
        return; // include cycle or repeat
    }
    w.files += 1;
    let Some(text) = read_or_note(w.root, report, path) else {
        return;
    };
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = match line.find(|c: char| c.is_whitespace() || c == '=') {
            Some(p) => (
                &line[..p],
                line[p..].trim_start_matches(|c: char| c.is_whitespace() || c == '='),
            ),
            None => (line, ""),
        };
        let key = k.to_ascii_lowercase();
        let loc = format!("{path}:{}", i + 1);
        match key.as_str() {
            "include" => {
                for pat in v.split_whitespace() {
                    for f in expand_pattern(w.root, pat, "/etc/ssh", lim.max_files_per_group) {
                        // An Include inside a Match block stays conditional.
                        let mut m = *in_match;
                        ssh_walk(w, report, &f, depth + 1, &mut m);
                    }
                }
            }
            "match" => {
                *in_match = true;
                w.saw_match = true;
            }
            _ => w.out.push(SshDirective {
                key,
                value: v.trim().trim_matches('"').to_string(),
                loc,
                in_match: *in_match,
            }),
        }
    }
}

/// Effective (first, unconditional) value of a directive, as sshd applies it.
pub(crate) fn ssh_effective<'a>(d: &'a [SshDirective], key: &str) -> Option<&'a SshDirective> {
    d.iter().find(|x| x.key == key && !x.in_match)
}

struct SshRule {
    id: &'static str,
    key: &'static str,
    bad: fn(&str) -> bool,
    severity: Severity,
    title: &'static str,
    fix: &'static str,
}

fn is_yes(v: &str) -> bool {
    v.eq_ignore_ascii_case("yes")
}
fn is_no(v: &str) -> bool {
    v.eq_ignore_ascii_case("no")
}

const WEAK_CIPHERS: &[&str] = &[
    "3des-cbc",
    "aes128-cbc",
    "aes192-cbc",
    "aes256-cbc",
    "blowfish-cbc",
    "cast128-cbc",
    "arcfour",
    "arcfour128",
    "arcfour256",
];
const WEAK_MACS: &[&str] = &[
    "hmac-md5",
    "hmac-md5-96",
    "hmac-sha1-96",
    "hmac-ripemd160",
    "umac-64@openssh.com",
    "hmac-md5-etm@openssh.com",
    "hmac-md5-96-etm@openssh.com",
    "hmac-sha1-96-etm@openssh.com",
    "umac-64-etm@openssh.com",
];

/// A list option is weak when it *adds* (no leading `-`) a deprecated token.
fn has_weak(list: &str, weak: &[&str]) -> bool {
    if list.starts_with('-') {
        return false;
    }
    list.split(',')
        .map(|t| t.trim().trim_start_matches(['+', '^']))
        .any(|t| weak.iter().any(|w| w.eq_ignore_ascii_case(t)))
}

const SSH_RULES: &[SshRule] = &[
    SshRule {
        id: "SSH-001",
        key: "permitrootlogin",
        bad: is_yes,
        severity: Severity::High,
        title: "sshd allows direct root login with a password",
        fix: "Set `PermitRootLogin no` (or prohibit-password).",
    },
    SshRule {
        id: "SSH-002",
        key: "passwordauthentication",
        bad: is_yes,
        severity: Severity::Medium,
        title: "sshd allows password authentication",
        fix: "Set `PasswordAuthentication no` and use keys.",
    },
    SshRule {
        id: "SSH-003",
        key: "permitemptypasswords",
        bad: is_yes,
        severity: Severity::Critical,
        title: "sshd allows empty passwords",
        fix: "Set `PermitEmptyPasswords no`.",
    },
    SshRule {
        id: "SSH-004",
        key: "protocol",
        bad: |v| v.split(',').any(|p| p.trim() == "1"),
        severity: Severity::High,
        title: "sshd enables the obsolete SSH protocol 1",
        fix: "Remove `Protocol` or set it to 2.",
    },
    SshRule {
        id: "SSH-005",
        key: "x11forwarding",
        bad: is_yes,
        severity: Severity::Low,
        title: "sshd enables X11 forwarding",
        fix: "Set `X11Forwarding no` unless required.",
    },
    SshRule {
        id: "SSH-006",
        key: "ciphers",
        bad: |v| has_weak(v, WEAK_CIPHERS),
        severity: Severity::Medium,
        title: "sshd offers deprecated ciphers",
        fix: "Remove CBC/3DES/arcfour ciphers from `Ciphers`.",
    },
    SshRule {
        id: "SSH-007",
        key: "macs",
        bad: |v| has_weak(v, WEAK_MACS),
        severity: Severity::Medium,
        title: "sshd offers deprecated MACs",
        fix: "Remove MD5/SHA1-96/UMAC-64 MACs from `MACs`.",
    },
    SshRule {
        id: "SSH-008",
        key: "maxauthtries",
        bad: |v| v.parse::<u32>().map(|n| n > 6).unwrap_or(false),
        severity: Severity::Low,
        title: "sshd allows many authentication attempts per connection",
        fix: "Set `MaxAuthTries 4` or lower.",
    },
    SshRule {
        id: "SSH-009",
        key: "hostbasedauthentication",
        bad: is_yes,
        severity: Severity::Medium,
        title: "sshd enables host-based authentication",
        fix: "Set `HostbasedAuthentication no`.",
    },
    SshRule {
        id: "SSH-013",
        key: "ignorerhosts",
        bad: is_no,
        severity: Severity::Medium,
        title: "sshd honours .rhosts files",
        fix: "Set `IgnoreRhosts yes`.",
    },
    SshRule {
        id: "SSH-014",
        key: "permituserenvironment",
        bad: is_yes,
        severity: Severity::Medium,
        title: "sshd lets users set environment variables",
        fix: "Set `PermitUserEnvironment no`.",
    },
];

fn audit_ssh(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut w = SshWalk {
        root,
        out: Vec::new(),
        visited: HashSet::new(),
        files: 0,
        saw_match: false,
    };
    let cfg = "/etc/ssh/sshd_config";
    let present = root.exists(cfg);
    let mut in_match = false;
    ssh_walk(&mut w, report, cfg, 0, &mut in_match);
    if w.saw_match {
        report.note("sshd_config: `Match` blocks are conditional and are not evaluated");
    }
    let dirs = w.out;

    for rule in SSH_RULES {
        if let Some(d) = ssh_effective(&dirs, rule.key) {
            if (rule.bad)(&d.value) {
                report.push(
                    Finding::new(
                        rule.id,
                        CAT,
                        rule.severity,
                        Confidence::Confirmed,
                        rule.title,
                        rule.fix,
                    )
                    .detail(format!("`{}` is set to `{}`.", d.key, d.value))
                    .at(d.loc.clone())
                    .subject(rule.key),
                );
            }
        }
    }
    // Default-dependent: upstream sshd defaults PasswordAuthentication to yes.
    if present && ssh_effective(&dirs, "passwordauthentication").is_none() {
        report.push(
            Finding::new("SSH-002", CAT, Severity::Low, Confidence::Likely,
                "sshd password authentication is not explicitly disabled",
                "Set `PasswordAuthentication no` explicitly.")
                .detail("No unconditional `PasswordAuthentication` directive found; the upstream default is `yes`. Distribution patches or a Match block may differ.")
                .at(cfg)
                .subject("passwordauthentication"),
        );
    }

    // Baked-in host keys are shared by every clone of this image.
    let (names, _) = root.list_dir("/etc/ssh");
    let keys: Vec<&String> = names
        .iter()
        .filter(|n| n.starts_with("ssh_host_") && n.ends_with("_key"))
        .collect();
    if !keys.is_empty() {
        let list = keys
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        report.push(
            Finding::new("SSH-010", CAT, Severity::Medium, Confidence::Likely,
                "SSH host private keys are baked into the image",
                "Delete /etc/ssh/ssh_host_* before capturing the image so each clone generates its own (virt-sysprep ssh-hostkeys).")
                .detail(format!("Present (names only): {list}."))
                .at("/etc/ssh")
                .subject("host-keys"),
        );
        for k in keys {
            let p = format!("/etc/ssh/{k}");
            if let Some(m) = root.mode(&p) {
                if m & 0o077 != 0 {
                    report.push(
                        Finding::new(
                            "SSH-011",
                            CAT,
                            Severity::High,
                            Confidence::Confirmed,
                            "SSH host private key is accessible to group or others",
                            "chmod 600 the key (sshd refuses keys with loose permissions).",
                        )
                        .detail(format!("mode is {m:04o}."))
                        .at(p)
                        .subject(k.clone()),
                    );
                }
            }
        }
    }

    // authorized_keys: counts only, never key material.
    let mut with_keys: Vec<String> = Vec::new();
    let mut total = 0usize;
    let mut homes = vec![("root".to_string(), "/root".to_string())];
    let (users, _) = root.list_dir("/home");
    homes.extend(
        users
            .into_iter()
            .filter_map(|u| join_guest("/home", &u).map(|h| (u, h))),
    );
    for (user, home) in homes {
        let p = format!("{home}/.ssh/authorized_keys");
        if let Some(t) = read_or_note(root, report, &p) {
            let n = t
                .lines()
                .filter(|l| {
                    let l = l.trim();
                    !l.is_empty() && !l.starts_with('#')
                })
                .count();
            if n > 0 {
                total += n;
                with_keys.push(user);
            }
        }
    }
    if !with_keys.is_empty() {
        report.push(
            Finding::new("SSH-012", CAT, Severity::Low, Confidence::Heuristic,
                "Authorized SSH keys are baked into the image",
                "Confirm these keys are intended for every clone; otherwise remove them (virt-sysprep ssh-userdir).")
                .detail(format!("{total} key(s) for account(s): {} (key material not shown).", with_keys.join(", ")))
                .at("/home")
                .subject("authorized-keys"),
        );
    }
}

// ----------------------------------------------------------------- sudo

struct SudoWalk<'a> {
    root: &'a GuestRoot,
    visited: HashSet<String>,
    files: usize,
}

fn join_continuations(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut start = 0usize;
    for (i, l) in text.lines().enumerate() {
        if cur.is_empty() {
            start = i + 1;
        }
        if let Some(stripped) = l.strip_suffix('\\') {
            cur.push_str(stripped);
            // Bound continuation growth from hostile input.
            if cur.len() > 64 * 1024 {
                cur.truncate(64 * 1024);
            }
            continue;
        }
        cur.push_str(l);
        out.push((start, std::mem::take(&mut cur)));
    }
    if !cur.is_empty() {
        out.push((start, cur));
    }
    out
}

fn sudo_walk(w: &mut SudoWalk, report: &mut AuditReport, path: &str, depth: usize) {
    let lim = w.root.limits();
    if depth > lim.max_include_depth {
        report.note(format!(
            "{path}: sudoers include nesting deeper than {} ignored",
            lim.max_include_depth
        ));
        return;
    }
    if w.files >= lim.max_files_per_group {
        report.note(format!(
            "sudoers: more than {} files, rest ignored",
            lim.max_files_per_group
        ));
        return;
    }
    if !w.visited.insert(path.to_string()) {
        return;
    }
    w.files += 1;
    if let Some(m) = w.root.mode(path) {
        if m & 0o022 != 0 {
            report.push(
                Finding::new(
                    "SUDO-004",
                    CAT,
                    Severity::High,
                    Confidence::Confirmed,
                    "sudoers file is writable by group or others",
                    "chmod 0440 the file; sudo refuses to read it as is.",
                )
                .detail(format!("mode is {m:04o}."))
                .at(path)
                .subject(path.rsplit('/').next().unwrap_or(path)),
            );
        }
    }
    let Some(text) = read_or_note(w.root, report, path) else {
        return;
    };
    for (lineno, line) in join_continuations(&text) {
        let l = line.trim();
        let loc = format!("{path}:{lineno}");
        // Directives: `@include`, `@includedir`, and the legacy `#include`.
        let directive = l
            .strip_prefix("@includedir")
            .map(|r| ("dir", r))
            .or_else(|| l.strip_prefix("#includedir").map(|r| ("dir", r)))
            .or_else(|| l.strip_prefix("@include").map(|r| ("file", r)))
            .or_else(|| l.strip_prefix("#include").map(|r| ("file", r)));
        if let Some((kind, rest)) = directive {
            let target = rest.trim().trim_matches('"');
            if target.starts_with('/') && !target.contains('%') {
                if kind == "dir" {
                    let (names, trunc) = w.root.list_dir(target);
                    if trunc {
                        report.note(format!("{target}: directory listing truncated"));
                    }
                    for n in names {
                        // sudo skips names containing `.` or ending in `~`.
                        if n.contains('.') || n.ends_with('~') {
                            continue;
                        }
                        if let Some(f) = join_guest(target, &n) {
                            sudo_walk(w, report, &f, depth + 1);
                        }
                    }
                } else {
                    sudo_walk(w, report, target, depth + 1);
                }
            }
            continue;
        }
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if l.starts_with("Defaults") {
            let body = l
                .split_once(char::is_whitespace)
                .map(|x| x.1)
                .unwrap_or("")
                .trim();
            for opt in body.split(',').map(str::trim) {
                if opt == "!authenticate" {
                    report.push(
                        Finding::new(
                            "SUDO-002",
                            CAT,
                            Severity::High,
                            Confidence::Confirmed,
                            "sudo authentication is disabled globally",
                            "Remove `!authenticate` from Defaults.",
                        )
                        .at(loc.clone())
                        .subject("!authenticate"),
                    );
                } else if opt == "!env_reset" {
                    report.push(
                        Finding::new(
                            "SUDO-003",
                            CAT,
                            Severity::Medium,
                            Confidence::Confirmed,
                            "sudo does not reset the environment",
                            "Remove `!env_reset` from Defaults.",
                        )
                        .at(loc.clone())
                        .subject("!env_reset"),
                    );
                }
            }
            continue;
        }
        if l.contains("NOPASSWD") {
            let who = l.split_whitespace().next().unwrap_or("?");
            if ["User_Alias", "Cmnd_Alias", "Host_Alias", "Runas_Alias"].contains(&who) {
                continue;
            }
            let after = l.rsplit("NOPASSWD").next().unwrap_or("");
            let all_cmds = after
                .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
                .split(',')
                .any(|c| c.trim() == "ALL");
            let sev = if all_cmds {
                Severity::High
            } else {
                Severity::Medium
            };
            report.push(
                Finding::new(
                    "SUDO-001",
                    CAT,
                    sev,
                    Confidence::Confirmed,
                    if all_cmds {
                        "Passwordless sudo for ALL commands"
                    } else {
                        "Passwordless sudo for selected commands"
                    },
                    "Require a password, or restrict NOPASSWD to the minimum command list.",
                )
                .detail("Expected on some cloud images (default user); verify it is intended.")
                .at(loc)
                .subject(who),
            );
        }
    }
}

fn audit_sudo(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut w = SudoWalk {
        root,
        visited: HashSet::new(),
        files: 0,
    };
    sudo_walk(&mut w, report, "/etc/sudoers", 0);
    // Debian/Ubuntu/RHEL ship `@includedir /etc/sudoers.d`; if the main file is
    // missing but the directory exists, still audit it.
    if !w.visited.contains("/etc/sudoers.d")
        && !root.exists("/etc/sudoers")
        && root.is_dir("/etc/sudoers.d")
    {
        let (names, _) = root.list_dir("/etc/sudoers.d");
        for n in names
            .into_iter()
            .filter(|n| !n.contains('.') && !n.ends_with('~'))
        {
            if let Some(f) = join_guest("/etc/sudoers.d", &n) {
                sudo_walk(&mut w, report, &f, 1);
            }
        }
    }
}

// ------------------------------------------------------------------ PAM

fn audit_pam(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    if !root.is_dir("/etc/pam.d") {
        return;
    }
    let (names, trunc) = root.list_dir("/etc/pam.d");
    if trunc {
        report.note("/etc/pam.d: directory listing truncated");
    }
    let mut lockout = false;
    let mut parsed = 0usize;
    for n in names {
        if parsed >= root.limits().max_files_per_group {
            report.note("/etc/pam.d: file cap reached, rest ignored");
            break;
        }
        let Some(path) = join_guest("/etc/pam.d", &n) else {
            continue;
        };
        let Some(text) = read_or_note(root, report, &path) else {
            continue;
        };
        parsed += 1;
        for (i, raw) in text.lines().enumerate() {
            let l = raw.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with('@') {
                continue;
            }
            let mut it = l.split_whitespace();
            let ty = it.next().unwrap_or("").trim_start_matches('-');
            let rest = it.collect::<Vec<_>>().join(" ");
            // control is either a word or a `[...]` group
            let (control, tail) = if rest.starts_with('[') {
                match rest.find(']') {
                    Some(e) => (rest[..=e].to_string(), rest[e + 1..].trim().to_string()),
                    None => continue,
                }
            } else {
                match rest.split_once(' ') {
                    Some((c, t)) => (c.to_string(), t.trim().to_string()),
                    None => continue,
                }
            };
            let mut parts = tail.split_whitespace();
            let module = parts.next().unwrap_or("");
            let args: Vec<&str> = parts.collect();
            let loc = format!("{path}:{}", i + 1);
            let mname = module.rsplit('/').next().unwrap_or(module);
            if mname.starts_with("pam_faillock") || mname.starts_with("pam_tally") {
                lockout = true;
            }
            if mname == "pam_unix.so"
                && ty == "auth"
                && args.iter().any(|a| *a == "nullok" || *a == "nullok_secure")
            {
                report.push(
                    Finding::new(
                        "PAM-001",
                        CAT,
                        Severity::Medium,
                        Confidence::Confirmed,
                        "pam_unix accepts empty passwords (nullok)",
                        "Remove `nullok` from pam_unix auth lines.",
                    )
                    .at(loc.clone())
                    .subject(n.clone()),
                );
            }
            if mname == "pam_permit.so"
                && (ty == "auth" || ty == "account")
                && control == "sufficient"
            {
                report.push(
                    Finding::new("PAM-002", CAT, Severity::Critical, Confidence::Likely,
                        "pam_permit is `sufficient`: the stack can succeed without credentials",
                        "Use pam_permit only as the final `required` fallback of a deny-first stack.")
                        .at(loc.clone()).subject(n.clone()),
                );
            }
            if mname == "pam_unix.so"
                && ty == "password"
                && args.iter().any(|a| *a == "md5" || *a == "bigcrypt")
            {
                report.push(
                    Finding::new(
                        "PAM-003",
                        CAT,
                        Severity::Medium,
                        Confidence::Confirmed,
                        "pam_unix hashes new passwords with a weak algorithm",
                        "Use sha512 or yescrypt.",
                    )
                    .at(loc)
                    .subject(n.clone()),
                );
            }
        }
    }
    if parsed > 0 && !lockout {
        report.push(
            Finding::new("PAM-004", CAT, Severity::Low, Confidence::Heuristic,
                "No account lockout module (pam_faillock/pam_tally2) found in /etc/pam.d",
                "Add pam_faillock to the auth stack. Lockout may be handled elsewhere (SSH-level tools, identity provider).")
                .at("/etc/pam.d").subject("lockout"),
        );
    }
}

// ----------------------------------------------------------- login.defs

fn audit_login_defs(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let path = "/etc/login.defs";
    let Some(text) = read_or_note(root, report, path) else {
        return;
    };
    for (i, raw) in text.lines().enumerate() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let mut it = l.split_whitespace();
        let (Some(k), Some(v)) = (it.next(), it.next()) else {
            continue;
        };
        let loc = format!("{path}:{}", i + 1);
        match k {
            "ENCRYPT_METHOD" if v.eq_ignore_ascii_case("MD5") || v.eq_ignore_ascii_case("DES") => {
                report.push(
                    Finding::new(
                        "LOGIN-001",
                        CAT,
                        Severity::High,
                        Confidence::Confirmed,
                        "Password hashing method is weak",
                        "Set `ENCRYPT_METHOD SHA512` (or YESCRYPT).",
                    )
                    .detail(format!("ENCRYPT_METHOD is {v}."))
                    .at(loc)
                    .subject(k),
                );
            }
            "PASS_MAX_DAYS" => {
                if v.parse::<i64>()
                    .map(|n| !(0..=365).contains(&n))
                    .unwrap_or(false)
                {
                    report.push(
                        Finding::new(
                            "LOGIN-002",
                            CAT,
                            Severity::Low,
                            Confidence::Likely,
                            "Passwords effectively never expire",
                            "Set PASS_MAX_DAYS to a policy value (for example 365 or less).",
                        )
                        .detail(format!(
                            "PASS_MAX_DAYS is {v}; applies to accounts created later."
                        ))
                        .at(loc)
                        .subject(k),
                    );
                }
            }
            "UMASK" => {
                if let Ok(u) = u32::from_str_radix(v, 8) {
                    if u & 0o022 != 0o022 {
                        report.push(
                            Finding::new(
                                "LOGIN-003",
                                CAT,
                                Severity::Low,
                                Confidence::Likely,
                                "Default umask leaves new files group- or world-writable",
                                "Use UMASK 027 or 077.",
                            )
                            .detail(format!("UMASK is {v}."))
                            .at(loc)
                            .subject(k),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------- accounts

const LOGIN_SHELLS: &[&str] = &["bash", "sh", "zsh", "dash", "ksh", "csh", "tcsh", "fish"];

fn audit_accounts(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    if let Some(m) = root.mode("/etc/shadow") {
        if m & 0o007 != 0 {
            report.push(
                Finding::new(
                    "ACCT-006",
                    CAT,
                    Severity::High,
                    Confidence::Confirmed,
                    "/etc/shadow is readable or writable by others",
                    "chmod 0640 (root:shadow) or 0600.",
                )
                .detail(format!("mode is {m:04o}."))
                .at("/etc/shadow")
                .subject("shadow-mode"),
            );
        }
    }
    if let Some(m) = root.mode("/etc/passwd") {
        if m & 0o022 != 0 {
            report.push(
                Finding::new(
                    "ACCT-006",
                    CAT,
                    Severity::High,
                    Confidence::Confirmed,
                    "/etc/passwd is writable by group or others",
                    "chmod 0644.",
                )
                .detail(format!("mode is {m:04o}."))
                .at("/etc/passwd")
                .subject("passwd-mode"),
            );
        }
    }

    let mut malformed = 0usize;
    if let Some(text) = read_or_note(root, report, "/etc/passwd") {
        let mut seen_uid0 = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let l = raw.trim_end();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let loc = format!("/etc/passwd:{}", i + 1);
            if l.starts_with('+') || l.starts_with('-') {
                report.push(
                    Finding::new(
                        "ACCT-003",
                        CAT,
                        Severity::Medium,
                        Confidence::Confirmed,
                        "Legacy NIS `+`/`-` entry in /etc/passwd",
                        "Remove the compat entry unless NIS is intentionally used.",
                    )
                    .at(loc)
                    .subject(l.split(':').next().unwrap_or("+").to_string()),
                );
                continue;
            }
            let f: Vec<&str> = l.split(':').collect();
            if f.len() < 7 {
                malformed += 1;
                continue;
            }
            let (name, pw, uid, shell) = (f[0], f[1], f[2], f[6]);
            let Ok(uid) = uid.parse::<u32>() else {
                malformed += 1;
                continue;
            };
            if uid == 0 && name != "root" {
                seen_uid0.push(name.to_string());
                report.push(
                    Finding::new(
                        "ACCT-001",
                        CAT,
                        Severity::Critical,
                        Confidence::Confirmed,
                        "Non-root account with UID 0",
                        "Remove the account or give it a unique UID.",
                    )
                    .at(loc.clone())
                    .subject(name.to_string()),
                );
            }
            if pw.is_empty() {
                report.push(
                    Finding::new(
                        "ACCT-002",
                        CAT,
                        Severity::Critical,
                        Confidence::Confirmed,
                        "Account has an empty password field in /etc/passwd",
                        "Lock the account (`passwd -l`) or set a password.",
                    )
                    .at(loc.clone())
                    .subject(name.to_string()),
                );
            }
            let base = shell.rsplit('/').next().unwrap_or("");
            if (1..1000).contains(&uid) && LOGIN_SHELLS.contains(&base) {
                report.push(
                    Finding::new(
                        "ACCT-004",
                        CAT,
                        Severity::Low,
                        Confidence::Heuristic,
                        "System account has an interactive login shell",
                        "Use /usr/sbin/nologin for service accounts.",
                    )
                    .detail(format!("uid {uid}, shell {shell}."))
                    .at(loc)
                    .subject(name.to_string()),
                );
            }
        }
        let _ = seen_uid0;
    }
    if let Some(text) = read_or_note(root, report, "/etc/shadow") {
        for (i, raw) in text.lines().enumerate() {
            let l = raw.trim_end();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let mut f = l.split(':');
            let (Some(name), Some(hash)) = (f.next(), f.next()) else {
                malformed += 1;
                continue;
            };
            let loc = format!("/etc/shadow:{}", i + 1);
            // The hash itself is never copied into the report.
            if hash.is_empty() {
                report.push(
                    Finding::new(
                        "ACCT-002",
                        CAT,
                        Severity::Critical,
                        Confidence::Confirmed,
                        "Account has an empty password hash",
                        "Lock the account (`passwd -l`) or set a password.",
                    )
                    .at(loc)
                    .subject(name.to_string()),
                );
            } else if hash.starts_with("$1$") {
                report.push(
                    Finding::new(
                        "ACCT-005",
                        CAT,
                        Severity::High,
                        Confidence::Confirmed,
                        "Password stored with MD5-crypt",
                        "Force a password change so it is re-hashed with SHA-512 or yescrypt.",
                    )
                    .detail("Algorithm: MD5-crypt ($1$).")
                    .at(loc)
                    .subject(name.to_string()),
                );
            } else if !hash.starts_with('$')
                && !hash.starts_with('!')
                && !hash.starts_with('*')
                && hash.len() == 13
            {
                report.push(
                    Finding::new(
                        "ACCT-005",
                        CAT,
                        Severity::High,
                        Confidence::Likely,
                        "Password stored with legacy DES crypt",
                        "Force a password change so it is re-hashed with SHA-512 or yescrypt.",
                    )
                    .detail("Algorithm: DES-crypt (13-character hash).")
                    .at(loc)
                    .subject(name.to_string()),
                );
            }
        }
    }
    if malformed > 0 {
        report.note(format!(
            "{malformed} malformed line(s) in /etc/passwd or /etc/shadow were skipped (corrupt or truncated file?)"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::super::root::fixture::Tree;
    use super::super::root::Limits;
    use super::*;

    fn run(t: &Tree) -> AuditReport {
        let mut r = AuditReport::new("test");
        audit_hardening(&t.root(), &mut r);
        r.finish();
        r
    }
    fn ids(r: &AuditReport) -> Vec<String> {
        r.findings.iter().map(|f| f.id.clone()).collect()
    }
    fn has(r: &AuditReport, id: &str, subject: &str) -> bool {
        r.findings
            .iter()
            .any(|f| f.id == id && f.subject.as_deref() == Some(subject))
    }

    #[test]
    fn wildcard_matcher() {
        assert!(wild_match("*.conf", "a.conf"));
        assert!(!wild_match("*.conf", "a.conf.bak"));
        assert!(wild_match("a?c", "abc"));
        assert!(wild_match("*", ""));
        // pathological pattern must stay fast
        assert!(!wild_match(&"*a".repeat(200), &"a".repeat(199)));
    }

    #[test]
    fn ssh_first_value_wins_and_dropin_overrides() {
        let t = Tree::new();
        t.file("/etc/ssh/sshd_config", "Include /etc/ssh/sshd_config.d/*.conf\nPermitRootLogin yes\nPasswordAuthentication yes\n");
        t.file("/etc/ssh/sshd_config.d/50-x.conf", "PermitRootLogin no\n");
        let r = run(&t);
        assert!(!has(&r, "SSH-001", "permitrootlogin"), "{:?}", ids(&r));
        let f = r.findings.iter().find(|f| f.id == "SSH-002").unwrap();
        assert_eq!(f.severity, Severity::Medium);
        assert_eq!(f.location.as_deref(), Some("/etc/ssh/sshd_config:3"));
    }

    #[test]
    fn ssh_root_login_and_weak_crypto_flagged() {
        let t = Tree::new();
        t.file("/etc/ssh/sshd_config", "PermitRootLogin yes\nPermitEmptyPasswords yes\nCiphers aes128-ctr,3des-cbc\nMACs -hmac-md5\nMaxAuthTries 10\nPasswordAuthentication no\n");
        let r = run(&t);
        assert!(has(&r, "SSH-001", "permitrootlogin"));
        assert!(has(&r, "SSH-003", "permitemptypasswords"));
        assert!(has(&r, "SSH-006", "ciphers"));
        assert!(!has(&r, "SSH-007", "macs"), "removal list is not weak");
        assert!(has(&r, "SSH-008", "maxauthtries"));
        assert!(!r.findings.iter().any(|f| f.id == "SSH-002"));
    }

    #[test]
    fn ssh_match_block_is_conditional_and_noted() {
        let t = Tree::new();
        t.file(
            "/etc/ssh/sshd_config",
            "PasswordAuthentication no\nMatch User bob\n  PermitRootLogin yes\n",
        );
        let r = run(&t);
        assert!(!has(&r, "SSH-001", "permitrootlogin"));
        assert!(r.notes.iter().any(|n| n.contains("Match")));
    }

    #[test]
    fn ssh_default_password_auth_is_likely_not_confirmed() {
        let t = Tree::new();
        t.file("/etc/ssh/sshd_config", "# nothing\n");
        let r = run(&t);
        let f = r.findings.iter().find(|f| f.id == "SSH-002").unwrap();
        assert_eq!(f.confidence, Confidence::Likely);
    }

    #[test]
    fn ssh_include_cycle_and_depth_terminate() {
        let t = Tree::new();
        t.file("/etc/ssh/sshd_config", "Include /etc/ssh/a.conf\n");
        t.file(
            "/etc/ssh/a.conf",
            "Include /etc/ssh/sshd_config\nInclude /etc/ssh/a.conf\nPermitRootLogin yes\n",
        );
        let r = run(&t);
        assert!(has(&r, "SSH-001", "permitrootlogin"));
    }

    #[test]
    fn ssh_include_fanout_is_capped() {
        let t = Tree::new();
        t.file("/etc/ssh/sshd_config", "Include /etc/ssh/d/*.conf\n");
        for i in 0..40 {
            t.file(&format!("/etc/ssh/d/{i:03}.conf"), "X11Forwarding yes\n");
        }
        let mut rep = AuditReport::new("t");
        let root = t.root_with(Limits {
            max_files_per_group: 5,
            ..Limits::default()
        });
        audit_hardening(&root, &mut rep);
        assert!(
            rep.notes.iter().any(|n| n.contains("more than 5 files")),
            "{:?}",
            rep.notes
        );
    }

    #[test]
    fn ssh_host_keys_and_authorized_keys_never_leak_material() {
        let t = Tree::new();
        t.file("/etc/ssh/sshd_config", "PasswordAuthentication no\n");
        t.file(
            "/etc/ssh/ssh_host_ed25519_key",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nSECRETSECRET\n",
        );
        t.mode("/etc/ssh/ssh_host_ed25519_key", 0o644);
        t.file(
            "/root/.ssh/authorized_keys",
            "ssh-ed25519 AAAASECRETKEYMATERIAL me@x\n# c\n",
        );
        let r = run(&t);
        assert!(has(&r, "SSH-010", "host-keys"));
        assert!(has(&r, "SSH-011", "ssh_host_ed25519_key"));
        assert!(has(&r, "SSH-012", "authorized-keys"));
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("SECRETSECRET") && !json.contains("AAAASECRET"));
    }

    #[test]
    fn sudo_nopasswd_levels_and_includedir() {
        let t = Tree::new();
        t.file(
            "/etc/sudoers",
            "Defaults !authenticate\nroot ALL=(ALL) ALL\n@includedir /etc/sudoers.d\n",
        );
        t.file(
            "/etc/sudoers.d/90-cloud",
            "ec2-user ALL=(ALL) NOPASSWD:ALL\n",
        );
        t.file(
            "/etc/sudoers.d/ops",
            "%ops ALL=(ALL) NOPASSWD: /usr/bin/systemctl, \\\n /usr/bin/journalctl\n",
        );
        t.file(
            "/etc/sudoers.d/skipped.bak",
            "evil ALL=(ALL) NOPASSWD: ALL\n",
        );
        let r = run(&t);
        assert!(has(&r, "SUDO-002", "!authenticate"));
        let a = r
            .findings
            .iter()
            .find(|f| f.subject.as_deref() == Some("ec2-user"))
            .unwrap();
        assert_eq!(a.severity, Severity::High);
        let b = r
            .findings
            .iter()
            .find(|f| f.subject.as_deref() == Some("%ops"))
            .unwrap();
        assert_eq!(b.severity, Severity::Medium);
        assert!(
            !has(&r, "SUDO-001", "evil"),
            "files with a dot are skipped by sudo"
        );
    }

    #[test]
    fn sudo_include_loop_terminates_and_loose_mode_flagged() {
        let t = Tree::new();
        t.file(
            "/etc/sudoers",
            "@include /etc/sudoers\n#include /etc/sudoers.d/x\n",
        );
        t.file("/etc/sudoers.d/x", "@include /etc/sudoers\n");
        t.mode("/etc/sudoers.d/x", 0o666);
        let r = run(&t);
        assert!(r.findings.iter().any(|f| f.id == "SUDO-004"));
    }

    #[test]
    fn pam_rules() {
        let t = Tree::new();
        t.file(
            "/etc/pam.d/common-auth",
            "auth [success=1 default=ignore] pam_unix.so nullok\nauth requisite pam_deny.so\n",
        );
        t.file(
            "/etc/pam.d/bad",
            "auth sufficient pam_permit.so\npassword required pam_unix.so md5 shadow\n",
        );
        let r = run(&t);
        assert!(has(&r, "PAM-001", "common-auth"));
        assert!(has(&r, "PAM-002", "bad"));
        assert!(has(&r, "PAM-003", "bad"));
        assert!(r.findings.iter().any(|f| f.id == "PAM-004"));
        let t2 = Tree::new();
        t2.file(
            "/etc/pam.d/system-auth",
            "auth required pam_faillock.so preauth\n",
        );
        assert!(!run(&t2).findings.iter().any(|f| f.id == "PAM-004"));
    }

    #[test]
    fn login_defs_rules() {
        let t = Tree::new();
        t.file(
            "/etc/login.defs",
            "ENCRYPT_METHOD MD5\nPASS_MAX_DAYS 99999\nUMASK 002\n",
        );
        let r = run(&t);
        assert!(has(&r, "LOGIN-001", "ENCRYPT_METHOD"));
        assert!(has(&r, "LOGIN-002", "PASS_MAX_DAYS"));
        assert!(has(&r, "LOGIN-003", "UMASK"));
    }

    #[test]
    fn account_rules_without_leaking_hashes() {
        let t = Tree::new();
        t.file("/etc/passwd", "root:x:0:0:r:/root:/bin/bash\ntoor:x:0:0::/:/bin/sh\nftp::14:50::/srv:/sbin/nologin\ndaemon:x:2:2::/:/bin/bash\n+::::::\n");
        t.file("/etc/shadow", "root:$6$salt$HASHVALUEHASH:19000:::::\nold:$1$abc$MD5HASH:1::::::\ndes:ab1CdEfGhIjKl:1::::::\nempty::1::::::\nlocked:!!:1::::::\n");
        t.mode("/etc/shadow", 0o644);
        let r = run(&t);
        assert!(has(&r, "ACCT-001", "toor"));
        assert!(!has(&r, "ACCT-001", "root"));
        assert!(has(&r, "ACCT-002", "ftp"));
        assert!(has(&r, "ACCT-002", "empty"));
        assert!(has(&r, "ACCT-003", "+"));
        assert!(has(&r, "ACCT-004", "daemon"));
        assert!(has(&r, "ACCT-005", "old"));
        assert!(has(&r, "ACCT-005", "des"));
        assert!(!has(&r, "ACCT-005", "locked"));
        assert!(has(&r, "ACCT-006", "shadow-mode"));
        let json = serde_json::to_string(&r).unwrap();
        assert!(
            !json.contains("HASHVALUEHASH")
                && !json.contains("MD5HASH")
                && !json.contains("ab1CdEfGhIjKl")
        );
    }

    #[test]
    fn corrupt_and_truncated_account_files_do_not_panic() {
        let t = Tree::new();
        t.bytes("/etc/passwd", b"root:x:0:0:r:/root:/bin/bash\nbroken\xff\xfe:x\nname:x:notanumber:0::/:/bin/sh\nhalf:x:5");
        t.file("/etc/shadow", "justaname\n:\n");
        let r = run(&t);
        assert!(r.notes.iter().any(|n| n.contains("malformed")));
    }

    #[test]
    fn oversized_config_is_skipped_with_note() {
        let t = Tree::new();
        t.bytes("/etc/ssh/sshd_config", &vec![b'#'; 4096]);
        let mut rep = AuditReport::new("t");
        let root = t.root_with(Limits {
            max_file_bytes: 1024,
            ..Limits::default()
        });
        audit_hardening(&root, &mut rep);
        assert!(rep.notes.iter().any(|n| n.contains("read cap")));
    }

    #[test]
    fn symlinked_config_cannot_read_host_files() {
        let t = Tree::new();
        t.link("/etc/ssh/sshd_config", "/etc/shadow-host-escape");
        t.link("/etc/passwd", "../../../../../../../etc/passwd");
        let r = run(&t);
        assert!(!ids(&r).contains(&"ACCT-001".to_string()));
    }

    #[test]
    fn empty_tree_yields_no_findings() {
        assert!(run(&Tree::new()).findings.is_empty());
    }
}
