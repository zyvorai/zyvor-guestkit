// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Baked-in identity and leftover-state checks ("clone safety").
//!
//! An image that is cloned or published with a machine-id, DHCP leases, shell
//! history, a registered subscription or a configuration-management identity
//! still inside it makes every clone share that state. The checklist follows
//! the operations `virt-sysprep` performs. Everything is read-only; findings
//! name files and counts, never contents (history lines, keytabs and
//! certificates are not copied).

use super::finding::{AuditReport, Confidence, Finding, Severity};
use super::root::{join_guest, GuestRoot};
use std::collections::BTreeSet;

const CAT: &str = "identity";

/// Run every identity rule group.
pub fn audit_identity(root: &GuestRoot, report: &mut AuditReport) {
    machine_id(root, report);
    dhcp_leases(root, report);
    shell_history(root, report);
    random_seed(root, report);
    scheduled_jobs(root, report);
    account_backups(root, report);
    mail_spool(root, report);
    registration_identity(root, report);
    config_management(root, report);
    kerberos_keytab(root, report);
    leftover_data(root, report);
}

/// Regular, non-empty files directly inside `dir` (names sorted), noting when
/// the directory listing was truncated.
fn files_in(root: &GuestRoot, report: &mut AuditReport, dir: &str) -> Vec<String> {
    let (names, truncated) = root.list_dir(dir);
    if truncated {
        report.note(format!(
            "{dir}: listing truncated at {} entries",
            root.limits().max_dir_entries
        ));
    }
    names
        .into_iter()
        .filter_map(|n| join_guest(dir, &n))
        .filter(|p| root.size(p).map(|s| s > 0).unwrap_or(false))
        .collect()
}

/// Home directories to inspect: `/root` plus the entries of `/home`.
fn home_dirs(root: &GuestRoot, report: &mut AuditReport) -> Vec<String> {
    let mut v = vec!["/root".to_string()];
    let (names, truncated) = root.list_dir("/home");
    if truncated {
        report.note("/home: listing truncated, some home directories were not checked");
    }
    v.extend(names.iter().filter_map(|n| join_guest("/home", n)));
    v
}

fn machine_id(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        // A symlink from the dbus copy to /etc/machine-id is the normal layout.
        if path != "/etc/machine-id" && root.is_dangling_symlink(path) {
            continue;
        }
        let Some(text) = root.read_text(path).text() else {
            continue;
        };
        let v = text.trim();
        // Empty, "uninitialized" (systemd first-boot marker) and an all-zero
        // placeholder are the correct states of a sealed image.
        if v.is_empty() || v == "uninitialized" || v.chars().all(|c| c == '0') {
            continue;
        }
        if path == "/var/lib/dbus/machine-id" && root.size("/etc/machine-id").is_some() {
            // Same id reached through a symlink or a duplicate; one finding is enough.
            let main = root.read_text("/etc/machine-id").text().unwrap_or_default();
            if main.trim() == v {
                continue;
            }
        }
        report.push(
            Finding::new(
                "IDENT-001",
                CAT,
                Severity::Medium,
                Confidence::Confirmed,
                "A machine-id is baked into the image",
                "Truncate it (`: > /etc/machine-id`) or run `virt-sysprep --operations machine-id` before cloning.",
            )
            .detail("Every clone would share one machine-id, which breaks DHCP client ids, journal ids and licence or telemetry identity.")
            .at(path)
            .subject("machine-id"),
        );
    }
}

fn dhcp_leases(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut found: Vec<String> = Vec::new();
    for dir in [
        "/var/lib/dhcp",
        "/var/lib/dhclient",
        "/var/lib/NetworkManager",
        "/var/lib/dhcpcd",
        "/var/db",
    ] {
        for p in files_in(root, report, dir) {
            let lower = p.to_ascii_lowercase();
            if lower.contains("lease") {
                found.push(p);
            }
        }
    }
    if let Some(first) = found.first() {
        report.push(
            Finding::new(
                "IDENT-002",
                CAT,
                Severity::Low,
                Confidence::Confirmed,
                "DHCP lease files are baked into the image",
                "Remove them (`virt-sysprep --operations dhcp-client-state`); a clone may try to renew another machine's lease.",
            )
            .detail(format!("{} lease file(s) with content.", found.len()))
            .at(first.clone())
            .subject("dhcp-leases"),
        );
    }
}

