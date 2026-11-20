# GuestCtl Inspection Quick Reference

## 🚀 Quick Start

```bash
# Basic inspection
guestkit inspect disk.qcow2

# Verbose mode (detailed logging)
guestkit inspect disk.qcow2 --verbose
```

## 📋 What Gets Inspected

| Category | Information Extracted |
|----------|----------------------|
| **Hardware** | Block devices, partitions, sector sizes, partition schemes |
| **OS** | Type, distribution, version, hostname, architecture |
| **Disk** | Usage, free space, filesystems, labels, UUIDs |
| **Packages** | Package format, manager, installed count |
| **Kernels** | Installed kernel versions in /boot |
| **Network** | Interfaces, IPs, MAC addresses, DHCP, DNS servers |
| **Users** | Regular users, system users, shells, home dirs |
| **SSH** | Port, root login, password auth settings |
| **Security** | SELinux status and mode |
| **Services** | Enabled systemd services, timers |
| **Boot** | Bootloader (GRUB2), timeout, default entry |
| **Storage** | LVM (PVs/VGs/LVs), swap, fstab mounts |
| **Runtimes** | Python, Node.js, Ruby, Java, Go, Perl |
| **Containers** | Docker, Podman, containerd, CRI-O |
| **Tasks** | Cron jobs, systemd timers |
| **Certs** | SSL/TLS certificates in standard locations |
| **Tuning** | Kernel parameters (sysctl.conf) |
| **Cloud** | Cloud-init detection |
| **VM Tools** | VMware Tools, QEMU GA, VirtualBox, Hyper-V |
| **Config** | Timezone, locale |

## Migration assurance

| Command | Purpose |
|---------|---------|
| `guestkit doctor IMAGE --target kvm` | Boot probability + blockers |
| `guestkit migrate-plan IMAGE --target proxmox` | Migration score, drivers, downtime |
| `guestkit migrate-plan IMAGE --target proxmox --export plan.yaml` | Export executable fix plan |
| `guestkit policy check IMAGE --policy FILE` | Policy-as-code (expression DSL) |
| `guestkit fleet analyze ./vms/` | Cluster VMs, snowflakes, blockers |
| `guestkit forensic-diff OLD NEW` | Security drift between snapshots |
| `guestkit repair IMAGE --fix boot` | Boot repair via fix plans |

```bash
guestkit doctor vm.qcow2 --target kvm --explain
guestkit migrate-plan vm.vmdk --target proxmox -o json
guestkit repair vm.qcow2 --fix boot --dry-run
```

Details: [migration-assurance.md](../features/migration-assurance.md)

## Assured QEMU launch (`guestkit-qemu`)

| Command | Purpose |
|---------|---------|
| `guestkit-qemu plan IMAGE` | Inspect with GuestKit → print QEMU plan / argv |
| `guestkit-qemu run IMAGE --min-boot-score 80` | Launch only if assurance gates pass |
| `guestkit-qemu qmp --socket PATH status\|pause\|resume\|balloon\|powerdown` | Day-2 QMP control |

```bash
guestkit-qemu plan vm.qcow2 --memory-mb 8192 --vcpus 4 --json
guestkit-qemu run vm.qcow2 --min-boot-score 80 --qmp-socket /run/guestkit/vm.qmp
guestkit-qemu run vm.qcow2 --uefi-code /usr/share/OVMF/OVMF_CODE.fd --uefi-vars ./vm_VARS.fd
guestkit-qemu qmp --socket /run/guestkit/vm.qmp status
```

Details: [qemu-runtime.md](../features/qemu-runtime.md)

## Live QGA (`guestkit qga` / `agent-call`)

`qga`, `agent-call` and `agent-proxy` need a build with `--features agent` (Unix); the release tarball does not include it, so run `cargo build --release --features agent` first.

| Command | Purpose |
|---------|---------|
| `guestkit qga --execute guest-ping` | Raw QGA over unix socket (auto-discovers libvirt/KubeVirt paths) |
| `guestkit qga --raw '{"execute":"guest-info"}'` | Full QGA JSON body |
| `guestkit agent-call --method guestkit.getVersion` | GuestKit JSON-RPC over the same socket |
| `guestkit agent-proxy --listen 127.0.0.1:8765` | HTTP bridge for live agent methods |

```bash
guestkit qga --execute guest-ping
guestkit qga --socket /var/lib/libvirt/qemu/channel/target/web01/org.qemu.guest_agent.0 \
  --execute guest-ping
guestkit agent-call --method guestkit.getBootAnalysis
```

Do **not** use `virsh qemu-agent-command` — cut-over map:
[virsh-to-guestkit.md](virsh-to-guestkit.md). Emergency fallback only with
`GUESTKIT_ALLOW_VIRSH=1` inside zyvor-api.

## Rescue & day-0

