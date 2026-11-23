// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Offline boot-readiness checks: kernel/initramfs pairing, BLS and GRUB
//! entries, `root=`/fstab device resolution, cloud-init and network config.
//!
//! Everything is derived from files in the mounted tree plus an optional
//! inventory of the image's filesystems (UUID/label). Where the inventory is
//! missing or cannot see through LVM/LUKS, device references are reported as
//! `Likely` or left unverified (with a note), never as `Confirmed`.

use super::finding::{AuditReport, Confidence, Finding, Severity};
use super::hardening::{read_or_note, wild_match};
use super::root::{join_guest, GuestRoot};
use std::collections::{BTreeMap, BTreeSet};

const CAT: &str = "boot";

/// One filesystem discovered in the image (from the partition scan).
#[derive(Debug, Clone, Default)]
pub struct KnownFilesystem {
    pub device: String,
    pub uuid: Option<String>,
    pub label: Option<String>,
    pub partuuid: Option<String>,
}

pub fn audit_boot_readiness(root: &GuestRoot, known: &[KnownFilesystem], report: &mut AuditReport) {
    if !root.exists("/etc") && !root.exists("/boot") {
        report
            .note("boot checks skipped: no /etc or /boot in the mounted tree (not a Linux root?)");
        return;
    }
    let mut cmdlines: Vec<Cmdline> = Vec::new();
    audit_kernels(root, report);
    audit_bls(root, report, &mut cmdlines);
    audit_grub(root, known, report, &mut cmdlines);
    audit_cmdline_refs(known, report, &cmdlines);
    audit_fstab(root, known, report);
    audit_cloud_init(root, report);
    audit_network(root, report);
}

// ------------------------------------------------------ device resolution