fn shell_history(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    const FILES: &[&str] = &[
        ".bash_history",
        ".zsh_history",
        ".history",
        ".python_history",
        ".mysql_history",
        ".psql_history",
        ".sqlite_history",
        ".node_repl_history",
        ".lesshst",
    ];
    for home in home_dirs(root, report) {
        for f in FILES {
            let Some(path) = join_guest(&home, f) else {
                continue;
            };
            let Some(size) = root.size(&path).filter(|s| *s > 0) else {
                continue;
            };
            let lines = root
                .read_text(&path)
                .text()
                .map(|t| t.lines().count().to_string())
                .unwrap_or_else(|| "many".into());
            report.push(
                Finding::new(
                    "IDENT-003",
                    CAT,
                    Severity::Medium,
                    Confidence::Confirmed,
                    "Shell or tool history is left in the image",
                    "Delete it (`virt-sysprep --operations bash-history`); history often contains pasted passwords and tokens.",
                )
                .detail(format!("{size} bytes, {lines} line(s); contents are not copied into this report."))
                .at(path)
                .subject(*f),
            );
        }
    }
}

fn random_seed(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    for path in ["/var/lib/systemd/random-seed", "/var/lib/random-seed"] {
        if root.size(path).map(|s| s > 0).unwrap_or(false) {
            report.push(
                Finding::new(
                    "IDENT-004",
                    CAT,
                    Severity::Low,
                    Confidence::Confirmed,
                    "A saved random seed is baked into the image",
                    "Remove it so each clone seeds its own entropy pool on first boot.",
                )
                .detail("Clones of this image start with identical saved entropy until the kernel gathers more.")
                .at(path)
                .subject("random-seed"),
            );
        }
    }
}

fn scheduled_jobs(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut found = 0usize;
    let mut first: Option<String> = None;
    for dir in [
        "/var/spool/cron",
        "/var/spool/cron/crontabs",
        "/var/spool/cron/atjobs",
        "/var/spool/at",
    ] {
        for p in files_in(root, report, dir) {
            // .SEQ is the at(1) sequence counter, not a job.
            if p.ends_with(".SEQ") {
                continue;
            }
            found += 1;
            first.get_or_insert(p);
        }
    }
    if let Some(p) = first {
        report.push(
            Finding::new(
                "IDENT-005",
                CAT,
                Severity::Low,
                Confidence::Confirmed,
                "User cron or at jobs are baked into the image",
                "Remove them (`virt-sysprep --operations cron-spool`) unless every clone should run them.",
            )
            .detail(format!("{found} spool file(s) with content."))
            .at(p)
            .subject("cron-spool"),
        );
    }
}

fn account_backups(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut names: BTreeSet<String> = BTreeSet::new();
    for base in ["passwd", "shadow", "group", "gshadow"] {
        for suffix in ["-", ".bak", ".old", "~", ".orig"] {
            let p = format!("/etc/{base}{suffix}");
            if root.size(&p).map(|s| s > 0).unwrap_or(false) {
                names.insert(p);
            }
        }
    }
    // /etc/shadow- and /etc/gshadow- hold password hashes; the others only
    // account names. Severity follows what the copy can leak.
    let sensitive = names.iter().any(|p| p.contains("shadow"));
    if let Some(first) = names.iter().next() {
        report.push(
            Finding::new(
                "IDENT-006",
                CAT,
                if sensitive {
                    Severity::Medium
                } else {
                    Severity::Low
                },
                Confidence::Confirmed,
                "Backup copies of account files are in the image",
                "Remove them (`virt-sysprep --operations passwd-backups`); they can hold older password hashes.",
            )
            .detail(format!(
                "{}.",
                names.iter().cloned().collect::<Vec<_>>().join(", ")
            ))
            .at(first.clone())
            .subject("account-backups"),
        );
    }
}

fn mail_spool(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut found: Vec<String> = Vec::new();
    for dir in ["/var/spool/mail", "/var/mail"] {
        found.extend(files_in(root, report, dir));
    }
    found.sort();
    found.dedup();
    if let Some(first) = found.first() {
        report.push(
            Finding::new(
                "IDENT-007",
                CAT,
                Severity::Low,
                Confidence::Confirmed,
                "Local mail is left in the spool",
                "Remove it (`virt-sysprep --operations mail-spool`); system mail often contains cron output and secrets.",
            )
            .detail(format!("{} mailbox file(s) with content.", found.len()))
            .at(first.clone())
            .subject("mail-spool"),
        );
    }
}

