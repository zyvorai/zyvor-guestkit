# GuestKit Python Examples

This directory contains example scripts demonstrating the GuestKit Python bindings for virtual machine disk image inspection and manipulation.

## Prerequisites

### System Requirements

- Python 3.10 or later
- GuestKit with Python bindings built
- QEMU tools (qemu-img, qemu-nbd)
- Root/sudo access (required for most operations)

### Installation

**Build GuestKit with Python bindings:**
```bash
# Clone and build
git clone https://github.com/zyvorai/guestkit
cd guestkit
cargo build --release --features python

# Install Python module
pip install .
```

**Install system dependencies:**
```bash
# Ubuntu/Debian
sudo apt-get install qemu-utils

# Fedora/RHEL/CentOS
sudo dnf install qemu-img

# Arch Linux
sudo pacman -S qemu
```

## Examples Overview

### 1. Basic Inspection (`basic_inspection.py`)

**Purpose:** Demonstrates basic OS detection and information gathering.

**What it does:**
- Detects operating systems in a disk image
- Displays OS type, distribution, version
- Shows hostname and architecture
- Lists mountpoints

**Usage:**
```bash
sudo python3 basic_inspection.py <disk-image>
```

**Example:**
```bash
sudo python3 basic_inspection.py /path/to/ubuntu.qcow2
```

**Output:**
```
=== OS #1 ===
Root device: /dev/sda2
Type: linux
Distribution: ubuntu
Version: 22.4
Hostname: ubuntu-server
Architecture: x86_64
```

**Concepts demonstrated:**
- Creating GuestFS handle
- Adding disk images read-only
- Launching the appliance
- OS inspection API
- Proper cleanup

---

### 2. List Filesystems (`list_filesystems.py`)

**Purpose:** Enumerate all storage devices, partitions, and filesystems.

**What it does:**
- Lists all block devices with sizes
- Shows partitions with filesystem types
- Displays filesystem labels and UUIDs
- Reports LVM information (volume groups, logical volumes)

**Usage:**
```bash
sudo python3 list_filesystems.py <disk-image>
```

**Example:**
```bash
sudo python3 list_filesystems.py /path/to/fedora.qcow2
```

**Output:**
```
--- Devices ---
Device: /dev/sda
  Size: 20.00 GB

--- Partitions ---
Partition: /dev/sda1
  Size: 512.00 MB
  Filesystem: vfat
  Label: EFI

Partition: /dev/sda2
  Size: 19.48 GB
  Filesystem: ext4
  Label: rootfs
  UUID: a1b2c3d4-...

--- LVM Information ---
Volume Groups: vg0
Logical Volumes: /dev/vg0/root, /dev/vg0/home
```

**Concepts demonstrated:**
- Device enumeration
- Partition inspection
- Filesystem detection
- Label and UUID retrieval
- LVM handling
- Human-readable size formatting

---

### 3. Mount and Explore (`mount_and_explore.py`)

**Purpose:** Mount filesystems and navigate directory structure.

**What it does:**
- Automatically mounts all filesystems from detected OS
- Explores root directory structure
- Reads and displays system files (os-release, hostname, fstab, hosts)
- Shows disk usage statistics

**Usage:**
```bash
sudo python3 mount_and_explore.py <disk-image>
```

**Example:**
```bash
sudo python3 mount_and_explore.py /path/to/debian.img
```

**Output:**
```
[1/5] Detecting operating systems...
Root device: /dev/sda1

[2/5] Mounting filesystems...
  Mounted /dev/sda1 at /
  Mounted /dev/sda2 at /home

[3/5] Exploring root filesystem...
📁 bin/
📁 boot/
📁 etc/
📁 home/
...

--- OS Release Information (/etc/os-release) ---
NAME="Debian GNU/Linux"
VERSION="12 (bookworm)"
...

[5/5] Disk usage information...
Total space: 18.50 GB
Used space:  5.23 GB
Free space:  13.27 GB
Used:        28.3%
```

**Concepts demonstrated:**
- Mounting filesystems
- Reading mountpoint information
- Directory traversal
- File reading
- File type detection (files, directories, symlinks)
- Disk usage statistics
- Proper mount ordering

---

### 4. Package Inspection (`package_inspection.py`)

**Purpose:** List and analyze installed software packages.

**What it does:**
- Detects package management system (dpkg, RPM, pacman)
- Lists installed packages with versions
- Shows package statistics by category
- Supports Debian/Ubuntu, Red Hat/Fedora, and Arch Linux

**Usage:**
```bash
sudo python3 package_inspection.py <disk-image>
```

**Example:**
```bash
sudo python3 package_inspection.py /path/to/ubuntu.qcow2
```

**Output:**
```
=== Debian/Ubuntu Package Inspection (dpkg) ===

Found 1847 installed packages

Package Name                     Version                 Description
------------------------------------------------------------------------------------------------
accountsservice                  0.6.55-0ubuntu12       query and manipulate user account
adduser                          3.118ubuntu2           add and remove users and groups
...

--- Package Statistics ---
Total packages: 1847
Library packages (lib*): 623
Python-related packages: 142
Kernel packages: 5
```

**Concepts demonstrated:**
- Package detection using inspect_list_applications
- Distribution-specific package handling
- Package database parsing
- Data aggregation and statistics

---

### 5. Create Disk Image (`create_disk.py`)

**Purpose:** Create new disk images with partitions and filesystems from scratch.

**What it does:**
- Creates empty disk image of specified size
- Sets up GPT partition table
- Creates EFI and root partitions
- Formats with appropriate filesystems (VFAT, ext4)
- Creates basic directory structure
- Writes configuration files (fstab, hostname)