#[derive(Debug, PartialEq, Eq)]
enum Resolution {
    Resolved,
    Unresolved,
    /// Cannot be decided offline (no inventory, LVM/LUKS names, by-id...).
    Unverifiable,
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn resolve_spec(spec: &str, known: &[KnownFilesystem]) -> Resolution {
    let spec = spec.trim().trim_matches('"');
    let (kind, value) = if let Some(v) = spec.strip_prefix("UUID=") {
        ("uuid", v)
    } else if let Some(v) = spec.strip_prefix("LABEL=") {
        ("label", v)
    } else if let Some(v) = spec.strip_prefix("PARTUUID=") {
        ("partuuid", v)
    } else if let Some(v) = spec.strip_prefix("/dev/disk/by-uuid/") {
        ("uuid", v)
    } else if let Some(v) = spec.strip_prefix("/dev/disk/by-label/") {
        ("label", v)
    } else if let Some(v) = spec.strip_prefix("/dev/disk/by-partuuid/") {
        ("partuuid", v)
    } else {
        return Resolution::Unverifiable;
    };
    let value = value.trim_matches('"');
    if value.is_empty() {
        return Resolution::Unresolved;
    }
    if known.is_empty() {
        return Resolution::Unverifiable;
    }
    let hit = match kind {
        "uuid" => known
            .iter()
            .any(|k| k.uuid.as_deref().is_some_and(|u| eq_ci(u, value))),
        "label" => known.iter().any(|k| k.label.as_deref() == Some(value)),
        _ => {
            if known.iter().all(|k| k.partuuid.is_none()) {
                return Resolution::Unverifiable;
            }
            known
                .iter()
                .any(|k| k.partuuid.as_deref().is_some_and(|u| eq_ci(u, value)))
        }
    };
    if hit {
        Resolution::Resolved
    } else {
        Resolution::Unresolved
    }
}

// ---------------------------------------------------- kernels / initramfs

fn is_junk_suffix(n: &str) -> bool {
    [
        ".hmac",
        ".sig",
        ".old",
        ".bak",
        ".dpkg-bak",
        ".dpkg-old",
        ".orig",
        ".rpmsave",
        ".rpmnew",
    ]
    .iter()
    .any(|s| n.ends_with(s))
}

fn audit_kernels(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let (names, trunc) = root.list_dir("/boot");
    if trunc {
        report.note("/boot: directory listing truncated at the fan-out cap");
    }

    let mut kernels: BTreeMap<String, String> = BTreeMap::new(); // version -> file
    let mut initrds: BTreeMap<String, String> = BTreeMap::new();
    for n in &names {
        if is_junk_suffix(n) {
            continue;
        }
        let p = format!("/boot/{n}");
        // Dangling convenience links (vmlinuz, initrd.img, *.old...) break boot
        // entries that reference them.
        if (n.starts_with("vmlinuz") || n.starts_with("initrd") || n.starts_with("initramfs"))
            && root.is_dangling_symlink(&p)
        {
            report.push(
                Finding::new("BOOT-004", CAT, Severity::High, Confidence::Confirmed,
                    "Boot file is a dangling symlink",
                    "Re-create the link to an existing kernel/initramfs (update-initramfs, kernel-install, dracut).")
                    .at(p.clone()).subject(n.clone()),
            );
            continue;
        }
        if let Some(v) = n
            .strip_prefix("vmlinuz-")
            .or_else(|| n.strip_prefix("vmlinux-"))
        {
            kernels.insert(v.to_string(), p);
        } else if let Some(v) = n.strip_prefix("initrd.img-") {
            initrds.insert(v.to_string(), p);
        } else if let Some(v) = n
            .strip_prefix("initramfs-")
            .and_then(|v| v.strip_suffix(".img"))
        {
            if !v.ends_with("kdump") {
                initrds.insert(v.strip_suffix("-fallback").unwrap_or(v).to_string(), p);
            }
        } else if let Some(v) = n.strip_prefix("initrd-") {
            if !v.ends_with("kdump") && !v.ends_with(".img") {
                initrds.insert(v.to_string(), p);
            }
        }
    }

    if kernels.is_empty() {
        let uki = root.is_dir("/boot/efi/EFI/Linux")
            && !root.list_dir("/boot/efi/EFI/Linux").0.is_empty()
            || root.is_dir("/efi/EFI/Linux") && !root.list_dir("/efi/EFI/Linux").0.is_empty();
        if !uki {
            report.push(
                Finding::new("BOOT-001", CAT, Severity::High, Confidence::Heuristic,
                    "No kernel image found under /boot",
                    "Install a kernel, or make sure a separate /boot partition is part of the image and mounted.")
                    .detail("No vmlinuz-*/vmlinux-* files and no unified kernel image (EFI/Linux). A separate, unmounted /boot would look the same.")
                    .at("/boot").subject("kernel"),
            );
        }
    }

    for (ver, kpath) in &kernels {
        let rescue = ver.contains("rescue");
        match root.size(kpath) {
            Some(0) => report.push(
                Finding::new(
                    "BOOT-005",
                    CAT,
                    Severity::High,
                    Confidence::Confirmed,
                    "Kernel image is empty (0 bytes)",
                    "Reinstall the kernel package.",
                )
                .at(kpath.clone())
                .subject(ver.clone()),
            ),
            Some(n) if n < 1024 => report.push(
                Finding::new(
                    "BOOT-005",
                    CAT,
                    Severity::High,
                    Confidence::Likely,
                    "Kernel image is implausibly small (truncated copy?)",
                    "Reinstall the kernel package.",
                )
                .detail(format!("{n} bytes."))
                .at(kpath.clone())
                .subject(ver.clone()),
            ),
            _ => {}
        }
        if rescue {
            continue;
        }
        if !initrds.contains_key(ver) {
            report.push(
                Finding::new("BOOT-002", CAT, Severity::High, Confidence::Likely,
                    "Kernel has no matching initramfs",
                    "Regenerate it (update-initramfs -c -k VERSION, dracut --force, mkinitcpio -P).")
                    .detail("Kernels with every needed driver built in can boot without one; most distribution kernels cannot.")
                    .at(kpath.clone()).subject(ver.clone()),
            );
        }
        // Distribution kernels are versioned like `6.1.0-13-amd64`; Arch uses
        // `linux`. Only version-like names map to /lib/modules/<version>.
        if ver.starts_with(|c: char| c.is_ascii_digit())
            && !root.is_dir(&format!("/lib/modules/{ver}"))
            && !root.is_dir(&format!("/usr/lib/modules/{ver}"))
        {
            report.push(
                Finding::new("BOOT-003", CAT, Severity::High, Confidence::Likely,
                    "Kernel has no /lib/modules directory",
                    "Reinstall the kernel modules package; the initramfs and the booted system need them.")
                    .at(kpath.clone()).subject(ver.clone()),
            );
        }
    }
    for (ver, ipath) in &initrds {
        match root.size(ipath) {
            Some(n) if n < 1024 => report.push(
                Finding::new(
                    "BOOT-006",
                    CAT,
                    Severity::High,
                    if n == 0 {
                        Confidence::Confirmed
                    } else {
                        Confidence::Likely
                    },
                    "Initramfs is empty or truncated",
                    "Regenerate the initramfs.",
                )
                .detail(format!("{n} bytes."))
                .at(ipath.clone())
                .subject(ver.clone()),
            ),
            _ => {}
        }
        if !kernels.contains_key(ver) && !kernels.is_empty() {
            report.push(
                Finding::new(
                    "BOOT-007",
                    CAT,
                    Severity::Low,
                    Confidence::Likely,
                    "Initramfs has no matching kernel (orphan)",
                    "Remove the stale initramfs, or reinstall the kernel it belongs to.",
                )
                .at(ipath.clone())
                .subject(ver.clone()),
            );
        }
    }
}

// ------------------------------------------------------------ BLS / GRUB

/// A kernel command line found in a boot entry.
#[derive(Debug)]
struct Cmdline {
    loc: String,
    text: String,
}

fn boot_file_exists(root: &GuestRoot, path: &str) -> bool {
    let p = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    // Only normal, in-tree paths; keep `..` out of boot-entry lookups.
    if p.split('/').any(|c| c == "..") {
        return false;
    }
    root.exists(&format!("/boot{p}")) || root.exists(&p)
}

fn audit_bls(root: &GuestRoot, report: &mut AuditReport, cmdlines: &mut Vec<Cmdline>) {
    report.checks_run += 1;
    let dir = "/boot/loader/entries";
    if !root.is_dir(dir) {
        return;
    }
    let (names, trunc) = root.list_dir(dir);
    if trunc {
        report.note(format!("{dir}: directory listing truncated"));
    }
    let cap = root.limits().max_files_per_group;
    for (seen, n) in names.iter().filter(|n| n.ends_with(".conf")).enumerate() {
        if seen >= cap {
            report.note(format!("{dir}: more than {cap} entries, rest ignored"));
            break;
        }
        let Some(path) = join_guest(dir, n) else {
            continue;
        };
        let Some(text) = read_or_note(root, report, &path) else {
            continue;
        };
        for (i, raw) in text.lines().enumerate() {
            let l = raw.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let (k, v) = l.split_once(char::is_whitespace).unwrap_or((l, ""));
            let v = v.trim();
            let loc = format!("{path}:{}", i + 1);
            match k {
                "linux" | "initrd" | "efi" | "devicetree" => {
                    for f in v.split_whitespace() {
                        if !boot_file_exists(root, f) {
                            report.push(
                                Finding::new("BOOT-010", CAT, Severity::High, Confidence::Confirmed,
                                    "Boot entry references a file that does not exist",
                                    "Regenerate boot entries (kernel-install, grubby) or restore the file.")
                                    .detail(format!("`{k}` -> {f}"))
                                    .at(loc.clone()).subject(n.clone()),
                            );
                        }
                    }
                }
                "options" => cmdlines.push(Cmdline {
                    loc,
                    text: v.to_string(),
                }),
                _ => {}
            }
        }
    }
}

fn grub_cfg_candidates(root: &GuestRoot) -> Vec<String> {
    let mut v = vec![
        "/boot/grub/grub.cfg".to_string(),
        "/boot/grub2/grub.cfg".to_string(),
    ];
    let (vendors, _) = root.list_dir("/boot/efi/EFI");
    for d in vendors {
        if let Some(p) = join_guest("/boot/efi/EFI", &d) {
            v.push(format!("{p}/grub.cfg"));
        }
    }
    v
}

fn audit_grub(
    root: &GuestRoot,
    known: &[KnownFilesystem],
    report: &mut AuditReport,
    cmdlines: &mut Vec<Cmdline>,
) {
    report.checks_run += 1;
    let mut found_any = false;
    let mut checked: BTreeSet<String> = BTreeSet::new();
    const MAX_REFS: usize = 2048;
    for cfg in grub_cfg_candidates(root) {
        if !root.exists(&cfg) {
            continue;
        }
        found_any = true;
        let Some(text) = read_or_note(root, report, &cfg) else {
            continue;
        };
        let has_entries = text.lines().any(|l| {
            let l = l.trim_start();
            l.starts_with("menuentry ")
                || l.starts_with("blscfg")
                || l.starts_with("configfile")
                || l.starts_with("source ")
                || l.starts_with("chainloader")
        });
        if !has_entries {
            report.push(
                Finding::new(
                    "BOOT-020",
                    CAT,
                    Severity::High,
                    Confidence::Likely,
                    "GRUB configuration contains no boot entries",
                    "Regenerate it (grub-mkconfig -o ..., grub2-mkconfig -o ...).",
                )
                .at(cfg.clone())
                .subject(cfg.rsplit('/').nth(1).unwrap_or("grub").to_string()),
            );
        }
        let mut refs = 0usize;
        for (i, raw) in text.lines().enumerate() {
            let l = raw.trim();
            let (cmd, rest) = l.split_once(char::is_whitespace).unwrap_or((l, ""));
            let loc = format!("{cfg}:{}", i + 1);
            match cmd {
                "linux" | "linuxefi" | "linux16" | "initrd" | "initrdefi" | "initrd16" => {
                    refs += 1;
                    if refs > MAX_REFS {
                        report.note(format!(
                            "{cfg}: more than {MAX_REFS} kernel/initrd lines, rest ignored"
                        ));
                        break;
                    }
                    let mut args = rest.split_whitespace();
                    let is_kernel = cmd.starts_with("linux");
                    let paths: Vec<&str> = if is_kernel {
                        args.next().into_iter().collect()
                    } else {
                        args.collect()
                    };
                    if is_kernel {
                        let cl: Vec<&str> = rest.split_whitespace().skip(1).collect();
                        cmdlines.push(Cmdline {
                            loc: loc.clone(),
                            text: cl.join(" "),
                        });
                    }
                    for p in paths {
                        // `(hd0,gpt2)/vmlinuz` -> `/vmlinuz`; skip variable paths.
                        let p = match p.strip_prefix('(') {
                            Some(r) => r.split_once(')').map(|x| x.1).unwrap_or(""),
                            None => p,
                        };
                        if p.is_empty() || p.contains('$') || !checked.insert(p.to_string()) {
                            continue;
                        }
                        if !boot_file_exists(root, p) {
                            report.push(
                                Finding::new(
                                    "BOOT-021",
                                    CAT,
                                    Severity::High,
                                    Confidence::Confirmed,
                                    "GRUB entry references a file that does not exist",
                                    "Regenerate the GRUB configuration or restore the file.",
                                )
                                .detail(format!("`{cmd}` -> {p}"))
                                .at(loc.clone())
                                .subject(p.to_string()),
                            );
                        }
                    }
                }
                "search" => {
                    // search --no-floppy --fs-uuid --set=root <uuid>
                    if rest.contains("--fs-uuid") {
                        if let Some(u) = rest.split_whitespace().last() {
                            if !u.starts_with('-')
                                && resolve_spec(&format!("UUID={u}"), known)
                                    == Resolution::Unresolved
                            {
                                report.push(
                                    Finding::new("BOOT-022", CAT, Severity::High, Confidence::Likely,
                                        "GRUB searches for a filesystem UUID that is not in the image",
                                        "Regenerate the GRUB configuration after the filesystem UUID changed.")
                                        .detail("LVM/LUKS inner filesystems may not be visible to the offline scan.")
                                        .at(loc).subject(format!("UUID={u}")),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if !found_any
        && !root.is_dir("/boot/loader/entries")
        && root.is_dir("/boot")
        && !root.list_dir("/boot").0.is_empty()
    {
        let uki = root.is_dir("/boot/efi/EFI/Linux");
        let sd =
            root.exists("/boot/efi/EFI/systemd") || root.exists("/boot/efi/loader/loader.conf");
        if !uki && !sd {
            report.push(
                Finding::new("BOOT-023", CAT, Severity::Medium, Confidence::Heuristic,
                    "No bootloader configuration found (GRUB, BLS, systemd-boot)",
                    "Install and configure a bootloader, or confirm the VM boots by direct kernel boot.")
                    .at("/boot").subject("bootloader"),
            );
        }
    }
}

/// Resolve `root=` and `resume=` from every collected command line.
fn audit_cmdline_refs(known: &[KnownFilesystem], report: &mut AuditReport, cmdlines: &[Cmdline]) {
    report.checks_run += 1;
    let mut unverified = 0usize;
    let mut done: BTreeSet<(String, String)> = BTreeSet::new();
    for c in cmdlines {
        for tok in c.text.split_whitespace() {
            let (key, val) = match tok.split_once('=') {
                Some(("root", v)) => ("root", v),
                Some(("resume", v)) => ("resume", v),
                _ => continue,
            };
            if !done.insert((key.to_string(), val.to_string())) {
                continue;
            }
            match resolve_spec(val, known) {
                Resolution::Resolved => {}
                Resolution::Unverifiable => unverified += 1,
                Resolution::Unresolved => {
                    let (id, sev, what) = if key == "root" {
                        (
                            "BOOT-030",
                            Severity::High,
                            "root= does not match any filesystem in the image",
                        )
                    } else {
                        (
                            "BOOT-036",
                            Severity::Medium,
                            "resume= does not match any device in the image",
                        )
                    };
                    report.push(
                        Finding::new(id, CAT, sev, Confidence::Likely, what,
                            "Update the kernel command line (and regenerate the bootloader config) to the current UUID or label.")
                            .detail("Likely, not confirmed: filesystems behind LVM or LUKS are not enumerated by the offline scan.")
                            .at(c.loc.clone()).subject(format!("{key}={val}")),
                    );
                }
            }
        }
    }
    if unverified > 0 {
        report.note(format!(
            "{unverified} kernel command line device reference(s) could not be verified offline (no filesystem inventory, or label/partuuid/by-id/mapper names)"
        ));
    }
}

// ----------------------------------------------------------------- fstab

const NET_FS: &[&str] = &[
    "nfs",
    "nfs4",
    "cifs",
    "smbfs",
    "glusterfs",
    "ceph",
    "davfs",
    "sshfs",
    "fuse.sshfs",
];
const KERNEL_DEV_PREFIXES: &[&str] = &[
    "/dev/sd",
    "/dev/hd",
    "/dev/vd",
    "/dev/xvd",
    "/dev/nvme",
    "/dev/mmcblk",
];

fn audit_fstab(root: &GuestRoot, known: &[KnownFilesystem], report: &mut AuditReport) {
    report.checks_run += 1;
    let path = "/etc/fstab";
    let Some(text) = read_or_note(root, report, path) else {
        return;
    };
    let mut mounts: BTreeMap<String, usize> = BTreeMap::new();
    let mut has_root = false;
    let mut malformed = 0usize;
    let mut unverified = 0usize;
    for (i, raw) in text.lines().enumerate() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() < 2 {
            malformed += 1;
            continue;
        }
        let (dev, mp) = (f[0], f[1]);
        let fstype = f.get(2).copied().unwrap_or("auto");
        let opts = f.get(3).copied().unwrap_or("defaults");
        let optset: BTreeSet<&str> = opts.split(',').collect();
        let loc = format!("{path}:{}", i + 1);
        let is_swap = fstype == "swap";
        if mp == "/" {
            has_root = true;
        }
        if !is_swap && mp != "none" {
            let m = mp.trim_end_matches('/');
            *mounts
                .entry(if m.is_empty() {
                    "/".to_string()
                } else {
                    m.to_string()
                })
                .or_insert(0) += 1;
        }
        let soft = optset.contains("nofail") || optset.contains("noauto");
        match resolve_spec(dev, known) {
            Resolution::Resolved => {}
            Resolution::Unverifiable => unverified += 1,
            Resolution::Unresolved => {
                let (sev, conf) = if is_swap {
                    (Severity::Medium, Confidence::Heuristic)
                } else if soft {
                    (Severity::Low, Confidence::Likely)
                } else {
                    (Severity::High, Confidence::Likely)
                };
                report.push(
                    Finding::new("BOOT-031", CAT, sev, conf,
                        "fstab device does not match any filesystem in the image",
                        "Fix the UUID/LABEL, or add `nofail` for optional mounts. A missing required mount drops systemd to emergency mode.")
                        .detail(if is_swap {
                            "Swap areas are not always enumerated by the offline scan."
                        } else {
                            "Filesystems behind LVM or LUKS are not enumerated by the offline scan."
                        })
                        .at(loc.clone()).subject(dev.to_string()),
                );
            }
        }
        if NET_FS.contains(&fstype)
            && !soft
            && !optset.contains("_netdev")
            && !opts.contains("x-systemd.automount")
        {
            report.push(
                Finding::new("BOOT-033", CAT, Severity::Medium, Confidence::Likely,
                    "Network filesystem in fstab without nofail/_netdev",
                    "Add `nofail,_netdev` (or x-systemd.automount) so an unreachable server cannot stall boot.")
                    .at(loc.clone()).subject(mp.to_string()),
            );
        }
        if KERNEL_DEV_PREFIXES.iter().any(|p| dev.starts_with(p)) {
            report.push(
                Finding::new("BOOT-034", CAT, Severity::Low, Confidence::Heuristic,
                    "fstab uses a kernel device name",
                    "Use UUID= or LABEL=: sdX/vdX/nvmeX names change between hypervisors and controller types.")
                    .at(loc).subject(dev.to_string()),
            );
        }
    }
    for (mp, n) in mounts.iter().filter(|(_, n)| **n > 1) {
        report.push(
            Finding::new(
                "BOOT-032",
                CAT,
                Severity::Medium,
                Confidence::Confirmed,
                "Mount point appears more than once in fstab",
                "Remove the duplicate entry; the later one shadows the earlier.",
            )
            .detail(format!("{n} entries."))
            .at(path)
            .subject(mp.clone()),
        );
    }
    if !has_root {
        report.push(
            Finding::new("BOOT-035", CAT, Severity::Medium, Confidence::Heuristic,
                "fstab has no root (/) entry",
                "Add one, unless root is supplied by the kernel command line (ostree, live, some btrfs layouts).")
                .at(path).subject("root"),
        );
    }
    if malformed > 0 {
        report.note(format!("/etc/fstab: {malformed} malformed line(s) skipped"));
    }
    if unverified > 0 {
        report.note(format!(
            "/etc/fstab: {unverified} device reference(s) could not be verified offline (no filesystem inventory, or label/partuuid/by-id/mapper names)"
        ));
    }
}

// -------------------------------------------------- YAML (cloud-init etc.)

/// Parse YAML defensively. Alias-heavy documents ("billion laughs") are not
/// parsed at all; they are reported via the returned error text.
fn parse_yaml(text: &str) -> Result<serde_yaml::Value, String> {
    // An alias is `*name` at a value or list-item position.
    let aliases = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .map(|l| {
            let t = l.trim_start();
            usize::from(t.starts_with("- *")) + l.matches(": *").count() + l.matches(", *").count()
        })
        .sum::<usize>();
    if aliases > 32 {
        return Err("too many YAML aliases (not parsed)".to_string());
    }
    serde_yaml::from_str::<serde_yaml::Value>(text).map_err(|e| e.to_string())
}

fn yaml_files(root: &GuestRoot, dir: &str, pattern: &str, report: &mut AuditReport) -> Vec<String> {
    let (names, trunc) = root.list_dir(dir);
    if trunc {
        report.note(format!("{dir}: directory listing truncated"));
    }
    names
        .into_iter()
        .filter(|n| wild_match(pattern, n))
        .filter_map(|n| join_guest(dir, &n))
        .take(root.limits().max_files_per_group)
        .collect()
}

fn audit_cloud_init(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    let mut files = Vec::new();
    if root.exists("/etc/cloud/cloud.cfg") {
        files.push("/etc/cloud/cloud.cfg".to_string());
    }
    files.extend(yaml_files(root, "/etc/cloud/cloud.cfg.d", "*.cfg", report));
    if files.is_empty() && !root.exists("/var/lib/cloud") {
        return;
    }
    for f in &files {
        let Some(text) = read_or_note(root, report, f) else {
            continue;
        };
        match parse_yaml(&text) {
            Err(e) => report.push(
                Finding::new("BOOT-040", CAT, Severity::High, Confidence::Confirmed,
                    "cloud-init configuration is not valid YAML",
                    "Fix the syntax; cloud-init ignores or fails on an unparsable file, so user/SSH/network setup will not run.")
                    .detail(e.lines().next().unwrap_or("").to_string())
                    .at(f.clone()).subject(f.rsplit('/').next().unwrap_or(f).to_string()),
            ),
            Ok(v) => {
                if let Some(ds) = v.get("datasource_list") {
                    if !ds.is_sequence() {
                        report.push(
                            Finding::new("BOOT-043", CAT, Severity::Medium, Confidence::Confirmed,
                                "cloud-init `datasource_list` is not a list",
                                "Use a YAML list, for example `datasource_list: [NoCloud, None]`.")
                                .at(f.clone()).subject("datasource_list"),
                        );
                    }
                }
            }
        }
    }
    if root.exists("/etc/cloud/cloud-init.disabled") {
        report.push(
            Finding::new("BOOT-041", CAT, Severity::Low, Confidence::Confirmed,
                "cloud-init is disabled in the image",
                "Remove /etc/cloud/cloud-init.disabled if the target platform injects users, keys or network config through cloud-init.")
                .at("/etc/cloud/cloud-init.disabled").subject("disabled"),
        );
    }
    let (inst, _) = root.list_dir("/var/lib/cloud/instances");
    if !inst.is_empty() {
        report.push(
            Finding::new("BOOT-042", CAT, Severity::Low, Confidence::Likely,
                "cloud-init per-instance state is baked into the image",
                "Run `cloud-init clean --logs` before capturing the image, otherwise per-instance modules may not re-run on clones.")
                .detail(format!("{} instance director(ies).", inst.len()))
                .at("/var/lib/cloud/instances").subject("instance-state"),
        );
    }
}

// --------------------------------------------------------------- network

fn netplan_mac_pins(v: &serde_yaml::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(net) = v.get("network").and_then(|n| n.as_mapping()) {
        for (sect, devs) in net {
            let (Some(sect), Some(devs)) = (sect.as_str(), devs.as_mapping()) else {
                continue;
            };
            if !matches!(sect, "ethernets" | "wifis" | "bonds" | "bridges" | "vlans") {
                continue;
            }
            for (name, cfg) in devs {
                if cfg.get("match").and_then(|m| m.get("macaddress")).is_some()
                    || cfg.get("macaddress").is_some()
                {
                    out.push(name.as_str().unwrap_or("?").to_string());
                }
            }
        }
    }
    out
}

fn audit_network(root: &GuestRoot, report: &mut AuditReport) {
    report.checks_run += 1;
    for f in yaml_files(root, "/etc/netplan", "*.yaml", report) {
        let Some(text) = read_or_note(root, report, &f) else {
            continue;
        };
        let name = f.rsplit('/').next().unwrap_or(&f).to_string();
        match parse_yaml(&text) {
            Err(e) => report.push(
                Finding::new("BOOT-050", CAT, Severity::High, Confidence::Confirmed,
                    "netplan configuration is not valid YAML",
                    "Fix the syntax; netplan will not apply it and the VM may boot without networking.")
                    .detail(e.lines().next().unwrap_or("").to_string())
                    .at(f).subject(name),
            ),
            Ok(v) => {
                match v.get("network") {
                    None => report.push(
                        Finding::new("BOOT-050", CAT, Severity::High, Confidence::Confirmed,
                            "netplan file has no top-level `network:` key",
                            "Add the `network:` mapping or delete the file.")
                            .at(f.clone()).subject(name.clone()),
                    ),
                    Some(n) => {
                        if n.get("version").and_then(|x| x.as_u64()) != Some(2) {
                            report.push(
                                Finding::new("BOOT-053", CAT, Severity::Medium, Confidence::Confirmed,
                                    "netplan `network.version` is missing or not 2",
                                    "Set `version: 2`.")
                                    .at(f.clone()).subject(name.clone()),
                            );
                        }
                    }
                }
                let pins = netplan_mac_pins(&v);
                if !pins.is_empty() {
                    report.push(
                        Finding::new("BOOT-051", CAT, Severity::Medium, Confidence::Likely,
                            "Network config is pinned to a MAC address",
                            "A new hypervisor or clone gets a different MAC and the interface will not be configured. Match by name, or let cloud-init regenerate it.")
                            .detail(format!("Interfaces: {}.", pins.join(", ")))
                            .at(f).subject(name),
                    );
                }
            }
        }
    }
    let (scripts, _) = root.list_dir("/etc/sysconfig/network-scripts");
    for n in scripts
        .iter()
        .filter(|n| n.starts_with("ifcfg-"))
        .take(root.limits().max_files_per_group)
    {
        let Some(p) = join_guest("/etc/sysconfig/network-scripts", n) else {
            continue;
        };
        let Some(t) = read_or_note(root, report, &p) else {
            continue;
        };
        if t.lines().any(|l| l.trim_start().starts_with("HWADDR=")) {
            report.push(
                Finding::new(
                    "BOOT-051",
                    CAT,
                    Severity::Medium,
                    Confidence::Likely,
                    "Network config is pinned to a MAC address (HWADDR)",
                    "Remove HWADDR (or switch to NAME/DEVICE matching) before moving the image.",
                )
                .at(p)
                .subject(n.clone()),
            );
        }
    }
    if root.exists("/etc/udev/rules.d/70-persistent-net.rules") {
        report.push(
            Finding::new("BOOT-052", CAT, Severity::Medium, Confidence::Likely,
                "Persistent NIC naming rules are baked into the image",
                "Delete /etc/udev/rules.d/70-persistent-net.rules so NICs are named for the new hardware.")
                .at("/etc/udev/rules.d/70-persistent-net.rules").subject("persistent-net"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::root::fixture::Tree;
    use super::super::root::Limits;
    use super::*;

    fn big() -> String {
        "k".repeat(4096)
    }
    fn fs(uuid: &str, label: Option<&str>) -> KnownFilesystem {
        KnownFilesystem {
            device: "/dev/sda1".into(),
            uuid: Some(uuid.into()),
            label: label.map(String::from),
            partuuid: None,
        }
    }
    fn run(t: &Tree, known: &[KnownFilesystem]) -> AuditReport {
        let mut r = AuditReport::new("t");
        audit_boot_readiness(&t.root(), known, &mut r);
        r.finish();
        r
    }
    fn has(r: &AuditReport, id: &str, subject: &str) -> bool {
        r.findings
            .iter()
            .any(|f| f.id == id && f.subject.as_deref() == Some(subject))
    }
    fn any(r: &AuditReport, id: &str) -> bool {
        r.findings.iter().any(|f| f.id == id)
    }

    #[test]
    fn healthy_debian_layout_has_no_findings() {
        let t = Tree::new();
        t.file("/etc/os-release", "ID=debian");
        t.file("/boot/vmlinuz-6.1.0-13-amd64", &big());
        t.file("/boot/initrd.img-6.1.0-13-amd64", &big());
        t.file("/lib/modules/6.1.0-13-amd64/modules.dep", "");
        t.link("/boot/vmlinuz", "vmlinuz-6.1.0-13-amd64");
        t.link("/boot/initrd.img", "initrd.img-6.1.0-13-amd64");
        t.file("/boot/grub/grub.cfg", "menuentry 'Debian' {\n linux /vmlinuz-6.1.0-13-amd64 root=UUID=AAAA-1 ro quiet\n initrd /initrd.img-6.1.0-13-amd64\n}\n");
        t.file("/etc/fstab", "UUID=AAAA-1 / ext4 defaults 0 1\n");
        let r = run(&t, &[fs("aaaa-1", None)]);
        assert!(r.findings.is_empty(), "{:#?}", r.findings);
    }

    #[test]
    fn kernel_without_initramfs_or_modules() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/boot/vmlinuz-5.14.0-1.el9", &big());
        t.file("/boot/initramfs-5.10.0-old.el9.img", &big());
        let r = run(&t, &[]);
        assert!(has(&r, "BOOT-002", "5.14.0-1.el9"));
        assert!(has(&r, "BOOT-003", "5.14.0-1.el9"));
        assert!(has(&r, "BOOT-007", "5.10.0-old.el9"));
    }

    #[test]
    fn arch_fallback_and_rescue_naming() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/boot/vmlinuz-linux", &big());
        t.file("/boot/initramfs-linux.img", &big());
        t.file("/boot/initramfs-linux-fallback.img", &big());
        t.file("/boot/vmlinuz-0-rescue-abc", &big());
        let r = run(&t, &[]);
        assert!(!any(&r, "BOOT-002"), "{:#?}", r.findings);
        assert!(!any(&r, "BOOT-003"));
    }

    #[test]
    fn truncated_boot_files_and_dangling_links() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/boot/vmlinuz-6.1.0-1", "");
        t.file("/boot/initrd.img-6.1.0-1", "tiny");
        t.link("/boot/vmlinuz", "vmlinuz-9.9.9");
        let r = run(&t, &[]);
        let k = r.findings.iter().find(|f| f.id == "BOOT-005").unwrap();
        assert_eq!(k.confidence, Confidence::Confirmed);
        let i = r.findings.iter().find(|f| f.id == "BOOT-006").unwrap();
        assert_eq!(i.confidence, Confidence::Likely);
        assert!(has(&r, "BOOT-004", "vmlinuz"));
    }

    #[test]
    fn missing_kernel_is_heuristic_because_boot_may_be_unmounted() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        let r = run(&t, &[]);
        let f = r.findings.iter().find(|f| f.id == "BOOT-001").unwrap();
        assert_eq!(f.confidence, Confidence::Heuristic);
    }

    #[test]
    fn uki_counts_as_a_kernel() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/boot/efi/EFI/Linux/fedora.efi", &big());
        assert!(!any(&run(&t, &[]), "BOOT-001"));
    }

    #[test]
    fn bls_entry_references_are_checked() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/boot/vmlinuz-5.14.0-1", &big());
        t.file("/boot/initramfs-5.14.0-1.img", &big());
        t.file("/lib/modules/5.14.0-1/a", "");
        t.file("/boot/loader/entries/a.conf", "title A\nlinux /vmlinuz-5.14.0-1\ninitrd /initramfs-5.14.0-1.img /initramfs-gone.img\noptions root=UUID=dead-beef ro\n");
        let r = run(&t, &[fs("1111", None)]);
        assert!(r
            .findings
            .iter()
            .any(|f| f.id == "BOOT-010" && f.detail.contains("initramfs-gone")));
        assert!(!r
            .findings
            .iter()
            .any(|f| f.id == "BOOT-010" && f.detail.contains("vmlinuz")));
        assert!(has(&r, "BOOT-030", "root=UUID=dead-beef"));
    }

    #[test]
    fn grub_missing_file_and_empty_config() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/boot/vmlinuz-6.0.0-1", &big());
        t.file("/boot/initrd.img-6.0.0-1", &big());
        t.file("/lib/modules/6.0.0-1/a", "");
        t.file("/boot/grub/grub.cfg", "menuentry 'a' {\n linux (hd0,gpt2)/vmlinuz-6.0.0-1 root=/dev/vda1\n initrd /initrd.img-gone $x\n}\n");
        t.file("/boot/grub2/grub.cfg", "# nothing\n");
        let r = run(&t, &[]);
        assert!(has(&r, "BOOT-021", "/initrd.img-gone"));
        assert!(!has(&r, "BOOT-021", "/vmlinuz-6.0.0-1"));
        assert!(any(&r, "BOOT-020"));
    }

    #[test]
    fn grub_search_uuid_resolution() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/boot/efi/EFI/ubuntu/grub.cfg", "search.fs_uuid 1 root\nsearch --no-floppy --fs-uuid --set=root ABCD-0001\nconfigfile $prefix/grub.cfg\n");
        let r = run(&t, &[fs("1111-2222", None)]);
        assert!(has(&r, "BOOT-022", "UUID=ABCD-0001"));
        assert!(!any(&r, "BOOT-020"));
    }

    #[test]
    fn fstab_resolution_and_hazards() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file(
            "/etc/fstab",
            "\
UUID=good / ext4 defaults 0 1
UUID=gone /data ext4 defaults 0 2
LABEL=opt /opt ext4 nofail 0 2
UUID=swapgone none swap sw 0 0
/dev/vdb1 /mnt ext4 defaults 0 0
nas:/export /srv nfs defaults 0 0
nas2:/x /srv2 nfs defaults,nofail,_netdev 0 0
UUID=good /data ext4 defaults 0 2
onlyonefield
",
        );
        let r = run(&t, &[fs("GOOD", Some("other"))]);
        let gone = r
            .findings
            .iter()
            .find(|f| f.id == "BOOT-031" && f.subject.as_deref() == Some("UUID=gone"))
            .unwrap();
        assert_eq!(
            (gone.severity, gone.confidence),
            (Severity::High, Confidence::Likely)
        );
        let opt = r
            .findings
            .iter()
            .find(|f| f.id == "BOOT-031" && f.subject.as_deref() == Some("LABEL=opt"))
            .unwrap();
        assert_eq!(opt.severity, Severity::Low);
        let sw = r
            .findings
            .iter()
            .find(|f| f.id == "BOOT-031" && f.subject.as_deref() == Some("UUID=swapgone"))
            .unwrap();
        assert_eq!(sw.confidence, Confidence::Heuristic);
        assert!(has(&r, "BOOT-033", "/srv"));
        assert!(!has(&r, "BOOT-033", "/srv2"));
        assert!(has(&r, "BOOT-034", "/dev/vdb1"));
        assert!(has(&r, "BOOT-032", "/data"));
        assert!(r.notes.iter().any(|n| n.contains("malformed")));
    }

    #[test]
    fn without_inventory_references_are_unverifiable_not_findings() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/etc/fstab", "UUID=whatever / ext4 defaults 0 1\n");
        let r = run(&t, &[]);
        assert!(!any(&r, "BOOT-031"));
        assert!(r.notes.iter().any(|n| n.contains("could not be verified")));
    }

    #[test]
    fn fstab_missing_root_entry() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/etc/fstab", "tmpfs /tmp tmpfs defaults 0 0\n");
        assert!(has(&run(&t, &[]), "BOOT-035", "root"));
    }