fn registration_identity(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut found: Vec<String> = Vec::new();
    for dir in ["/etc/pki/consumer", "/etc/pki/entitlement"] {
        found.extend(files_in(root, report, dir));
    }
    for p in ["/etc/sysconfig/rhn/systemid", "/var/lib/yum/uuid"] {
        if root.size(p).map(|s| s > 0).unwrap_or(false) {
            found.push(p.to_string());
        }
    }
    if let Some(first) = found.first() {
        report.push(
            Finding::new(
                "IDENT-008",
                CAT,
                Severity::Medium,
                Confidence::Confirmed,
                "Subscription or registration identity is baked into the image",
                "Unregister before capture (`subscription-manager unregister`, or `virt-sysprep --operations rh-subscription-manager,rhn-systemid,yum-uuid`).",
            )
            .detail(format!(
                "{} identity or entitlement file(s); clones would consume or share one registration.",
                found.len()
            ))
            .at(first.clone())
            .subject("registration"),
        );
    }
}

fn config_management(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    // (label, probe path). Presence of the client key or certificate means the
    // image carries an enrolled node identity.
    const PROBES: &[(&str, &str)] = &[
        ("salt-minion", "/etc/salt/pki/minion/minion.pem"),
        ("salt-minion", "/etc/salt/minion_id"),
        ("chef-client", "/etc/chef/client.pem"),
        ("puppet-agent", "/etc/puppetlabs/puppet/ssl/private_keys"),
        ("puppet-agent", "/var/lib/puppet/ssl/private_keys"),
        ("ansible-pull", "/etc/ansible/facts.d"),
        ("zabbix-agent", "/etc/zabbix/zabbix_agentd.psk"),
    ];
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (label, path) in PROBES {
        let present = if root.is_dir(path) {
            !root.list_dir(path).0.is_empty()
        } else {
            root.size(path).map(|s| s > 0).unwrap_or(false)
        };
        if !present || !seen.insert(label) {
            continue;
        }
        let sev = if *label == "ansible-pull" {
            Severity::Info
        } else {
            Severity::Medium
        };
        report.push(
            Finding::new(
                "IDENT-009",
                CAT,
                sev,
                Confidence::Confirmed,
                format!("{label} node identity is baked into the image"),
                "Remove the enrolled key or certificate before capture so each clone enrols itself.",
            )
            .detail("Clones would present the same node identity to the management server; key material is not copied into this report.")
            .at(*path)
            .subject(*label),
        );
    }
}

fn kerberos_keytab(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    if root
        .size("/etc/krb5.keytab")
        .map(|s| s > 0)
        .unwrap_or(false)
    {
        report.push(
            Finding::new(
                "IDENT-010",
                CAT,
                Severity::High,
                Confidence::Confirmed,
                "A Kerberos host keytab is baked into the image",
                "Remove it (`virt-sysprep --operations kerberos-hostkeytab`) and rejoin the domain on each clone.",
            )
            .detail("The host keytab authenticates the machine to the realm; every clone would impersonate the same host principal.")
            .at("/etc/krb5.keytab")
            .subject("krb5.keytab"),
        );
    }
}

