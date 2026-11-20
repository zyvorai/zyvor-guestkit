# GuestKit Batch/Script Mode Examples

This directory contains example batch scripts for automating disk inspection tasks.

## Usage

Execute a batch script:
```bash
guestkit script disk.qcow2 inspect.gk
```

Execute with fail-fast mode (stop on first error):
```bash
guestkit script disk.qcow2 inspect.gk --fail-fast
```

Use the batch alias:
```bash
guestkit batch disk.qcow2 security-audit.gk
```

Enable verbose output:
```bash
guestkit -v script disk.qcow2 inspect.gk
```

## Example Scripts

### inspect.gk
Comprehensive disk inspection that extracts:
- List of installed packages
- Running services
- OS release information
- Configuration files
- System logs

**Use case**: General VM inventory and documentation

### security-audit.gk
Security-focused inspection that extracts:
- User accounts with shell access
- SSH configuration
- Active services
- Network configuration
- SUID binaries
- Cron jobs
- Authentication and system logs

**Use case**: Security auditing and compliance checking

## Script Syntax

### Comments
Lines starting with `#` are ignored:
```bash
# This is a comment
```

### Available Commands

#### File Operations
- `mount <device> <mountpoint>` - Mount a device
- `umount <mountpoint>` - Unmount a device
- `ls [path]` - List directory contents
- `cat <path>` - Display file contents
- `find <pattern>` - Find files matching pattern
- `download <remote> <local>` - Download file from guest to host

#### Inspection Commands
- `packages` or `pkg` - List installed packages
- `services` or `svc` - List system services

### Output Redirection
Redirect command output to a file:
```bash
packages > packages.txt
services > services.txt
```

### Error Handling
By default, script execution continues on errors. Use `--fail-fast` to stop on first error:
```bash
guestkit script disk.qcow2 script.gk --fail-fast
```

## Execution Report

After execution, you'll see a detailed report:
```
============================================================
Batch Execution Report
============================================================

Script: inspect.gk
Total Commands: 10
Successful: 10
Failed: 0

============================================================
✓ All commands executed successfully!
```

## Creating Your Own Scripts

1. Create a new `.gk` file (or any text file)
2. Add commands one per line
3. Use `#` for comments
4. Use `>` for output redirection
5. Test with `--fail-fast` first
6. Run in production without `--fail-fast` for resilience

## Tips

- Always mount before accessing files
- Always unmount when done
- Use comments to document your script
- Test scripts on test VMs first
- Use output redirection to save results
- Chain scripts together with shell scripting

## Example Workflow

```bash
# Run batch inspection
guestkit script vm1.qcow2 inspect.gk

# Compare results from multiple VMs
guestkit script vm1.qcow2 inspect.gk
guestkit script vm2.qcow2 inspect.gk
diff vm1-packages.txt vm2-packages.txt

# Security audit across fleet
for vm in vm*.qcow2; do
    guestkit script "$vm" security-audit.gk
    mkdir -p "results/$(basename $vm .qcow2)"
    mv *.txt *.log "results/$(basename $vm .qcow2)/"
done
```

## Integration with CI/CD

```yaml
# Example GitHub Actions workflow
- name: Inspect VM Image
  run: |
    guestkit script build/image.qcow2 inspect.gk --fail-fast

- name: Upload Inspection Results
  uses: actions/upload-artifact@v3
  with:
    name: inspection-results
    path: '*.txt'
```
