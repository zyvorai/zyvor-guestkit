# Offline Patch & Fix Preview Mode

**Status:** Shipped (CLI generate / preview / apply / export)
**Version:** 0.3.19+
**Last Updated:** 2026-08-07

## Overview

The Offline Patch & Fix Preview Mode enables safe, reviewable VM fixes with complete separation of concerns. Instead of directly applying changes, GuestKit generates detailed fix plans that can be previewed, reviewed, exported as scripts, and applied with safety checks.

Part of [GuestKit on zyvor.dev](https://zyvor.dev/guestkit). Pairs with [migration assurance](migration-assurance.md) (`doctor`, `migrate-plan --export`) and Linux [`rescue`](#rescue-shortcuts) day-0 ops.

## Workflow

```
Inspect → Diagnose → Generate Plan → Review → Approve → Execute
```

**Boot repair path:** `guestkit doctor` → `guestkit repair --fix boot` generates a `boot-repair` plan from blockers/warnings, applies it, then re-runs doctor for a before/after score. See [migration-assurance.md](migration-assurance.md).

This workflow matches enterprise change management requirements and provides:
- **Safety**: See exactly what will change before applying
- **Auditability**: Plans are version-controllable artifacts
- **Scriptability**: Export plans as bash/ansible for review
- **Reversibility**: Backup and rollback capabilities
- **Collaboration**: Security team generates, ops team applies

## Generate-only day-0 profiles

These profiles skip inspect→finding heuristics and emit a fixed offline-safe plan. Prefer `plan apply --skip-backup` for registry/file-only edits.

| Profile | Flags | What it does |
|---------|-------|--------------|
| `windows-rdp` | — | Terminal Server allow, NLA, TermService/UmRdpService Automatic, port 3389, firewall TCP/UDP |
| `windows-hostname` | `--hostname NAME` | ComputerName + ActiveComputerName + Tcpip Hostname / NV Hostname |
| `windows-winrm` | — | WinRM Automatic + `WINRM-HTTP-In-TCP` (review auth before exposing) |
| `windows-domain-leave` | `--workgroup NAME` (default `WORKGROUP`) | Clear Tcpip Domain, set NV Domain + Winlogon to workgroup, stage RunOnce `Add-Computer` (DC account delete still needs live AD) |
| `windows-timezone` | `--timezone KEY` | `TimeZoneKeyName` (e.g. `UTC`, `Pacific Standard Time`) |
| `windows-static-ip` | `--interface-guid` `--ip` `--mask` [`--gateway`] [`--dns`] | Disable DHCP + MULTI_SZ IP/mask/gateway on that interface |
| `windows-dhcp` | `--interface-guid` | Enable DHCP (`EnableDHCP=1`) on that interface |
| `windows-dns` | `--interface-guid` `--dns` | Set `NameServer` (space/comma-separated) on that interface |
| `linux-hostname` | `--hostname NAME` | `/etc/hostname` + `/etc/hosts` patch |
| `linux-grub` | `--grub-timeout N`, `--grub-cmdline TOKEN` | Offline `/etc/default/grub`; pair with `rescue -o fix-grub` for mkconfig/install |
| `linux-ssh` | optional `--user` + `--key` / `--key-file` | Remove `sshd_not_to_be_run`, wants `Symlink`, sshd drop-in, optional `authorized_keys` |

```bash
guestkit plan generate win.qcow2 -p windows-rdp -o rdp.yaml
guestkit plan apply rdp.yaml --vm win.qcow2 --yes --skip-backup

guestkit plan generate win.qcow2 -p windows-hostname --hostname WIN-APP01 -o host.yaml
guestkit plan generate win.qcow2 -p windows-winrm -o winrm.yaml

guestkit plan generate win.qcow2 -p windows-domain-leave --workgroup CORPWG -o leave.yaml
guestkit plan generate win.qcow2 -p windows-timezone --timezone "UTC" -o tz.yaml
guestkit plan generate win.qcow2 -p windows-static-ip \
  --interface-guid a1b2c3d4-e5f6-7890-abcd-ef1234567890 \
  --ip 10.0.0.50 --mask 255.255.255.0 --gateway 10.0.0.1 \
  --dns "1.1.1.1 8.8.8.8" -o ip.yaml
guestkit plan generate win.qcow2 -p windows-dhcp \
  --interface-guid a1b2c3d4-e5f6-7890-abcd-ef1234567890 -o dhcp.yaml
guestkit plan generate win.qcow2 -p windows-dns \
  --interface-guid a1b2c3d4-e5f6-7890-abcd-ef1234567890 --dns "1.1.1.1 8.8.8.8" -o dns.yaml
guestkit plan generate linux.qcow2 -p linux-hostname --hostname web01 -o lhost.yaml
guestkit plan generate linux.qcow2 -p linux-grub --grub-timeout 5 \
  --grub-cmdline nomodeset -o grub.yaml

guestkit plan generate linux.qcow2 -p linux-ssh \
  --user ubuntu --key-file ~/.ssh/id_ed25519.pub -o ssh.yaml
guestkit plan apply ssh.yaml --vm linux.qcow2 --yes --skip-backup
```

Inspect profiles (`security`, `migration`, `compliance`, `hardening`, `windows-migration`, …) still feed the heuristic generator. `PackageInstall` stages `.rpm`/`.deb` from `GUESTKIT_PACKAGE_CACHE` (or `host_cache`) for a first-boot oneshot when files match; with `GUESTKIT_PACKAGE_FETCH=1`, missing packages are downloaded on the host before staging. `ServiceOperation` enable/disable applies via systemd wants Symlink/FileDelete; start/restart and other `CommandExec` lines try chroot then stage `guestkit-firstboot-live.service`. Heuristics prefer offline-safe ops where possible (firewalld/ufw/SSH/SELinux; `systemctl enable/disable` → Symlink/FileDelete; fail2ban/auditd/chrony/apparmor enable).

## VirtIO driver inject (offline)

Migration repair and plan apply can inject Windows VirtIO drivers when the host has a virtio-win tree:

```bash
export GUESTKIT_VIRTIO_WIN=/mnt/virtio-win   # extracted ISO or layout
guestkit migrate-repair win.qcow2 --target kvm --apply
# equivalent: --virtio-win /mnt/virtio-win
```

Requires a build with `registry-write,agent`. See [migration-assurance.md](migration-assurance.md).

## Rescue shortcuts

Offline rescue (`guestkit rescue -o …`) for day-0 jobs without writing a plan file first. Use `--export-plan` to review, then `plan apply`.

| Operation | Flags | Notes |
|-----------|-------|-------|
| `enable-ssh` | `--force` (also PermitRootLogin) | Linux |
| `inject-ssh-key` | `--user`, `--key` / `--key-file` | Linux |
| `set-hostname` | `--hostname` | Linux `/etc/hostname`; Windows registry via day-0 plan |
| `enable-rdp` | — | Windows Terminal Server + firewall (`registry-write`) |
| `enable-winrm` | — | Windows WinRM Automatic + firewall |
| `set-timezone` | `--timezone` | Windows `TimeZoneKeyName` |
| `reset-password` | `--user`, `--password` | Linux `/etc/shadow`; Windows AES/RC4 SAM NT-hash via SYSKEY (needs SYSTEM hive + `registry-write`), else SAM blank + RunOnce `net user`. Omit `--password` on Windows to blank only. |
| `fix-fstab` | `--backup` | Linux |
| `check-grub` | — | Diagnose-only |
| `fix-grub` | `--force` (BIOS `grub-install` on NBD, or EFI `--target=*-efi --efi-directory --no-nvram --removable`) | Chroot `grub-mkconfig`/`update-grub`; first-boot oneshot fallback; `--export-plan` stages first-boot unit |

```bash
guestkit rescue disk.qcow2 -o enable-ssh --export-plan ssh.yaml
guestkit rescue disk.qcow2 -o set-hostname --hostname web01 --export-plan host.yaml
guestkit rescue disk.qcow2 -o fix-grub
guestkit rescue disk.qcow2 -o fix-grub --force          # also try grub-install
guestkit rescue disk.qcow2 -o fix-grub --export-plan grub-fb.yaml

# Windows password (AES SAM when SYSTEM available; build with --features registry-write)
guestkit rescue win.qcow2 -o reset-password --user Administrator --password 'S3cret!'
guestkit rescue win.qcow2 -o reset-password --user Administrator   # blank only

guestkit plan apply ssh.yaml --vm disk.qcow2 --yes --skip-backup
```

### PackageInstall offline staging / host fetch

| Env | Effect |
|-----|--------|
| `GUESTKIT_PACKAGE_CACHE` | Colon-separated host dirs of `.rpm`/`.deb` (also `PackageInstall.host_cache`) |
| `GUESTKIT_PACKAGE_FETCH=1` | Download missing packages on the host (`dnf download` / `yumdownloader` / `apt-get download`) into the cache or `~/.cache/guestkit/packages`, then stage first-boot oneshot |
| `GUESTKIT_PACKAGE_MIRROR` | Comma-separated HTTP bases (optional `{name}`/`{ext}`) tried via `curl`/`wget` when host package tools are missing or fail |

```bash
export GUESTKIT_PACKAGE_CACHE=~/pkg-cache
# optional: fetch anything not already in the cache
export GUESTKIT_PACKAGE_FETCH=1
guestkit plan apply hardening.yaml --vm linux.qcow2 --yes
```

## Architecture

### Core Components

#### 1. **Plan Types** (`types.rs`)

Complete data structures for representing fix plans:

```rust
pub struct FixPlan {
    pub version: String,
    pub vm: String,
    pub generated: DateTime<Utc>,
    pub profile: String,
    pub overall_risk: String,
    pub estimated_duration: String,
    pub metadata: PlanMetadata,
    pub operations: Vec<Operation>,
    pub post_apply: Vec<PostApplyAction>,
}
```

**Operation Types:**
- `FileEdit` - Line-by-line file modifications
- `FileWrite` - Create/overwrite a whole file (offline-friendly)
- `FileDelete` - Remove a guest file (`missing_ok` supported)
- `Symlink` - Force symlink via guestfs `ln_sf` (offline-friendly)
- `PackageInstall` - Live install, or offline stage from `GUESTKIT_PACKAGE_CACHE` / `host_cache` (first-boot oneshot); optional host fetch via `GUESTKIT_PACKAGE_FETCH=1`
- `ServiceOperation` - Offline enable/disable via wants Symlink/FileDelete; start/restart staged to first-boot oneshot
- `SELinuxMode` - SELinux mode changes
- `RegistryEdit` - Windows registry modifications (`registry-write` feature)
- `CommandExec` - Try chroot; else stage to `guestkit-firstboot-live.service` (systemctl enable/disable rewritten offline)
- `FileCopy` - File copy operations
- `DirectoryCreate` - Directory creation
- `FilePermissions` - Permission/ownership changes
- `DriverInject` - Windows driver inject (`host_dir` / `GUESTKIT_VIRTIO_WIN`; needs `registry-write,agent`)

**Priority Levels:**
- Critical
- High
- Medium
- Low
- Info

#### 2. **Plan Generator** (`generator.rs`)

Converts security profile findings into executable fix plans, plus canned day-0 builders (`windows_rdp_enable_plan`, `linux_ssh_enable_plan`, `windows_hostname_plan`, `windows_winrm_enable_plan`).

```rust
let generator = PlanGenerator::new("vm.qcow2".to_string());
let plan = generator.from_security_profile(&security_report)?;
```

**Features:**
- Heuristic-based remediation parsing
- Automatic dependency detection
- Duration estimation
- Post-apply action generation
- Risk level mapping

#### 3. **Plan Preview** (`preview.rs`)

Human-readable plan display with colors and formatting:

```rust
PlanPreview::display(&plan);        // Formatted output
PlanPreview::display_diff(&plan);   // Unified diff view
PlanPreview::print_summary(&plan);  // Summary statistics
```

#### 4. **Plan Applicator** (`apply.rs`)

Executes fix plans offline via guestfs with backup / `--skip-backup`, dry-run, and rollback.

#### 5. **Plan Exporter** (`export.rs`)

Export plans to bash, Ansible, JSON, or YAML.

## Usage Examples

### CLI

```bash
# Generate from an inspect profile
guestkit plan generate vm.qcow2 -p security -o security-fixes.yaml

# Day-0 canned profiles (see table above)
guestkit plan generate vm.qcow2 -p linux-ssh -o ssh.yaml
guestkit plan generate vm.qcow2 -p windows-rdp -o rdp.yaml

# Preview / validate / export
guestkit plan preview security-fixes.yaml
guestkit plan preview security-fixes.yaml --diff
guestkit plan validate security-fixes.yaml
guestkit plan export security-fixes.yaml -o fixes.sh --format bash

# Apply (default takes a full-image backup first)
guestkit plan apply security-fixes.yaml --vm vm.qcow2 --yes
guestkit plan apply rdp.yaml --vm win.qcow2 --yes --skip-backup

# Rollback
guestkit plan rollback /path/to/backup --vm vm.qcow2
```

### Programmatic Usage

```rust
use guestkit::cli::plan::*;

let generator = PlanGenerator::new("vm.qcow2".to_string());
let plan = generator.from_security_profile(&security_report)?;

PlanPreview::display(&plan);

let script = PlanExporter::to_bash(&plan)?;
std::fs::write("fixes.sh", script)?;

let applicator = PlanApplicator::new("vm.qcow2".to_string(), false);
let validation = applicator.validate(&plan)?;

if validation.valid {
    let applicator_dry = PlanApplicator::new("vm.qcow2".to_string(), true);
    let result = applicator_dry.apply(&plan)?;
}
```

## Plan Format

### YAML Example

```yaml
version: "1.0"
vm: production-web-01.qcow2
generated: "2026-01-27T14:30:00Z"
profile: security
overall_risk: high
estimated_duration: "5-10 minutes"

metadata:
  author: guestkit-profiles
  review_required: true
  reversible: true
  description: Security hardening plan
  tags:
    - security
    - automated

operations:
  - id: sec-001
    type: file_edit
    priority: critical
    description: Disable root SSH login
    risk: low
    reversible: true
    file: /etc/ssh/sshd_config
    backup: true
    changes:
      - line: 28
        before: "PermitRootLogin yes"
        after: "PermitRootLogin no"
        context: |
          # Authentication:
          LoginGraceTime 2m
          PermitRootLogin yes  # CHANGE THIS
          StrictModes yes
    validation:
      command: "sshd -t"
      expected_exit: 0

  - id: sec-002
    type: package_install
    priority: high
    description: Install fail2ban
    risk: low
    reversible: true
    packages:
      - fail2ban
    estimated_size: "~15MB"

  - id: sec-003
    type: service_operation
    priority: high
    description: Enable and start firewalld
    risk: low
    reversible: true
    service: firewalld
    state: enabled
    start: true
    depends_on:
      - sec-004
    validation:
      command: "firewall-cmd --state"
      expected_output: "running"

post_apply:
  - type: service_restart
    services:
      - sshd
      - firewalld
  - type: validation
    command: "firewall-cmd --state"
    expected_output: "running"
  - type: reboot_required
    reason: "SELinux mode change requires reboot"
```

### Bash Script Export

```bash
#!/bin/bash
# Generated by GuestKit v0.3.1
# Profile: security
# VM: production-web-01.qcow2
# Generated: 2026-01-27T14:30:00Z

set -e

echo "Applying security fixes..."

# Create backup
BACKUP_DIR="/backup/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$BACKUP_DIR"

# sec-001: Disable root SSH login
cp "/etc/ssh/sshd_config" "$BACKUP_DIR/"
sed -i 's/PermitRootLogin yes/PermitRootLogin no/g' "/etc/ssh/sshd_config"
sshd -t || { echo "Validation failed for sec-001"; exit 1; }

# sec-004: Install firewalld
dnf install -y firewalld

# sec-003: Enable and start firewalld
systemctl enable firewalld
systemctl start firewalld
firewall-cmd --state || { echo "Validation failed for sec-003"; exit 1; }

# Post-apply actions
systemctl restart sshd
systemctl restart firewalld
firewall-cmd --state

echo "✓ All fixes applied successfully"
```

### Ansible Playbook Export

```yaml
---
- name: GuestKit security Fixes
  hosts: vm
  become: yes
  tasks:
    - name: Disable root SSH login
      lineinfile:
        path: /etc/ssh/sshd_config
        regexp: '^PermitRootLogin yes$'
        line: 'PermitRootLogin no'
        backup: yes
      notify: restart sshd

    - name: Install firewalld
      package:
        name:
          - firewalld
        state: present

    - name: Enable and start firewalld
      service:
        name: firewalld
        enabled: yes
        state: started
```

## Preview Output

```
📋 Fix Plan Preview
════════════════════════════════════════════════════════════

VM: production-web-01.qcow2
Profile: security (HIGH risk)
Operations: 6
Estimated Duration: 5-10 minutes

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🔴 CRITICAL Priority (2 operations)

[sec-001] Disable root SSH login
  File: /etc/ssh/sshd_config
  Line 28: PermitRootLogin yes → PermitRootLogin no
  Risk: Low | Reversible: Yes

[sec-005] Set SELinux to enforcing mode
  File: /etc/selinux/config
  permissive → enforcing
  ⚠️  Requires reboot to take full effect

🟠 HIGH Priority (3 operations)

[sec-002] Install fail2ban
  Packages: fail2ban (~15MB)

[sec-003] Enable firewalld service
  Service: firewalld (enable + start)
  Depends on: [sec-004]

[sec-004] Install firewalld
  Packages: firewalld

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Dependencies:
  sec-004 → sec-003

Post-Apply Actions:
  • Restart services: sshd, firewalld
  • Validate: firewall-cmd --state
  ⚠️ Reboot required: SELinux mode change requires reboot

Backup: Will create automatic backup
Rollback: Available for all operations
```

## Safety Features

### 1. **Validation Before Apply**
- Check VM exists
- Detect circular dependencies
- Verify all dependencies exist
- Warn about non-reversible operations

### 2. **Dry-Run Mode**
- Simulate application without changes
- Report what would be done
- Validate plan structure

### 3. **Backup Creation**
- Automatic backup before applying
- Timestamped backup directory
- Includes all modified files

### 4. **Rollback Capability** (Planned)
- Restore from backup
- Undo individual operations
- Transaction-like behavior

### 5. **Dependency Management**
- Automatic dependency detection
- Topological sort for execution order
- Circular dependency prevention

## Current Status (Phase 2)

- ✅ Plan generation from profiles and boot reports
- ✅ Preview and diff display
- ✅ Export to bash/ansible/json/yaml
- ✅ Validation framework
- ✅ **`guestkit plan apply`** — apply fix plans with dry-run and backup
- ✅ **`guestkit plan rollback`** — restore from backup
- ✅ **`guestkit repair --fix boot`** — doctor-driven boot repair loop
- ✅ **`guestkit migrate-plan --export`** — migration scoring → fix plan
- ⏳ **Progress tracking during apply** (Phase 2 polish)
- ✅ **TUI preview** (Phase 3 partial) — read-only operation list from Assurance (`p`, `: plan preview`)
- ⏳ **TUI apply** (Phase 3) — apply/rollback with write mount and progress UI

## Roadmap

### Phase 2: Application & Safety (mostly complete)
- ✅ Plan application with dry-run and backup
- ✅ Rollback execution
- ⏳ Progress tracking and recovery UX polish
- ⏳ Error handling hardening

### Phase 3: TUI Integration
- ✅ Read-only plan preview in Assurance view (v0.3.6)
- ✅ Export fix plan from TUI (`e`, `: export plan`)
- ⏳ Checkbox operation selection
- ⏳ Apply from TUI (requires write mount + backups)

### Phase 4: Advanced Features
- Plan merging and composition
- Incremental application
- Remote application (via SSH)
- Fleet-wide plan deployment
- Plan versioning and history

## Use Cases

### 1. **Security Hardening**
```bash
# Generate security fixes
guestkit plan generate prod-web.qcow2 --profile security --output security.yaml

# Review and approve
guestkit plan preview security.yaml

# Export for change control
guestkit plan export security.yaml --format bash > security-fixes.sh

# Apply in maintenance window
guestkit plan apply security.yaml --backup /backups/
```

### 2. **Fleet Management**
```bash
# Generate plan from one VM
guestkit plan generate template.qcow2 --profile security --output fleet-security.yaml

# Export to Ansible
guestkit plan export fleet-security.yaml --format ansible > fleet.yml

# Apply to entire fleet
ansible-playbook -i inventory fleet.yml
```

### 3. **Compliance Automation**
```bash
# Generate compliance fixes
guestkit plan generate vm.qcow2 --profile security --output compliance.yaml

# Store in version control
git add compliance.yaml
git commit -m "Add compliance fixes for Q1 2026"

# Review in PR
git push origin compliance-fixes

# Apply after approval
guestkit plan apply compliance.yaml
```

### 4. **Migration Preparation**
```bash
# Generate migration fixes
guestkit migrate-plan hyperv-vm.vhdx --target kvm --export migration.yaml

# Preview changes
guestkit plan preview migration.yaml

# Export as runbook
guestkit plan export migration.yaml --format bash > migration-runbook.sh

# Execute during migration
bash migration-runbook.sh
```

## Best Practices

1. **Always Preview First**
   - Review plans before applying
   - Check dependencies and order
   - Validate risk levels

2. **Version Control Plans**
   - Store plans in git
   - Review in pull requests
   - Track changes over time

3. **Test in Staging**
   - Apply to test VMs first
   - Validate results
   - Then promote to production

4. **Use Backups**
   - Always enable backup before apply
   - Keep backups for rollback
   - Test restore procedures

5. **Document Changes**
   - Add descriptions to plans
   - Tag appropriately
   - Include rationale in metadata

## Integration with Existing Features

- **Security Profiles**: Generate plans from security findings
- **Compliance Profiles**: Automated compliance remediation
- **Migration Profiles**: Pre-migration fixes
- **TUI Dashboard**: Visual plan management (Phase 3)
- **Batch Processing**: Apply plans to fleets
- **Export Formats**: HTML/PDF reports with plans

## Contributing

To extend the plan system:

1. **Add New Operation Types**: Edit `types.rs`
2. **Improve Remediation Parsing**: Edit `generator.rs`
3. **Add Export Formats**: Edit `export.rs`
4. **Enhance Preview**: Edit `preview.rs`

## References

- [Profiles](../user-guides/profiles.md)
- [Export Formats](export-formats.md)
- [TUI enhancements](tui-enhancements.md)

---

**Last Updated:** 2026-01-27
**Status:** Phase 1 Complete, Phase 2 In Progress