    #[test]
    fn cloud_init_yaml_validation() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file(
            "/etc/cloud/cloud.cfg",
            "users:\n  - default\ndatasource_list: NoCloud\n",
        );
        t.file(
            "/etc/cloud/cloud.cfg.d/99-bad.cfg",
            "key: [unclosed\n  - : :\n",
        );
        t.file("/etc/cloud/cloud-init.disabled", "");
        t.file("/var/lib/cloud/instances/i-123/x", "");
        let r = run(&t, &[]);
        assert!(has(&r, "BOOT-043", "datasource_list"));
        assert!(has(&r, "BOOT-040", "99-bad.cfg"));
        assert!(any(&r, "BOOT-041"));
        assert!(any(&r, "BOOT-042"));
    }

    #[test]
    fn yaml_alias_bomb_is_not_expanded() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        let mut y = String::from("a: &a [x, x, x, x]\n");
        for i in 0..60 {
            y.push_str(&format!("k{i}: *a\n"));
        }
        t.file("/etc/cloud/cloud.cfg", &y);
        let r = run(&t, &[]);
        assert!(r
            .findings
            .iter()
            .any(|f| f.id == "BOOT-040" && f.detail.contains("aliases")));
    }

    #[test]
    fn netplan_checks() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file("/etc/netplan/50-cloud-init.yaml", "network:\n  version: 2\n  ethernets:\n    eth0:\n      match:\n        macaddress: 52:54:00:aa:bb:cc\n      set-name: eth0\n");
        t.file("/etc/netplan/10-bad.yaml", "foo: bar\n");
        t.file("/etc/netplan/20-ver.yaml", "network:\n  ethernets: {}\n");
        t.file("/etc/netplan/30-broken.yaml", "network: [\n");
        t.file("/etc/udev/rules.d/70-persistent-net.rules", "");
        t.file(
            "/etc/sysconfig/network-scripts/ifcfg-eth0",
            "DEVICE=eth0\nHWADDR=52:54:00:aa:bb:cc\n",
        );
        let r = run(&t, &[]);
        assert!(has(&r, "BOOT-051", "50-cloud-init.yaml"));
        assert!(has(&r, "BOOT-051", "ifcfg-eth0"));
        assert!(has(&r, "BOOT-050", "10-bad.yaml"));
        assert!(has(&r, "BOOT-053", "20-ver.yaml"));
        assert!(has(&r, "BOOT-050", "30-broken.yaml"));
        assert!(any(&r, "BOOT-052"));
    }

    #[test]
    fn hostile_boot_entries_cannot_escape_the_guest() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.file(
            "/boot/loader/entries/a.conf",
            "linux /../../../../etc/passwd\ninitrd /../x\n",
        );
        let r = run(&t, &[]);
        assert_eq!(r.findings.iter().filter(|f| f.id == "BOOT-010").count(), 2);
    }

    #[test]
    fn boot_dir_fanout_is_capped_with_note() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        for i in 0..30 {
            t.file(&format!("/boot/junk-{i}"), "");
        }
        let mut r = AuditReport::new("t");
        let root = t.root_with(Limits {
            max_dir_entries: 8,
            ..Limits::default()
        });
        audit_boot_readiness(&root, &[], &mut r);
        assert!(r.notes.iter().any(|n| n.contains("truncated")));
    }

    #[test]
    fn grub_cfg_symlink_loop_and_oversize_do_not_hang() {
        let t = Tree::new();
        t.file("/etc/os-release", "x");
        t.link("/boot/grub/grub.cfg", "/boot/grub/grub.cfg");
        t.bytes("/boot/grub2/grub.cfg", &vec![b'#'; 4096]);
        let mut r = AuditReport::new("t");
        let root = t.root_with(Limits {
            max_file_bytes: 1024,
            ..Limits::default()
        });
        audit_boot_readiness(&root, &[], &mut r);
        assert!(r.notes.iter().any(|n| n.contains("read cap")));
    }

    #[test]
    fn non_linux_tree_is_skipped() {
        let t = Tree::new();
        t.file("/Windows/System32/x", "");
        let r = run(&t, &[]);
        assert!(r.findings.is_empty());
        assert!(r.notes.iter().any(|n| n.contains("skipped")));
    }

    #[test]
    fn spec_resolution_forms() {
        let k = [KnownFilesystem {
            device: "d".into(),
            uuid: Some("AB-12".into()),
            label: Some("data".into()),
            partuuid: Some("p-1".into()),
        }];
        assert_eq!(resolve_spec("UUID=ab-12", &k), Resolution::Resolved);
        assert_eq!(
            resolve_spec("/dev/disk/by-label/data", &k),
            Resolution::Resolved
        );
        assert_eq!(resolve_spec("PARTUUID=P-1", &k), Resolution::Resolved);
        assert_eq!(resolve_spec("PARTUUID=zz", &k), Resolution::Unresolved);
        assert_eq!(
            resolve_spec("/dev/mapper/vg-root", &k),
            Resolution::Unverifiable
        );
        assert_eq!(resolve_spec("UUID=", &k), Resolution::Unresolved);
        assert_eq!(resolve_spec("UUID=x", &[]), Resolution::Unverifiable);
    }
}