fn leftover_data(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    // Files left in /tmp and /var/tmp (systemd-private-* and lost+found are noise).
    let mut tmp = 0usize;
    for dir in ["/tmp", "/var/tmp"] {
        let (names, truncated) = root.list_dir(dir);
        if truncated {
            report.note(format!("{dir}: listing truncated"));
        }
        tmp += names
            .iter()
            .filter(|n| !n.starts_with("systemd-private-") && *n != "lost+found")
            .count();
    }
    if tmp > 0 {
        report.push(
            Finding::new(
                "IDENT-011",
                CAT,
                Severity::Info,
                Confidence::Confirmed,
                "Files are left in /tmp or /var/tmp",
                "Clean them (`virt-sysprep --operations tmp-files`); they are build leftovers and may hold credentials.",
            )
            .detail(format!("{tmp} entr(ies)."))
            .at("/tmp")
            .subject("tmp-files"),
        );
    }

    // Crash dumps and core files.
    let mut crash = Vec::new();
    for dir in ["/var/crash", "/var/lib/systemd/coredump", "/var/spool/abrt"] {
        let (names, _) = root.list_dir(dir);
        if names.iter().any(|n| n != "lost+found") {
            crash.push(dir);
        }
    }
    if let Some(first) = crash.first() {
        report.push(
            Finding::new(
                "IDENT-012",
                CAT,
                Severity::Low,
                Confidence::Confirmed,
                "Crash dumps are left in the image",
                "Remove them (`virt-sysprep --operations crash-data,abrt-data`); dumps contain process memory.",
            )
            .detail(format!("In {}.", crash.join(", ")))
            .at(*first)
            .subject("crash-dumps"),
        );
    }

    // Package manager caches.
    let mut cached = 0usize;
    for dir in [
        "/var/cache/apt/archives",
        "/var/cache/dnf",
        "/var/cache/yum",
        "/var/cache/zypp/packages",
        "/var/cache/pacman/pkg",
    ] {
        cached += root
            .list_dir(dir)
            .0
            .iter()
            .filter(|n| *n != "partial" && *n != "lock")
            .count();
    }
    if cached > 0 {
        report.push(
            Finding::new(
                "IDENT-013",
                CAT,
                Severity::Info,
                Confidence::Confirmed,
                "Package manager caches are populated",
                "Clean them (`apt-get clean`, `dnf clean all`, or `virt-sysprep --operations package-manager-cache`).",
            )
            .detail(format!("{cached} cache entr(ies); they only enlarge the image."))
            .at("/var/cache")
            .subject("package-cache"),
        );
    }

    // Login records and logs with content.
    let mut logs = 0usize;
    for p in ["/var/log/wtmp", "/var/log/btmp", "/var/log/lastlog"] {
        if root.size(p).map(|s| s > 0).unwrap_or(false) {
            logs += 1;
        }
    }
    let (log_names, _) = root.list_dir("/var/log");
    for n in &log_names {
        if let Some(p) = join_guest("/var/log", n) {
            if (n.ends_with(".log") || n == "messages" || n == "syslog" || n == "secure")
                && root.size(&p).map(|s| s > 0).unwrap_or(false)
            {
                logs += 1;
            }
        }
    }
    if logs > 0 {
        report.push(
            Finding::new(
                "IDENT-014",
                CAT,
                Severity::Info,
                Confidence::Confirmed,
                "Log and login-record files with content are in the image",
                "Truncate them (`virt-sysprep --operations logfiles,utmp`); they expose past logins and build activity.",
            )
            .detail(format!("{logs} file(s) with content."))
            .at("/var/log")
            .subject("logs"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::root::fixture::Tree;
    use super::*;

    fn run(t: &Tree) -> AuditReport {
        let mut r = AuditReport::new("test");
        audit_identity(&t.root(), &mut r);
        r.finish();
        r
    }

    fn ids(r: &AuditReport) -> Vec<&str> {
        r.findings.iter().map(|f| f.id.as_str()).collect()
    }

    #[test]
    fn sealed_image_is_clean() {
        let t = Tree::new();
        t.file("/etc/machine-id", "");
        t.file("/etc/os-release", "ID=x\n");
        t.file("/tmp/.keep-nothing", "x");
        let r = run(&t);
        // Only the /tmp file is reported.
        assert_eq!(ids(&r), vec!["IDENT-011"]);
        let t2 = Tree::new();
        t2.file("/etc/machine-id", "uninitialized\n");
        assert!(run(&t2).findings.is_empty());
    }

    #[test]
    fn machine_id_baked_in_is_found_and_value_not_copied() {
        let t = Tree::new();
        let id = "0123456789abcdef0123456789abcdef";
        t.file("/etc/machine-id", &format!("{id}\n"));
        t.link("/var/lib/dbus/machine-id", "/etc/machine-id");
        let r = run(&t);
        assert_eq!(ids(&r), vec!["IDENT-001"]);
        assert!(!serde_json::to_string(&r).unwrap().contains(id));
    }

    #[test]
    fn zero_placeholder_machine_id_is_ok() {
        let t = Tree::new();
        t.file("/etc/machine-id", "00000000000000000000000000000000\n");
        assert!(run(&t).findings.is_empty());
    }

    #[test]
    fn dbus_machine_id_that_differs_is_reported() {
        let t = Tree::new();
        t.file("/etc/machine-id", "");
        t.file(
            "/var/lib/dbus/machine-id",
            "abcdef0123456789abcdef0123456789\n",
        );
        let r = run(&t);
        assert_eq!(ids(&r), vec!["IDENT-001"]);
        assert_eq!(
            r.findings[0].location.as_deref(),
            Some("/var/lib/dbus/machine-id")
        );
    }

    #[test]
    fn history_is_counted_not_copied() {
        let t = Tree::new();
        t.file("/root/.bash_history", "mysql -pSuperSecretPw\nls\n");
        t.file("/home/alice/.zsh_history", "x\n");
        t.file("/home/bob/.bash_history", "");
        let r = run(&t);
        assert_eq!(ids(&r), vec!["IDENT-003", "IDENT-003"]);
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("SuperSecretPw"));
        assert!(json.contains("2 line(s)"));
    }

    #[test]
    fn dhcp_cron_mail_seed_keytab_registration() {
        let t = Tree::new();
        t.file("/var/lib/dhcp/dhclient.eth0.leases", "lease {}\n");
        t.file("/var/lib/dhcp/dhclient.conf", "x\n"); // not a lease
        t.file("/var/spool/cron/crontabs/root", "* * * * * x\n");
        t.file("/var/spool/at/.SEQ", "1\n");
        t.file("/var/mail/root", "From: x\n");
        t.file("/var/lib/systemd/random-seed", "seed");
        t.file("/etc/krb5.keytab", "KEYTAB");
        t.file("/etc/pki/consumer/cert.pem", "CERT");
        let r = run(&t);
        let got = ids(&r);
        for want in [
            "IDENT-002",
            "IDENT-004",
            "IDENT-005",
            "IDENT-007",
            "IDENT-008",
            "IDENT-010",
        ] {
            assert!(got.contains(&want), "missing {want} in {got:?}");
        }
        // .SEQ is not a job, conf is not a lease.
        let cron = r.findings.iter().find(|f| f.id == "IDENT-005").unwrap();
        assert!(cron.detail.starts_with("1 spool"));
        let dhcp = r.findings.iter().find(|f| f.id == "IDENT-002").unwrap();
        assert!(dhcp.detail.starts_with("1 lease"));
        let krb = r.findings.iter().find(|f| f.id == "IDENT-010").unwrap();
        assert_eq!(krb.severity, Severity::High);
        assert!(!serde_json::to_string(&r).unwrap().contains("KEYTAB"));
    }

    #[test]
    fn shadow_backup_is_medium_passwd_backup_is_low() {
        let t = Tree::new();
        t.file("/etc/passwd-", "root:x:0:0\n");
        assert_eq!(run(&t).findings[0].severity, Severity::Low);
        t.file("/etc/shadow-", "root:$6$h:1::::::\n");
        let r = run(&t);
        assert_eq!(r.findings[0].severity, Severity::Medium);
        assert!(!serde_json::to_string(&r).unwrap().contains("$6$h"));
    }

    #[test]
    fn config_management_identity_and_leftovers() {
        let t = Tree::new();
        t.file("/etc/salt/pki/minion/minion.pem", "KEY");
        t.file("/etc/salt/minion_id", "web01");
        t.file("/etc/chef/client.pem", "KEY2");
        t.file("/var/crash/core.1", "x");
        t.file("/var/cache/apt/archives/a.deb", "x");
        t.file("/var/log/wtmp", "x");
        t.file("/var/log/syslog", "x");
        let r = run(&t);
        let salt: Vec<_> = r
            .findings
            .iter()
            .filter(|f| f.subject.as_deref() == Some("salt-minion"))
            .collect();
        assert_eq!(salt.len(), 1, "one finding per node identity");
        for want in ["IDENT-009", "IDENT-012", "IDENT-013", "IDENT-014"] {
            assert!(ids(&r).contains(&want), "missing {want}");
        }
        assert!(!serde_json::to_string(&r).unwrap().contains("KEY2"));
    }

    #[test]
    fn hostile_tree_does_not_escape_or_panic() {
        let t = Tree::new();
        // Escaping and looping links must stay confined to the guest root.
        t.link("/root/.bash_history", "/etc/passwd");
        t.link("/etc/machine-id", "/etc/machine-id");
        t.link("/var/mail", "../../../../..");
        let r = run(&t);
        assert!(r.findings.is_empty(), "{:?}", ids(&r));
    }
}
