# Export & Reporting Guide

GuestCtl can export VM inspection results in multiple formats for documentation, compliance, and reporting purposes.

## Table of Contents

- [Overview](#overview)
- [HTML Reports](#html-reports)
- [Markdown Export](#markdown-export)
- [Output Formats vs Export Formats](#output-formats-vs-export-formats)
- [Use Cases](#use-cases)
- [Examples](#examples)

## Overview

GuestCtl supports two types of output:

### Output Formats (--output)
Structured data for automation:
- `json` - JSON format
- `yaml` - YAML format
- `text` - Plain text (default)

### Export Formats (--export)
Document generation for humans:
- `html` - Interactive HTML reports
- `markdown` - Git-friendly Markdown documents

## HTML Reports

Generate self-contained, interactive HTML reports for documentation and compliance.

### Features

- **Self-Contained**: All CSS and JavaScript embedded, no external dependencies
- **Interactive**: Collapsible sections for better readability
- **Professional Design**: Gradient header, modern typography, clean layout
- **Print-Friendly**: Optimized CSS for printing and PDF conversion
- **Responsive**: Works on desktop and mobile devices

### Usage

```bash
# Basic HTML report
guestkit inspect vm.qcow2 --export html --export-output report.html

# With security profile
guestkit inspect vm.qcow2 --profile security --export html --export-output security-audit.html

# With migration profile
guestkit inspect vm.qcow2 --profile migration --export html --export-output migration-plan.html
```

### What's Included

HTML reports include:

- **Header Section**
  - VM name (from hostname)
  - Generation timestamp
  - Gradient background with branding

- **Summary Cards**
  - OS type and distribution
  - Version number
  - Architecture
  - Hostname

- **Detailed Sections** (Collapsible)
  - Operating System details
  - Packages (up to 100 shown in HTML)
  - Services (all enabled services)
  - Users (regular users only)
  - Network interfaces

### Opening HTML Reports

```bash
# Generate report
guestkit inspect vm.qcow2 --export html --export-output report.html

# Open in browser (Linux)
xdg-open report.html

# Open in browser (macOS)
open report.html

# Open in browser (Windows)
start report.html
```

### Converting to PDF

```bash
# Method 1: Using wkhtmltopdf
guestkit inspect vm.qcow2 --export html --export-output report.html
wkhtmltopdf report.html report.pdf

# Method 2: Using headless Chrome
chrome --headless --print-to-pdf=report.pdf report.html

# Method 3: Using Firefox
firefox --headless --print-to-pdf=report.pdf report.html

# Method 4: Print from browser (Ctrl+P or Cmd+P)
# Select "Save as PDF" as printer
```

### Example HTML Structure

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <title>GuestCtl Inspection Report - fedora-server</title>
    <style>
        /* Embedded CSS - modern, clean design */
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>VM Inspection Report</h1>
            <div class="meta">Generated: 2026-01-24 15:30:00</div>
        </div>

        <div class="summary">
            <!-- Summary cards -->
        </div>

        <div class="content">
            <!-- Collapsible sections -->
        </div>
    </div>

    <script>
        // JavaScript for section toggling
    </script>
</body>
</html>
```

## Markdown Export

Generate Git-friendly Markdown documents for version-controlled VM inventory.

### Features

- **Git-Friendly**: Plain text format, easy to diff and version
- **Documentation-Ready**: Clean Markdown with tables and lists
- **Portable**: View anywhere (GitHub, GitLab, editors, browsers)
- **Scriptable**: Easy to parse and transform

### Usage

```bash
# Basic Markdown export
guestkit inspect vm.qcow2 --export markdown --export-output inventory.md

# Track in Git
guestkit inspect prod-vm.qcow2 --export markdown --export-output inventory/prod-vm.md
git add inventory/prod-vm.md
git commit -m "Update prod VM inventory"

# Generate documentation site
guestkit inspect vm.qcow2 --export markdown --export-output docs/infrastructure/vm-inventory.md
```

### What's Included

Markdown exports include:

- **Header**
  - Report title with VM name
  - Generation timestamp

- **Operating System**
  - Type, distribution, version
  - Architecture, hostname
  - Product name
  - Package format and manager

- **Packages**
  - Total count
  - Installed kernel versions

- **Services**
  - Count of enabled services
  - Table of service names and states

- **Users**
  - Regular users count
  - System accounts count
  - Table of usernames, UIDs, home directories

- **Network**
  - Network interfaces table (name, IPs, MAC, DHCP)
  - DNS servers list

- **Filesystems**
  - Filesystem table (device, mount point, type)

- **Storage**
  - LVM physical volumes, volume groups, logical volumes

- **System Configuration**
  - Timezone
  - SELinux status

### Example Markdown Output

````markdown
# VM Inspection Report: fedora-server

**Generated:** 2026-01-24 15:30:00

---

## Operating System

- **Type:** linux
- **Distribution:** Fedora
- **Version:** 39.0
- **Architecture:** x86_64
- **Hostname:** fedora-server
- **Product:** Fedora Linux 39
- **Package Format:** rpm
- **Package Management:** dnf

## Packages

**Total Packages:** 1247

### Installed Kernels

- kernel-6.6.8-200.fc39.x86_64
- kernel-6.5.6-300.fc39.x86_64

## Services

**Enabled Services:** 42

| Service | State |
|---------|-------|
| sshd.service | active |
| firewalld.service | active |
| httpd.service | active |
...and 39 more services

## User Accounts

**Regular Users:** 3
**System Accounts:** 39

### Regular Users

| Username | UID | Home Directory |
|----------|-----|----------------|
| john | 1000 | /home/john |
| jane | 1001 | /home/jane |
| admin | 1002 | /home/admin |

## Network Configuration

**Network Interfaces:** 1

| Interface | IP Address | MAC Address | DHCP |
|-----------|------------|-------------|------|
| eth0      | 192.168.1.100 | 52:54:00:12:34:56 | No |

### DNS Servers

- 8.8.8.8
- 8.8.4.4

---

*Generated by GuestCtl - Pure Rust VM Inspection Toolkit*
````

## Output Formats vs Export Formats

### Key Differences

| Feature | Output Formats (--output) | Export Formats (--export) |
|---------|---------------------------|---------------------------|
| Purpose | Automation, scripting | Documentation, reporting |
| Target | Machines, tools | Humans, browsers |
| Formats | JSON, YAML, text | HTML, Markdown |
| Self-Contained | Depends on viewer | Yes (HTML embeds CSS/JS) |
| Version Control | JSON/YAML can be tracked | Markdown is ideal for Git |
| Interactive | No | Yes (HTML collapsible sections) |

### When to Use Each

**Use Output Formats (--output) when:**
- Integrating with automation tools
- Feeding data to monitoring systems
- Building CI/CD pipelines
- Querying with jq/yq
- Processing with scripts

**Use Export Formats (--export) when:**
- Creating documentation
- Generating compliance reports
- Sharing with non-technical stakeholders
- Building knowledge bases
- Archiving VM configurations

### Can You Use Both?

No, they are mutually exclusive:

```bash
# This will use export and ignore output format
guestkit inspect vm.qcow2 --output json --export html --export-output report.html
# Result: HTML report is generated, JSON output is not

# For both, run twice:
guestkit inspect vm.qcow2 --output json > data.json
guestkit inspect vm.qcow2 --export html --export-output report.html
```

## Use Cases

### Compliance Documentation

Generate HTML reports for audit trails:

```bash
#!/bin/bash
# Monthly compliance reports

MONTH=$(date +%Y-%m)
REPORT_DIR="compliance-reports-$MONTH"
mkdir -p "$REPORT_DIR"

for vm in prod-*.qcow2; do
    NAME=$(basename "$vm" .qcow2)

    guestkit inspect "$vm" --profile security \
        --export html --export-output "$REPORT_DIR/$NAME-compliance.html"
done

# Create index
cat > "$REPORT_DIR/index.html" << EOF
<html>
<head><title>Compliance Reports - $MONTH</title></head>
<body>
<h1>Monthly Compliance Reports - $MONTH</h1>
<ul>
$(for f in "$REPORT_DIR"/*.html; do
    [[ "$f" != *"index.html" ]] && echo "<li><a href=\"$(basename "$f")\">$(basename "$f" .html)</a></li>"
done)
</ul>
</body>
</html>
EOF

echo "Reports generated in $REPORT_DIR/"
echo "Open $REPORT_DIR/index.html to view"
```

### Infrastructure Documentation

Maintain Git-tracked VM inventory:

```bash
#!/bin/bash
# Update VM inventory in documentation repository

DOC_REPO="./infrastructure-docs"
INVENTORY_DIR="$DOC_REPO/vm-inventory"

mkdir -p "$INVENTORY_DIR"

for vm in *.qcow2; do
    NAME=$(basename "$vm" .qcow2)

    guestkit inspect "$vm" --export markdown \
        --export-output "$INVENTORY_DIR/$NAME.md"
done

# Commit changes
cd "$DOC_REPO"
git add vm-inventory/
git commit -m "Update VM inventory $(date +%Y-%m-%d)"
git push

echo "VM inventory updated in Git"
```

### Migration Documentation

Generate migration checklists:

```bash
#!/bin/bash
# Create migration documentation package

SOURCE_VM="$1"
TARGET_ENV="$2"

if [ -z "$SOURCE_VM" ] || [ -z "$TARGET_ENV" ]; then
    echo "Usage: $0 <source-vm.qcow2> <target-environment>"
    exit 1
fi

NAME=$(basename "$SOURCE_VM" .qcow2)
DOC_DIR="migration-$NAME-to-$TARGET_ENV"

mkdir -p "$DOC_DIR"

# HTML report for stakeholders
guestkit inspect "$SOURCE_VM" --profile migration \
    --export html --export-output "$DOC_DIR/migration-overview.html"

# Markdown checklist for engineers
guestkit inspect "$SOURCE_VM" --profile migration \
    --export markdown --export-output "$DOC_DIR/migration-checklist.md"

# JSON for automation
guestkit inspect "$SOURCE_VM" --profile migration --output json \
    > "$DOC_DIR/migration-data.json"

# Create README
cat > "$DOC_DIR/README.md" << EOF
# Migration: $NAME to $TARGET_ENV

Generated: $(date +%Y-%m-%d)

## Files

- \`migration-overview.html\` - High-level overview for stakeholders
- \`migration-checklist.md\` - Detailed checklist for engineers
- \`migration-data.json\` - Machine-readable data for automation

## Next Steps

1. Review migration-overview.html with stakeholders
2. Use migration-checklist.md during migration
3. Feed migration-data.json to automation tools

EOF

echo "Migration documentation package created in $DOC_DIR/"
```

### Performance Baseline Documentation

Track performance baselines over time:

```bash
#!/bin/bash
# Performance baseline tracking

VM="$1"
NAME=$(basename "$VM" .qcow2)
DATE=$(date +%Y-%m-%d)
BASELINE_DIR="performance-baselines/$NAME"

mkdir -p "$BASELINE_DIR"

# Generate baseline report
guestkit inspect "$VM" --profile performance \
    --export markdown --export-output "$BASELINE_DIR/baseline-$DATE.md"

# Also save JSON for trending
guestkit inspect "$VM" --profile performance --output json \
    > "$BASELINE_DIR/baseline-$DATE.json"

# Create comparison if previous baseline exists
LATEST=$(ls -t "$BASELINE_DIR"/baseline-*.md 2>/dev/null | head -n2 | tail -n1)
if [ -n "$LATEST" ] && [ "$LATEST" != "$BASELINE_DIR/baseline-$DATE.md" ]; then
    diff "$LATEST" "$BASELINE_DIR/baseline-$DATE.md" > "$BASELINE_DIR/diff-$(basename "$LATEST" .md)-to-$DATE.txt"
    echo "Comparison saved to $BASELINE_DIR/diff-$(basename "$LATEST" .md)-to-$DATE.txt"
fi

echo "Performance baseline saved to $BASELINE_DIR/"
```

## Examples

### Generate All Report Types

```bash
#!/bin/bash
# Generate all report formats for a VM

VM="$1"
NAME=$(basename "$VM" .qcow2)
OUTPUT_DIR="reports-$NAME"

mkdir -p "$OUTPUT_DIR"

# HTML report
guestkit inspect "$VM" --export html --export-output "$OUTPUT_DIR/report.html"

# Markdown documentation
guestkit inspect "$VM" --export markdown --export-output "$OUTPUT_DIR/inventory.md"

# JSON for automation
guestkit inspect "$VM" --output json > "$OUTPUT_DIR/data.json"

# YAML for configuration
guestkit inspect "$VM" --output yaml > "$OUTPUT_DIR/data.yaml"

# Profile reports
guestkit inspect "$VM" --profile security --export html --export-output "$OUTPUT_DIR/security.html"
guestkit inspect "$VM" --profile migration --export markdown --export-output "$OUTPUT_DIR/migration.md"
guestkit inspect "$VM" --profile performance --export html --export-output "$OUTPUT_DIR/performance.html"

echo "All reports generated in $OUTPUT_DIR/"
ls -lh "$OUTPUT_DIR"
```

### Automated Documentation Site

```bash
#!/bin/bash
# Generate documentation site for all VMs

SITE_DIR="vm-documentation-site"
mkdir -p "$SITE_DIR"/{html,markdown}

# Generate reports for all VMs
for vm in *.qcow2; do
    NAME=$(basename "$vm" .qcow2)

    # HTML for web viewing
    guestkit inspect "$vm" --export html --export-output "$SITE_DIR/html/$NAME.html"

    # Markdown for GitHub/GitLab
    guestkit inspect "$vm" --export markdown --export-output "$SITE_DIR/markdown/$NAME.md"
done

# Create index pages
cat > "$SITE_DIR/html/index.html" << 'EOF'
<!DOCTYPE html>
<html>
<head><title>VM Documentation</title>
<style>
body { font-family: sans-serif; max-width: 800px; margin: 40px auto; padding: 20px; }
h1 { color: #667eea; }
ul { list-style: none; padding: 0; }
li { margin: 10px 0; }
a { color: #764ba2; text-decoration: none; }
a:hover { text-decoration: underline; }
</style>
</head>
<body>
<h1>VM Documentation</h1>
<ul>
EOF

for f in "$SITE_DIR"/html/*.html; do
    [[ "$f" != *"index.html" ]] && \
        echo "<li><a href=\"$(basename "$f")\">$(basename "$f" .html)</a></li>" >> "$SITE_DIR/html/index.html"
done

cat >> "$SITE_DIR/html/index.html" << 'EOF'
</ul>
<footer>
<p><small>Generated by GuestCtl</small></p>
</footer>
</body>
</html>
EOF

# Create Markdown index
{
    echo "# VM Documentation"
    echo ""
    echo "## VMs"
    echo ""
    for f in "$SITE_DIR"/markdown/*.md; do
        echo "- [$(basename "$f" .md)]($(basename "$f"))"
    done
    echo ""
    echo "---"
    echo "*Generated by GuestCtl*"
} > "$SITE_DIR/markdown/README.md"

echo "Documentation site generated in $SITE_DIR/"
echo "HTML index: $SITE_DIR/html/index.html"
echo "Markdown index: $SITE_DIR/markdown/README.md"
```

## Best Practices

### HTML Reports

1. **Descriptive Filenames**: Use meaningful names like `prod-web-01-security-2026-01-24.html`
2. **Archiving**: Store HTML reports in date-organized directories
3. **PDF Conversion**: Convert critical reports to PDF for long-term storage
4. **Sharing**: HTML reports are self-contained, safe to email or share via web
5. **Printing**: Use browser print preview to ensure layout before printing

### Markdown Export

1. **Version Control**: Always commit Markdown exports to Git
2. **Directory Structure**: Organize by environment or service
3. **Naming Convention**: Use consistent names like `<vm-name>-<date>.md`
4. **Documentation Sites**: Use tools like MkDocs or Jekyll to build sites from Markdown
5. **Diffs**: Use `git diff` to track VM configuration changes over time

### General

1. **Automation**: Script report generation for regular updates
2. **Retention**: Define retention policies for reports (e.g., keep 12 months)
3. **Indexing**: Create index pages for collections of reports
4. **Metadata**: Include generation date and tool version in filenames
5. **Validation**: Review generated reports before sharing with stakeholders

## See Also

- [Profiles Guide](../user-guides/profiles.md) - Specialized inspection profiles
- [CLI Guide](../user-guides/cli-guide.md) - Command and flag summary