| Command | Purpose |
|---------|---------|
| `guestkit rescue IMAGE -o enable-ssh` | Offline SSH enable (Linux) |
| `guestkit rescue IMAGE -o fix-grub` | Chroot grub-mkconfig / first-boot fallback |
| `guestkit rescue IMAGE -o fix-grub --force` | Also attempt grub-install on NBD |
| `guestkit rescue IMAGE -o reset-password --user U --password P` | Linux shadow; Windows AES SAM (or RunOnce fallback) |
| `guestkit plan generate IMAGE -p linux-grub --grub-timeout 5` | Offline `/etc/default/grub` |
| `guestkit plan generate IMAGE -p linux-ssh --user U --key-file KEY` | Offline SSH day-0 plan |

```bash
guestkit rescue linux.qcow2 -o fix-grub
guestkit rescue win.qcow2 -o reset-password --user Administrator --password 'S3cret!'
export GUESTKIT_PACKAGE_CACHE=~/pkgs GUESTKIT_PACKAGE_FETCH=1
# optional: GUESTKIT_PACKAGE_MIRROR=https://mirror.example/pkgs
guestkit plan apply plan.yaml --vm linux.qcow2 --yes
```

Details: [fix-plans.md](../features/fix-plans.md)

## TUI (`guestctl tui IMAGE`)

| Keys | Action |
|------|--------|
| `Tab` / `Shift+Tab` | Next/prev view in current group |
| `{` / `}` | Previous/next group (Overview · System · Security) |
| `Ctrl+P` | Jump menu (all views, filter + scroll) |
| `Ctrl+Shift+P` | Global search (packages, boot blockers, migration items, …) |
| `:` | Command palette (`doctor`, `migrate-plan`, `export plan`, `goto assurance`) |
| `,` / `.` | Scroll view tab row when tabs overflow |
| `h` / `?` | Full help / context help for current view |

### Assurance view (Security group)

| Keys | Action |
|------|--------|
| `d` | Run doctor (boot gate + migration score) |
| `t` | Cycle target: kvm → proxmox → aws |
| `p` | Read-only fix-plan preview (after load) |
| `e` | Export fix plan YAML to cwd |
| `a` | From **Dashboard**: open Assurance |

Config: `~/.config/guestkit/tui.toml` — `default_migration_target`, `assurance_on_startup`, `density`, glass theme.

