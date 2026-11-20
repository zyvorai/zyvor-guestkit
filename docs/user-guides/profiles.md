# Inspection Profiles Guide

GuestCtl provides specialized inspection profiles for focused VM analysis. Each profile targets specific use cases and provides tailored output.

## Table of Contents

- [Overview](#overview)
- [Security Profile](#security-profile)
- [Migration Profile](#migration-profile)
- [Performance Profile](#performance-profile)
- [Using Profiles](#using-profiles)
- [Output Formats](#output-formats)

## Overview

Inspection profiles analyze VMs from different perspectives:

- **Security** - Audit security posture and identify risks
- **Migration** - Inventory for VM migration planning
- **Performance** - Identify tuning opportunities
- **Compliance / Hardening / Windows-migration** - Specialized inspect reports

Each inspect profile:
- Reuses existing inspection functions
- Provides focused, relevant output
- Supports JSON/YAML output for automation
- Generates actionable findings

**Generate-only plan profiles** (not inspect reports) live under `plan generate`:
`windows-rdp`, `windows-hostname`, `windows-winrm`, `linux-ssh`. See
[fix-plans.md](../features/fix-plans.md).

## Security Profile

Audit VM security configuration and identify risks.

### Usage

```bash
# Basic security audit
guestkit inspect vm.qcow2 --profile security

# JSON output for automation
guestkit inspect vm.qcow2 --profile security --output json

# Export as HTML report
guestkit inspect vm.qcow2 --profile security --export html --export-output security-audit.html
```

### What It Checks

#### SSH Configuration
- Root login permitted (Critical if enabled)
- Password authentication status
- SSH port (22 or custom)
- SSH service enabled/disabled

#### User Security
- Users with UID 0 (should only be root)
- Disabled login accounts
- Total user count

#### Firewall & Mandatory Access Control
- Firewall status (enabled/disabled)
- SELinux mode (enforcing/permissive/disabled)
- AppArmor status

#### Kernel Security
- Kernel version
- Kernel parameters affecting security

### Risk Scoring

The security profile calculates an overall risk level:

- **Critical** - Multiple critical findings (root SSH, no firewall, etc.)
- **High** - Several high-risk issues
- **Medium** - Some warnings, mostly secure
- **Low** - Minor issues only
- **Info** - No security concerns

### Example Output

```
=== Security Audit Profile ===

━━━ SSH Configuration ━━━

  ✗ PermitRootLogin: yes (CRITICAL - should be 'no')
  ✓ PasswordAuthentication: no
  ✓ Port: 22

━━━ User Accounts ━━━

  ⚠ Users with UID 0: 1
  ✓ Disabled logins: 0

━━━ Firewall & MAC ━━━

  ✗ Firewall: disabled (CRITICAL)
  ⚠ SELinux: permissive (should be enforcing)

━━━ Kernel ━━━

  ℹ Kernel: 6.6.8-200.fc39.x86_64

Overall Risk: HIGH
```

### Automation Example

```bash
# Run security audit on all VMs and save JSON results
for vm in prod-*.qcow2; do
    guestkit inspect "$vm" --profile security --output json > "audit-$(basename $vm .qcow2).json"
done

# Extract critical findings
jq '.sections[] | select(.findings[] | .status == "Fail") | .title' audit-*.json
```

## Migration Profile

Generate comprehensive inventory for VM migration planning.

### Usage

```bash
# Migration inventory
guestkit inspect vm.qcow2 --profile migration

# Export as JSON for migration tools
guestkit inspect vm.qcow2 --profile migration --output json > migration-plan.json

# Export as Markdown for documentation
guestkit inspect vm.qcow2 --profile migration --export markdown --export-output migration-checklist.md
```

### What It Captures

#### Operating System Details
- Distribution and version
- Architecture
- Hostname
- Product name
- Package format and manager

#### Package Inventory
- Total package count
- Installed kernel versions
- Package format (dpkg, rpm, etc.)

#### Storage Configuration
- LVM physical volumes, volume groups, logical volumes
- Swap devices
- Filesystem mounts from /etc/fstab
- Partition layout

#### Network Configuration
- Network interfaces with IPs and MAC addresses
- DHCP status per interface
- DNS servers
- Static routes (count)

#### System Services
- Enabled services (systemd units)
- Service states
- Timers

#### User Accounts
- Regular users (UID ≥ 1000)
- System accounts count
- Home directories

### Example Output

```
=== Migration Planning Profile ===

━━━ Operating System ━━━

  Distribution: Fedora
  Version: 39.0
  Architecture: x86_64
  Hostname: prod-web-01
  Package Manager: dnf

━━━ Package Inventory ━━━

  Total Packages: 1247
  Package Format: rpm

  Installed Kernels:
    • kernel-6.6.8-200.fc39.x86_64
    • kernel-6.5.6-300.fc39.x86_64

━━━ Storage Layout ━━━

  LVM Physical Volumes:
    • /dev/sda2

  LVM Volume Groups:
    • fedora

  LVM Logical Volumes:
    • /dev/fedora/root
    • /dev/fedora/swap

  Swap Devices:
    • /dev/fedora/swap

━━━ Network Configuration ━━━

  Network Interfaces:
    • eth0: 192.168.1.100 (MAC: 52:54:00:12:34:56, DHCP: no)

  DNS Servers:
    • 8.8.8.8
    • 8.8.4.4

━━━ System Services ━━━

  Enabled Services: 42
    • sshd.service (active)
    • firewalld.service (active)
    • httpd.service (active)
    [... truncated ...]

━━━ User Accounts ━━━

  Regular Users: 3
  System Accounts: 39
```

### Migration Workflow

```bash
# 1. Inventory source VM
guestkit inspect source-vm.qcow2 --profile migration --output json > source-inventory.json

# 2. Compare source and target environments
guestkit inspect source-vm.qcow2 --profile migration > source.txt
guestkit inspect target-vm.qcow2 --profile migration > target.txt
diff source.txt target.txt

# 3. Generate migration checklist
guestkit inspect source-vm.qcow2 --profile migration --export markdown --export-output migration-checklist.md
```

## Performance Profile

Identify performance tuning opportunities and potential bottlenecks.

### Usage

```bash
# Performance analysis
guestkit inspect vm.qcow2 --profile performance

# JSON output for monitoring systems
guestkit inspect vm.qcow2 --profile performance --output json > perf-baseline.json
```

### What It Analyzes

#### Kernel Parameters
- vm.swappiness
- net.core.* network tuning
- fs.* filesystem parameters
- Recommendation: values outside optimal ranges

#### Swap Configuration
- Swap device paths
- Swap usage patterns (if detectable)

#### Disk I/O
- I/O scheduler (CFQ, deadline, noop, none)
- Mount options (noatime, relatime, etc.)
- Filesystem types and their performance characteristics

#### Network Tuning
- TCP buffer sizes
- Network stack parameters
- Interface configuration

#### System Services
- Resource-heavy services
- Boot time impact
- Unnecessary services

### Example Output

```
=== Performance Tuning Profile ===

━━━ Kernel Parameters ━━━

  ⚠ vm.swappiness: 60 (consider reducing to 10-20 for server workloads)
  ✓ net.core.rmem_max: 212992
  ✓ net.core.wmem_max: 212992

━━━ Swap Configuration ━━━

  Swap Devices:
    • /dev/fedora/swap

━━━ Disk I/O ━━━

  ℹ Default I/O Scheduler: Check /sys/block/*/queue/scheduler

  Mount Options:
    • / mounted with: rw,relatime
    ⚠ Consider: noatime for better performance

━━━ Network Configuration ━━━

  Network Interfaces: 1

━━━ System Services ━━━

  Enabled Services: 42
  ℹ Review service list for unnecessary daemons
```

### Performance Tuning Workflow

```bash
# 1. Establish baseline
guestkit inspect vm.qcow2 --profile performance > baseline-perf.txt

# 2. Apply tuning changes to VM
# (manual VM configuration)

# 3. Re-inspect to verify changes
guestkit inspect vm-tuned.qcow2 --profile performance > tuned-perf.txt

# 4. Compare before/after
diff baseline-perf.txt tuned-perf.txt
```

## Using Profiles

### Command-Line Syntax

```bash
guestkit inspect <IMAGE> --profile <PROFILE_NAME> [OPTIONS]
```

**Available profiles:**
- `security` - Security audit
- `migration` - Migration inventory
- `performance` - Performance analysis

### Combining with Other Options

#### Output Formats

```bash
# JSON output
guestkit inspect vm.qcow2 --profile security --output json

# YAML output
guestkit inspect vm.qcow2 --profile migration --output yaml

# Default text output (human-readable)
guestkit inspect vm.qcow2 --profile performance
```

#### Export Formats

```bash
# HTML report
guestkit inspect vm.qcow2 --profile security --export html --export-output report.html

# Markdown documentation
guestkit inspect vm.qcow2 --profile migration --export markdown --export-output inventory.md
```

#### Caching

**Note:** Profiles cannot be used with cached results because they require live inspection data.

```bash
# This will warn and skip profile
guestkit inspect vm.qcow2 --profile security

# Caching is on by default; --cache-refresh ignores cached results
guestkit inspect vm.qcow2 --profile security --cache-refresh
```

## Output Formats

### Text (Default)

Human-readable output with sections, icons, and colors:

```
=== Security Audit Profile ===

━━━ SSH Configuration ━━━
  ✓ PermitRootLogin: no
  ✗ PasswordAuthentication: yes (CRITICAL)
```

### JSON

Structured data for automation:

```json
{
  "profile_name": "Security Audit",
  "sections": [
    {
      "title": "SSH Configuration",
      "findings": [
        {
          "label": "PermitRootLogin",
          "value": "no",
          "status": "Pass",
          "severity": null,
          "recommendation": null
        },
        {
          "label": "PasswordAuthentication",
          "value": "yes",
          "status": "Fail",
          "severity": "Critical",
          "recommendation": "Disable password authentication, use key-based auth"
        }
      ]
    }
  ],
  "risk_level": "High"
}
```

### YAML

Configuration-friendly format:

```yaml
profile_name: Security Audit
sections:
  - title: SSH Configuration
    findings:
      - label: PermitRootLogin
        value: 'no'
        status: Pass
      - label: PasswordAuthentication
        value: 'yes'
        status: Fail
        severity: Critical
        recommendation: Disable password authentication, use key-based auth
risk_level: High
```

## Best Practices

### Security Profile

1. **Regular Audits**: Run security audits on all production VMs monthly
2. **Baseline Comparison**: Store JSON baselines and track changes over time
3. **Automated Scanning**: Integrate into CI/CD for golden image validation
4. **Remediation Tracking**: Export HTML reports for audit documentation

### Migration Profile

1. **Pre-Migration Inventory**: Always run before migration planning
2. **Comparison**: Diff source and target to identify compatibility issues
3. **Documentation**: Export Markdown for migration runbooks
4. **Validation**: Re-run on target after migration to verify completeness

### Performance Profile

1. **Baseline Before Tuning**: Always capture before-state
2. **Iterative Tuning**: Apply one change at a time, re-inspect after each
3. **Monitoring Integration**: Export JSON for integration with monitoring tools
4. **Periodic Reviews**: Re-analyze every quarter to catch drift

## Examples

### Security Compliance Workflow

```bash
#!/bin/bash
# Audit all production VMs for compliance

REPORT_DIR="security-audits-$(date +%Y%m%d)"
mkdir -p "$REPORT_DIR"

for vm in prod-*.qcow2; do
    NAME=$(basename "$vm" .qcow2)

    # Run security audit
    guestkit inspect "$vm" --profile security \
        --export html --export-output "$REPORT_DIR/$NAME-audit.html"

    # Also save JSON for trending
    guestkit inspect "$vm" --profile security --output json > "$REPORT_DIR/$NAME-audit.json"

    # Extract risk level
    RISK=$(jq -r '.risk_level' "$REPORT_DIR/$NAME-audit.json")
    echo "$NAME: $RISK"
done

echo "Reports saved to $REPORT_DIR/"
```

### Migration Planning Workflow

```bash
#!/bin/bash
# Generate migration checklist

SOURCE="legacy-app.qcow2"
TARGET_ENV="aws"

# Generate complete inventory
guestkit inspect "$SOURCE" --profile migration \
    --export markdown --export-output "migration-checklist-$TARGET_ENV.md"

# Extract package list for compatibility check
guestkit inspect "$SOURCE" --profile migration --output json | \
    jq -r '.sections[] | select(.title == "Package Inventory") | .findings[] | "\(.label): \(.value)"' \
    > packages-to-migrate.txt

echo "Migration checklist generated"
echo "Review migration-checklist-$TARGET_ENV.md"
```

### Performance Baseline Workflow

```bash
#!/bin/bash
# Establish performance baseline for all VMs

BASELINE_DIR="perf-baselines-$(date +%Y%m%d)"
mkdir -p "$BASELINE_DIR"

for vm in *.qcow2; do
    NAME=$(basename "$vm" .qcow2)

    guestkit inspect "$vm" --profile performance --output json \
        > "$BASELINE_DIR/$NAME-perf.json"

    # Extract swappiness value
    SWAP=$(jq -r '.sections[] | select(.title == "Kernel Parameters") | .findings[] | select(.label | contains("swappiness")) | .value' \
        "$BASELINE_DIR/$NAME-perf.json")

    echo "$NAME: swappiness=$SWAP"
done

echo "Baselines saved to $BASELINE_DIR/"
```

## See Also

- [CLI Guide](cli-guide.md) - Complete CLI reference
- [Export formats](../features/export-formats.md) - Report generation and export