**Basic mode:**
```bash
sudo python3 create_disk.py /tmp/basic-disk.img 1024
```

**Advanced mode (BTRFS with subvolumes):**
```bash
sudo python3 create_disk.py /tmp/advanced-disk.img 2048 --advanced
```

**Output:**
```
=== Creating Basic Disk Image ===
Output: /tmp/basic-disk.img
Size: 1024 MB

[1/7] Creating empty disk image...
✓ Created /tmp/basic-disk.img

[2/7] Adding disk and launching appliance...
✓ Appliance launched

[3/7] Creating GPT partition table...
✓ Created GPT partition table on /dev/sda

[4/7] Creating partitions...
✓ Created EFI partition: /dev/sda1
✓ Created root partition: /dev/sda2

[5/7] Creating filesystems...
✓ Created VFAT filesystem on /dev/sda1 with label 'EFI'
✓ Created ext4 filesystem on /dev/sda2 with label 'rootfs'

[6/7] Creating directory structure...
  Created /etc
  Created /home
  ...

[7/7] Finalizing...

✓ Disk image created successfully: /tmp/basic-disk.img
  Size: 1024.00 MB
```

**Concepts demonstrated:**
- Disk creation (disk_create)
- Partition table initialization
- Partition creation with specific sizes
- GPT partition type GUIDs
- Filesystem creation
- Filesystem labeling
- Directory creation
- File writing
- fstab generation

**Advanced mode features:**
- BTRFS filesystem
- Swap partition
- BTRFS subvolumes (@, @home, @var, @snapshots)
- Mount options (compression, subvolume selection)

---

## Running the Examples

### Basic Workflow

1. **Prepare a disk image:**
   ```bash
   # Use existing VM image
   ls /var/lib/libvirt/images/*.qcow2

   # Or download a cloud image
   wget https://cloud-images.ubuntu.com/releases/22.04/release/ubuntu-22.04-server-cloudimg-amd64.img
   ```

2. **Run an example with sudo:**
   ```bash
   sudo python3 basic_inspection.py ubuntu-22.04-server-cloudimg-amd64.img
   ```

3. **Create a test disk:**
   ```bash
   sudo python3 create_disk.py /tmp/test-disk.img 1024
   ```

4. **Inspect the test disk:**
   ```bash
   sudo python3 list_filesystems.py /tmp/test-disk.img
   ```

### Common Issues

**Problem:** `ModuleNotFoundError: No module named 'guestkit'`

**Solution:** Install Python bindings:
```bash
sudo apt-get install python3-guestfs  # Ubuntu/Debian
sudo dnf install python3-GuestKit   # Fedora/RHEL
```

**Problem:** `RuntimeError: permission denied`

**Solution:** Run with sudo:
```bash
sudo python3 script.py image.img
```

**Problem:** `RuntimeError: appliance failed to start`

**Solutions:**
1. Check GuestKit is installed
2. Ensure virtualization (KVM) is available
3. Check permissions on disk image file

## Learning Path

### Beginner

1. Start with `basic_inspection.py` - Understand the basic workflow
2. Try `list_filesystems.py` - Learn about device/partition enumeration
3. Run `mount_and_explore.py` - See how to read files

### Intermediate

4. Study `package_inspection.py` - Distribution-specific operations
5. Experiment with `create_disk.py` - Disk creation basics
6. Modify examples to suit your needs

### Advanced

7. Combine multiple examples into custom tools
8. Read the full API documentation: [docs/user-guides/python-bindings.md](../../docs/user-guides/python-bindings.md)
9. Study the Rust ergonomic API for type-safe patterns

## Code Patterns

### Error Handling

All examples use try-except for robust error handling:

```python
try:
    hostname = g.inspect_get_hostname(root)
    print(f"Hostname: {hostname}")
except Exception as e:
    print(f"Could not get hostname: {e}")
```

### Resource Cleanup

Proper cleanup to avoid resource leaks:

```python
# Always cleanup
g.umount_all()
g.shutdown()

# Or use context manager
with Guestfs() as g:
    # ... operations ...
    pass  # Automatic cleanup
```

### Defensive Programming

Check before operating:

```python
if g.is_file(path):
    content = g.read_file(path)
elif g.is_dir(path):
    entries = g.ls(path)
```

## Additional Resources

- **Python API:** [docs/user-guides/python-bindings.md](../../docs/user-guides/python-bindings.md)
- **VM migration:** [docs/user-guides/vm-migration.md](../../docs/user-guides/vm-migration.md)
- **Documentation index:** [docs/INDEX.md](../../docs/INDEX.md)
- **Main README:** `README.md`
- **GuestKit Documentation:** https://GuestKit.org/

## Quick Reference

### Essential Operations

```python
from guestkit import Guestfs

# Create and launch
g = Guestfs()
g.add_drive_ro("/path/to/disk.img")
g.launch()

# Inspect OS
roots = g.inspect_os()
root = roots[0]

# Get info
os_type = g.inspect_get_type(root)
distro = g.inspect_get_distro(root)

# Mount
mountpoints = g.inspect_get_mountpoints(root)
for mount_path, device in sorted(mountpoints, key=lambda x: len(x[0])):
    g.mount_ro(device, mount_path)

# Read file
if g.is_file("/etc/hostname"):
    content = g.read_file("/etc/hostname")
    print(content.decode('utf-8'))

# List packages
apps = g.inspect_list_applications(root)
for app in apps:
    print(f"{app['app_name']} {app['app_version']}")

# Cleanup
g.umount_all()
g.shutdown()
```

## Contributing

Found a bug or have an improvement? Please open an issue or pull request on GitHub!

---

**Happy VM disk image hacking! 🎉**