[zyvor.dev/guestkit](https://zyvor.dev/guestkit) · [tui-enhancements.md](../features/tui-enhancements.md) · [migration-assurance.md](../features/migration-assurance.md)

## 🎯 Common Commands

```bash
# Inspect a QCOW2 image
guestkit inspect vm-disk.qcow2

# Inspect with verbose logging
guestkit inspect vm-disk.qcow2 --verbose 2>verbose.log

# Inspect a RAW disk
guestkit inspect disk.img

# Inspect and save output
guestkit inspect disk.qcow2 > inspection-report.txt

# Inspect with verbose to separate files
guestkit inspect disk.qcow2 --verbose >report.txt 2>debug.log
```

## 📊 Sample Output Sections

```
=== Block Devices ===
  /dev/sda: 21474836480 bytes (21.47 GB)
    Read-only: no
    Sector size: 512 bytes

=== Partitions ===
  /dev/sda1
    Number: 1
    Start:  1048576 bytes
    Size:   21473787904 bytes (21.47 GB)

=== Operating Systems ===
  Root: /dev/sda1
    Type:         linux
    Distribution: fedora
    Product:      Fedora Linux
    Version:      39.0
    Hostname:     fedora-server
    Init system:  systemd
    Pkg Manager:  dnf

    Disk usage:
      Total: 20.00 GB
      Used:  8.50 GB (42.5%)
      Free:  11.50 GB

    === Network Configuration ===
      Interface: eth0
        IP: 192.168.1.100
        DHCP: no

    === User Accounts ===
      Regular users: 2
        john (uid: 1000)
        jane (uid: 1001)

    === Language Runtimes ===
      python3: installed
      nodejs: installed

    === Container Runtimes ===
      docker
      podman
```

## 🔍 Verbose Output Examples

```bash
$ guestkit inspect disk.qcow2 --verbose

[VERBOSE] Adding drive: disk.qcow2
[VERBOSE] Launching QEMU appliance...
[VERBOSE] Enumerating block devices...
[VERBOSE] Found device: /dev/sda (21474836480 bytes)
[VERBOSE] Analyzing partition table...
[VERBOSE] Examining partition: /dev/sda1
[VERBOSE] Partition scheme: gpt
[VERBOSE] Detecting filesystems...
[VERBOSE] Filesystem on /dev/sda1: ext4
[VERBOSE] Running OS detection algorithms...
[VERBOSE] Inspecting OS at root: /dev/sda1
[VERBOSE] OS type detected: linux
[VERBOSE] Distribution: fedora
[VERBOSE] Gathering system configuration...
[VERBOSE] Analyzing network configuration...
[VERBOSE] Listing user accounts...
[VERBOSE] Detecting language runtimes...
[VERBOSE] Shutting down appliance...
[VERBOSE] Inspection complete
```

## 💡 Pro Tips

### 1. Filter Specific Information
```bash
# Get only network info
guestkit inspect disk.qcow2 | grep -A 20 "Network Configuration"

# Get only user accounts
guestkit inspect disk.qcow2 | grep -A 30 "User Accounts"

# Get OS summary
guestkit inspect disk.qcow2 | grep -A 15 "Operating Systems"
```

### 2. Compare Two VMs
```bash
# Inspect both and compare
guestkit inspect vm1.qcow2 > vm1-report.txt
guestkit inspect vm2.qcow2 > vm2-report.txt
diff vm1-report.txt vm2-report.txt
```

### 3. Extract Specific Data
```bash
# Get hostname
guestkit inspect disk.qcow2 | grep "Hostname:"

# Get installed kernels
guestkit inspect disk.qcow2 | grep -A 5 "Installed kernels"

# Get enabled services
guestkit inspect disk.qcow2 | grep -A 20 "Systemd Services"
```

### 4. Debugging Issues
```bash
# Full verbose output for troubleshooting
guestkit inspect problematic.qcow2 --verbose 2>&1 | tee full-debug.log

# Check what failed
guestkit inspect disk.qcow2 --verbose 2>&1 | grep -i "error\|failed"
```

### 5. Automation
```bash
# Inspect all QCOW2 files in directory
for img in *.qcow2; do
    echo "=== $img ==="
    guestkit inspect "$img"
    echo ""
done > all-vms-report.txt
```

## 🎨 Output Formatting Tips

### Create Summary Report
```bash
#!/bin/bash
DISK=$1
echo "VM Inspection Report"
echo "===================="
echo "Date: $(date)"
echo "Disk: $DISK"
echo ""

guestkit inspect "$DISK" | grep -E "(Root:|Type:|Distribution:|Product:|Version:|Hostname:|Disk usage:)"
```

### Extract JSON-like Data (with jq-style parsing)
```bash
# Get OS info as key-value pairs
guestkit inspect disk.qcow2 | grep -A 10 "Operating Systems" | grep ":" | sed 's/^[[:space:]]*//'
```

## 🔧 Programmatic Usage (Rust)

```rust
use guestkit::guestfs::Guestfs;

fn inspect_vm(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;
    g.add_drive_ro(path)?;
    g.launch()?;

    let roots = g.inspect_os()?;
    for root in &roots {
        // Basic info
        println!("OS: {}", g.inspect_get_product_name(root)?);
        println!("Hostname: {}", g.inspect_get_hostname(root)?);

        // Network
        let interfaces = g.inspect_network(root)?;
        for iface in &interfaces {
            println!("Interface {}: {:?}", iface.name, iface.ip_address);
        }

        // Users
        let users = g.inspect_users(root)?;
        println!("User count: {}", users.len());

        // Services
        let services = g.inspect_systemd_services(root)?;
        println!("Enabled services: {}", services.len());

        // Runtimes
        let runtimes = g.inspect_runtimes(root)?;
        for (name, version) in runtimes {
            println!("Runtime: {} ({})", name, version);
        }
    }

    g.shutdown()?;
    Ok(())
}
```

## 📚 Related Commands

```bash
# List files in VM
guestkit list disk.qcow2 /etc

# Extract file from VM
guestkit extract disk.qcow2 /etc/hostname hostname.txt

# Execute a command in the VM (chroot into the mounted guest root on your host,
# as root: it runs the image's own binaries, so use it only on trusted images)
guestkit exec disk.qcow2 cat /etc/os-release

# Check filesystem
guestkit fsck disk.qcow2

# Show disk usage
guestkit df disk.qcow2
```

## ⚡ Performance Tips

1. **Use SSD**: Store disk images on SSD for faster inspection
2. **Verbose mode**: Only use when debugging (adds overhead)
3. **Local files**: Inspect local files rather than network-mounted
4. **Read-only**: `inspect`, `doctor` and the other report commands mount the guest read-only. Guest filesystems are always mounted `nodev,nosuid`.

## 🐛 Troubleshooting

| Issue | Solution |
|-------|----------|
| "No OS found" | Check if disk has a bootable OS partition |
| "Permission denied" | Run with appropriate permissions or use sudo |
| "Unsupported format" | Check if disk format is supported (QCOW2, RAW, etc.) |
| Missing info | Some info requires OS-specific files; may not exist |
| Slow performance | Check disk I/O, use SSD, ensure enough memory |

## 🎓 Learn More

- Command reference: [cli-guide.md](cli-guide.md)
- Rust API reference: `cargo doc --open`

## ✨ What `inspect` covers

The inspection report includes:
- ✅ Network configuration analysis
- ✅ User account enumeration
- ✅ SSH configuration inspection
- ✅ SELinux status
- ✅ Language runtime detection
- ✅ Container runtime detection
- ✅ LVM analysis
- ✅ Boot configuration
- ✅ Scheduled tasks (cron, timers)
- ✅ SSL certificate discovery
- ✅ Kernel parameter inspection
- ✅ VM tools detection
- ✅ Cloud-init detection
- ✅ Comprehensive verbose logging

Happy inspecting! 🎉
