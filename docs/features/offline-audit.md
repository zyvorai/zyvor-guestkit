# Offline image audit

`guestkit image-audit` inspects a disk image **without booting it and without
any agent**, using a read-only mount, and reports hardening problems and boot
hazards with a severity and a confidence for every finding.

```
guestkit image-audit disk.qcow2                       # text, all rules
guestkit image-audit disk.qcow2 --scope hardening     # hardening | boot | identity | all
guestkit image-audit /mnt/ro-guest-root               # an already mounted or extracted guest root works too
guestkit image-audit disk.qcow2 -o json > audit.json  # json | yaml | text | sarif
guestkit image-audit disk.qcow2 --fail-on high        # exit 2 if any finding >= high

guestkit image-audit-diff golden.qcow2 candidate.qcow2
guestkit image-audit-diff before.json after.json --fail-on-regression medium -o json
```

`image-audit-diff` accepts disk images or saved `image-audit -o json` reports
(schema `guestkit.offline-audit/1`), so a golden image can be audited once and
compared against later builds in CI. The diff output has schema
`guestkit.offline-audit-diff/1` with `added`, `resolved`, `severity_changed`,
`unchanged` and a `verdict` of `unchanged`, `improved`, `regressed` or `mixed`.
Findings are matched by rule id, file and subject, not by line number.
`--fail-on-regression SEVERITY` exits 2 when a new or worsened finding is at or
above the severity.

## Identity and leftover state (`--scope identity`)

An image that still carries a machine-id, DHCP leases, shell history, a
registration or a node identity makes every clone share it. The rules follow
the operations of `virt-sysprep`, and each remediation names the matching
operation. History, keytabs, keys and mail are counted or named, never copied.

| Id | Rule |
|---|---|
| IDENT-001 | machine-id baked in (empty, `uninitialized` and all-zero are fine; `/var/lib/dbus/machine-id` only when it differs) |
| IDENT-002 | DHCP client lease files with content |
| IDENT-003 | shell and tool history (bash, zsh, python, mysql, psql, sqlite, node, less) for `/root` and `/home/*` |
| IDENT-004 | saved random seed (`/var/lib/systemd/random-seed`) |
| IDENT-005 | user cron and at jobs in the spool |
| IDENT-006 | account file backups (`passwd-`, `shadow-`, `group-`, `gshadow-`, `.bak`, `.old`); Medium when a shadow file is among them |
| IDENT-007 | local mail spool |
| IDENT-008 | subscription or registration identity (`/etc/pki/consumer`, `/etc/pki/entitlement`, RHN `systemid`, `yum/uuid`) |
| IDENT-009 | enrolled config-management identity (Salt minion key, Chef client key, Puppet agent key, Zabbix PSK) |
| IDENT-010 | Kerberos host keytab (High) |
| IDENT-011..014 | leftovers: files in `/tmp` and `/var/tmp`, crash dumps, package caches, logs and login records |

## Output as SARIF

`-o sarif` writes SARIF 2.1.0 for code-scanning dashboards and other SARIF
consumers. Rule ids become SARIF rules, severity maps to `error` (critical,
high), `warning` (medium) and `note` (low, info), and the rule carries a
`security-severity` score. Locations are guest paths relative to the guest root
(`uriBaseId` `GUESTROOT`, with `startLine` when known). Each result has a stable
`partialFingerprints` key (rule, file and subject, not the line). The report's
`notes` become tool notifications. Waived findings are kept as results with a
SARIF `suppressions` entry (`kind: external`, the justification and owner).
Whether a given dashboard honours `suppressions` varies; GitHub code scanning is
reported by several projects to ignore them, which was not confirmed against its
documentation. Filter with `--waivers` if you need the findings gone from the
upload.

## Waivers and baselines

```
guestkit image-audit disk.qcow2 --waivers waivers.yaml
guestkit image-audit disk.qcow2 --baseline golden-audit.json
```

```yaml
waivers:
  - id: SSH-*                       # rule id, * and ? wildcards
    path: /etc/ssh/*                # optional, matched against the file (no line number)
    subject: PermitRootLogin        # optional
    reason: bastion host needs it   # required
    owner: security-team            # optional
    expires: 2027-03-31             # optional, valid through that day
```

A waiver is a documented decision, so `reason` is required and the file is
validated strictly (unknown keys, bad dates and an empty reason are errors).
Waived findings leave `findings` (and the counts and `--fail-on`) but stay in the
report under `waived` with the reason, owner and expiry. An **expired** waiver no
longer suppresses and is listed in `notes`; a waiver that matches nothing is
listed as **stale**, so the file cannot silently rot. `--baseline` takes a saved
`image-audit -o json` report and suppresses findings that were already in it
(matched by rule, file and subject, so a moved line does not make a finding new),
which is the "only fail on new problems" mode for existing images. A baseline that
is not an audit report is an error, not "no baseline".

## Your own rules (`--rules FILE`)

```yaml
rules:
  - id: ORG-001
    title: Corporate login banner is missing
    severity: medium            # info | low | medium | high | critical
    confidence: confirmed       # optional: confirmed (default) | likely | heuristic
    category: policy            # optional, default custom
    kind: line-missing
    path: /etc/issue            # absolute; * and ? allowed in the last component only
    pattern: "Authorized use only"
    remediation: Install the standard banner.
```

Kinds: `file-present` (fails when a matching file exists), `file-missing`,
`line-matches` (fails per matching line), `line-missing` (fails when no line
matches; `missing_ok: true` ignores an absent file) and `max-mode` (fails when
permission bits exceed `max_mode`, for example `0640`). Rules run through the
same confined reader as the built-in rules. A finding gives the line number and
the pattern, never the matching line, so a rule aimed at a credentials file
cannot leak the value. The file is validated before the image is opened: unknown
keys, duplicate ids, relative or `..` paths, wildcards in directory components,
invalid or oversized regexes (linear-time engine, 1 KiB, size-limited) and more
than 500 rules are errors.

## Audit a directory

`image-audit` also accepts an already mounted or extracted guest root directory
instead of a disk image. The same confined reader and rules run; filesystem
UUID/label references cannot be resolved without an image, which is stated in the
report's `notes`. This needs no libguestfs, root or KVM, so it also works in CI
over an unpacked container or rootfs tarball.

## Boundaries

* Read-only: the image is opened and mounted read-only; nothing is written.
* Zero boot, no agent: files are read from the mounted tree only. No guest code
  is executed.
* Hostile images are expected. All reads go through `offline::GuestRoot`:
  symlinks are resolved against the guest root (an absolute link such as
  `/etc/passwd` cannot reach the host), loops fail cleanly, FIFOs, sockets and
  devices are never opened, and caps apply (1 MiB per file, 4096 directory
  entries, include depth 8, 256 files per rule group, YAML documents with many
  aliases are not parsed).
* No secrets in output. Password hashes, host private keys and authorized keys
  are never copied: findings contain rule ids, locations, account names and
  counts.

## Confidence

| Value | Meaning |
|---|---|
| `confirmed` | The exact setting or missing file was read from the image. |
| `likely` | Observed, but the effect depends on distribution defaults or on devices the offline scan cannot see (LVM, LUKS inner filesystems, swap). |
| `heuristic` | Cannot be proven offline (for example an apparently missing kernel when `/boot` may be a separate partition that was not part of the image). |

The report's `notes` list states every limit that applied (oversized or
unreadable files, caps reached, `Match` blocks not evaluated, device references
that could not be verified). A clean report with notes is not "all clear" for
the noted items.

## Hardening rules (`--scope hardening`)

| Id | Rule |
|---|---|
| SSH-001..009, 013, 014 | sshd: root login, password auth, empty passwords, protocol 1, X11, weak ciphers/MACs, MaxAuthTries, host-based auth, IgnoreRhosts, PermitUserEnvironment. `Include` is followed, first value wins, `Match` blocks are noted but not evaluated. |
| SSH-010, 011, 012 | baked-in host keys (names only), loose host key permissions, authorized keys present (counts only) |
| SUDO-001..004 | NOPASSWD (High for ALL commands), `!authenticate`, `!env_reset`, group/world-writable sudoers files; `@includedir` and `#include` followed with sudo's file-name rules |
| PAM-001..004 | `nullok`, `pam_permit` as `sufficient`, weak `pam_unix` hashing, no lockout module |
| LOGIN-001..003 | `ENCRYPT_METHOD` MD5/DES, `PASS_MAX_DAYS`, `UMASK` |
| ACCT-001..006 | extra UID 0, empty passwords, NIS `+` entries, system accounts with login shells, MD5/DES password hashes, `/etc/shadow` and `/etc/passwd` permissions |

## Boot rules (`--scope boot`)

| Id | Rule |
|---|---|
| BOOT-001..007 | no kernel, kernel without initramfs, no `/lib/modules/VERSION`, dangling boot symlinks, empty or truncated kernel/initramfs, orphan initramfs |
| BOOT-010 | BLS entry (`/boot/loader/entries`) references a missing file |
| BOOT-020..023 | GRUB config without entries, missing `linux`/`initrd` target, `search --fs-uuid` for a UUID not in the image, no bootloader config |
| BOOT-030, 036 | `root=` / `resume=` does not resolve to a filesystem in the image |
| BOOT-031..035 | fstab: unresolved UUID/LABEL, duplicate mount points, network mounts without `nofail`/`_netdev`, kernel device names, no root entry |
| BOOT-040..043 | cloud-init config not valid YAML, disabled, per-instance state baked in, bad `datasource_list` |
| BOOT-050..053 | netplan not valid YAML or no `network:`, MAC-pinned network config (netplan, ifcfg `HWADDR`), persistent net udev rules, netplan version |

Filesystem UUIDs and labels come from the image's partitions. PARTUUID and
PARTLABEL, `/dev/disk/by-id`, `/dev/mapper` and LVM names are reported as
unverifiable (counted in the notes) rather than guessed.

## Library and MCP

```rust
use guestkit::offline::{GuestRoot, AuditScope, run_audit, diff_reports};
let root = GuestRoot::new("/mnt/ro-guest-root")?;          // any mounted tree
let report = run_audit(&root, AuditScope::All, &[], "disk.img");
```

`Guestfs::guest_root_dir()` returns the host directory of a read-only guest
mount. `guestkit mcp-serve` (feature `mcp`) exposes the report through the
`get_offline_audit` tool. Everything is additive; no existing public item
changed.

## Tests

The rule sets are tested on synthetic trees built in temporary directories,
including hostile cases: absolute and `..` symlink escapes, symlink loops, FIFOs,
oversized files, directory and include fan-out, include cycles, truncated and
non-UTF-8 account files, YAML alias bombs and path traversal in boot entries.
No image, root, KVM or libguestfs is needed to run them
(`cargo test --lib offline::`).
