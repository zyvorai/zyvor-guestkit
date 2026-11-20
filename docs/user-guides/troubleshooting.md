# Troubleshooting Guide

This guide helps you diagnose and resolve common issues with guestkit.

## Table of Contents

- [Installation Issues](#installation-issues)
- [Runtime Errors](#runtime-errors)
- [Performance Issues](#performance-issues)
- [Integration Issues](#integration-issues)
- [Common Error Messages](#common-error-messages)
- [Debugging Tips](#debugging-tips)
- [Getting Help](#getting-help)

---

## Installation Issues

### Compilation Fails with Missing Dependencies

**Symptoms:**
```
error: linking with `cc` failed
could not find qemu-img
```

**Solution:**
Install system dependencies:

```bash
# Fedora/RHEL
sudo dnf install qemu-img cryptsetup lvm2

# Ubuntu/Debian
sudo apt-get install qemu-utils cryptsetup lvm2

# macOS (Homebrew)
brew install qemu
```

### Cargo Build Fails with Disk Quota Error

**Symptoms:**
```
error: failed to write: Disk quota exceeded (os error 122)
```

**Solution:**
Clean up target directory:

```bash
cargo clean
rm -rf target/
# Free up disk space, then rebuild
cargo build
```

### Rust Version Too Old

**Symptoms:**
```
error: package requires rustc X.Y or newer
```

The repository does not pin a `rust-version`; dependencies set the floor, so build with a current stable toolchain.

**Solution:**
Update Rust:

```bash
rustup update stable
rustc --version  # Use the current stable release
```

---

## Runtime Errors

### "Failed to launch" or "Not ready" Errors

**Symptoms:**
```
Error: NotReady("Must call launch() first")
```

**Solution:**
Ensure you call `launch()` after adding drives:

```rust
let mut g = Guestfs::new()?;
g.add_drive_ro("/path/to/disk.img")?;
g.launch()?;  // ← Required before operations
```

### Loop Device Issues

**Symptoms:**
```
Error: losetup failed: permission denied
Error: No free loop devices
```

**Solutions:**

1. **Permission denied** - Run with sudo:
   ```bash
   sudo guestkit inspect disk.raw
   ```

2. **No free loop devices** - Check and clean up:
   ```bash
   # Check current loop devices
   losetup -a

   # Disconnect all unused
   sudo losetup -D

   # Or increase max loop devices
   sudo modprobe loop max_loop=16
   ```

3. **losetup not found** - Install util-linux:
   ```bash
   # Fedora/RHEL
   sudo dnf install util-linux

   # Ubuntu/Debian
   sudo apt-get install util-linux
   ```

4. **"Loop device ... did not become ready in time" (before v1.2.10)** - On Ubuntu 26.04 (kernel 7.x) devices attached through `sudo` are `root:disk` mode `0660`, so the unprivileged readiness check failed. Upgrade to v1.2.10, which reads readiness from sysfs. Reading the device afterwards still needs root, membership of the `disk` group, or a udev rule; see [Device-node permissions](../prerequisites-and-build.md#device-node-permissions-ubuntu-2604-and-other-recent-kernels).

**Note:** Loop devices are used by default for RAW/IMG/ISO files. They're built into the Linux kernel and should always be available. On Ubuntu 26.04 the `/dev/loopN` and `/dev/nbdN` nodes are created on demand, so do not rely on `chmod` of the nodes that exist at boot.

### NBD Mount Fails

**Symptoms:**
```
Error: Failed to mount NBD device
Error: No available NBD devices found
```

**Solution:**

guestkit automatically loads the NBD module, but if that fails:

```bash
# Manual module load
sudo modprobe nbd max_part=16

# Make it persistent
echo "nbd" | sudo tee /etc/modules-load.d/nbd.conf

# Verify
lsmod | grep nbd
```

**Clean up stuck NBD devices:**
```bash
# Disconnect all NBD devices
for i in {0..15}; do
    sudo qemu-nbd --disconnect /dev/nbd$i 2>/dev/null
done

# If still stuck, reboot
sudo reboot
```

**Install qemu-nbd if missing:**
```bash
# Fedora/RHEL
sudo dnf install qemu-img

# Ubuntu/Debian
sudo apt-get install qemu-utils
```

**Note:** NBD is used for QCOW2/VMDK/VDI/VHD/VHDX formats (detected from the file header, not the extension). Use RAW format for simpler setup. NBD needs `qemu-nbd` (`qemu-utils` on Debian/Ubuntu) and the `nbd` kernel module; the same device-node permission rules as for loop devices apply (see [Device-node permissions](../prerequisites-and-build.md#device-node-permissions-ubuntu-2604-and-other-recent-kernels)).

### Permission Denied Errors

**Symptoms:**
```
Error: Permission denied (os error 13)
Error: Operation not permitted
```

**Solutions:**

1. **For NBD operations** - Run with appropriate privileges:
   ```bash
   sudo guestkit inspect disk.img
   ```

2. **For LUKS operations** - Requires root or appropriate capabilities:
   ```bash
   sudo guestkit ...
   # OR use capabilities
   sudo setcap cap_sys_admin+ep target/release/guestkit
   ```

3. **For disk image access** - Check file permissions:
   ```bash
   chmod 644 disk.img
   # Or make readable by your user
   sudo chown $USER:$USER disk.img
   ```

### LUKS Operations Fail

**Symptoms:**
```
Error: cryptsetup command failed
Error: LUKS device not found
```

**Solutions:**

1. Ensure cryptsetup is installed:
   ```bash
   which cryptsetup
   sudo dnf install cryptsetup  # or apt-get
   ```

2. Verify LUKS header:
   ```bash
   sudo cryptsetup luksDump /dev/sda1
   ```

3. Check device mapper:
   ```bash
   ls -la /dev/mapper/
   ```

### LVM Operations Fail

**Symptoms:**
```
Error: Volume group not found
Error: Logical volume not found
```

**Solutions:**

1. Scan for volume groups:
   ```rust
   g.vgscan()?;
   g.vg_activate_all(true)?;
   ```

2. Check LVM tools are installed:
   ```bash
   which vgscan lvscan pvs
   sudo dnf install lvm2
   ```

3. Verify volume groups:
   ```bash
   sudo vgscan
   sudo vgs
   sudo lvs
   ```

### File Not Found in Guest

**Symptoms:**
```
Error: exists: No such file or directory
```

**Solutions:**

1. Verify filesystem is mounted:
   ```rust
   let mounts = g.mounts()?;
   println!("Mounted: {:?}", mounts);
   ```

2. List available files:
   ```rust
   let files = g.ls("/")?;
   println!("Available: {:?}", files);
   ```

3. Check path is absolute:
   ```rust
   // Good
   g.cat("/etc/passwd")?;

   // Bad
   g.cat("etc/passwd")?;  // Missing leading /
   ```

### Errors Added by the v1.2.11 Hardening

These are deliberate refusals. Each one closes a way for a hostile disk image or job to reach your host; see the [1.2.11 changelog](../development/CHANGELOG.md).

| Message or symptom | Cause | What to do |
|--------------------|-------|------------|
| `Failed to acquire NBD allocation lock: ...` | The NBD allocation lock is only created in `/run/lock/guestkit-nbd.lock`, then `$XDG_RUNTIME_DIR` or `/run/user/<uid>`; it is never placed in `/tmp`. It is opened with `O_NOFOLLOW`, mode 0600, and must be a regular file owned by the current user. | Run through `sudo` (root can use `/run/lock`), or make sure `XDG_RUNTIME_DIR` / `/run/user/<uid>` exists for your user. Remove a stale or foreign `guestkit-nbd.lock` that is a symlink or belongs to another user. |
| `lock file is not a regular file owned by the current user` | The lock path already exists as a symlink, directory or another user's file | Delete it (as root) and retry |
| `Failed to read <path>: not a regular file` | `cat` / `read_file` refuse FIFOs, device nodes and directories in the guest | Use `list` for directories; device nodes are not readable from an image on purpose |
| `Failed to read <path>: file is N bytes, limit is 1073741824` | `cat` / `read_file` read at most 1 GiB | Copy the file to the host with `extract` instead of loading it into memory |
| `Invalid mountpoint (must not contain '..')` | A guest mount point with a `..` component is rejected | Use a mount point inside the guest root, for example `/boot/efi` |
| Device nodes or setuid binaries in a mounted guest do not work | Every host mount of a guest filesystem adds `nodev,nosuid` (appended last, so a caller-supplied `dev` or `suid` is overridden) | Expected. To work with a device node, use the guest itself, not a host mount |
| `unsupported image.format 'x'` / `output would overwrite the source image` (worker `convert` job) | Worker convert allows only `raw`, `qcow2`, `qcow`, `vmdk`, `vdi`, `vhd`, `vpc`, `vhdx`, `qed` and `parallels`, and an output path equal to the source is refused | Pick a listed format and a different output path |
| Job rejected with `job_id` invalid / `unsafe job id` (worker) | Job ids must be 8 to 128 characters of letters, digits, `-`, `_` and `.`, must not start with `.` and must not contain `..`, because they become file names in the spool and result directories | Use a UUID or `job-<digits>` style id |
| A `grep`, `egrep` or `fgrep` pattern that starts with `-` now matches that text | Since v1.2.11 the pattern is passed after `-e` and `--`, so it can no longer be read as a `grep` option (for example `-f <host file>`) | Nothing to do; this is the fix |
| Log line `Primary cache directory unavailable ..., falling back to /tmp/guestkit-<uid>/binary` | The binary cache directory (`~/.cache/guestkit/binary`) is created owner-only (0700). If that fails, the fallback is per-user and is refused when it is a symlink or owned by someone else, and cached data does not survive a reboot | Fix the ownership or mode of `~/.cache/guestkit`, or remove a planted `/tmp/guestkit-<uid>` |

---

## Performance Issues

### Slow Launch Time

**Symptoms:**
- `launch()` takes many seconds
- Startup is slower than expected

**Solutions:**

1. **Reduce disk image size** - Large images take longer to analyze:
   ```bash
   # Use qcow2 instead of raw for better performance
   guestkit convert disk.raw --output disk.qcow2 --format qcow2
   ```

2. **Use read-only mode** when possible:
   ```rust
   g.add_drive_ro(path)?;  // Faster than read-write
   ```

3. **Disable verbose logging** in production:
   ```rust
   g.set_verbose(false);  // Default
   ```

### Slow File Operations

**Symptoms:**
- File reads/writes are slow
- Archive operations take too long

**Solutions:**

1. **Use bulk operations** instead of many small operations:
   ```rust
   // Good - one tar operation
   g.tar_out("/data", "/backup.tar")?;

   // Bad - many individual file copies
   for file in files {
       g.cp(&file, &dest)?;  // Slow!
   }
   ```

2. **Minimize mount/unmount cycles**:
   ```rust
   // Good - mount once, do all operations
   g.mount("/dev/sda1", "/")?;
   g.cat("/file1")?;
   g.cat("/file2")?;
   g.umount("/")?;

   // Bad - mount/unmount for each operation
   g.mount(...)?; g.cat("/file1")?; g.umount(...)?;
   g.mount(...)?; g.cat("/file2")?; g.umount(...)?;
   ```

3. **Use compressed formats** for archives:
   ```rust
   g.tar_out_opts("/data", "/backup.tar.gz", Some("gzip"))?;
   ```

### High Memory Usage

**Symptoms:**
- Process uses excessive memory
- Out of memory errors

**Solutions:**

1. **Process files in chunks** instead of reading entirely:
   ```rust
   // For large files, use streaming or download
   g.download("/large-file", "/tmp/output")?;
   ```

2. **Clean up between operations**:
   ```rust
   g.umount_all()?;
   g.shutdown()?;
   // Start fresh for next image
   let mut g = Guestfs::new()?;
   ```

3. **Limit concurrent operations** in loops:
   ```rust
   for disk in disks.chunks(5) {  // Process 5 at a time
       // Process batch
   }
   ```

---

## Integration Issues

### Works Standalone but Fails in Production

**Checklist:**
- [ ] Are all system dependencies installed on production?
- [ ] Is NBD kernel module loaded?
- [ ] Are there sufficient permissions?
- [ ] Is there enough disk space for temporary files?
- [ ] Are ulimits appropriate (open files, processes)?

**Verify environment:**
```bash
# Check dependencies
which qemu-img cryptsetup
lsmod | grep nbd

# Check disk space
df -h /tmp

# Check limits
ulimit -a
```

### Integration with Docker/Podman

**Issue:** NBD doesn't work in containers

**Solution:** Run with privileged mode and device access:

```bash
podman run --privileged \
  --device /dev/nbd0 \
  -v /path/to/images:/images:ro \
  guestkit inspect /images/vm.qcow2
```

**Better:** Use container with pre-loaded NBD module:

```dockerfile
FROM fedora:latest
RUN dnf install -y qemu-img
# Load NBD module on host before running container
COPY guestkit /usr/local/bin/
```

### Integration with Kubernetes

**Issue:** Pods don't have NBD access

**Solution:** Use privileged pods or node-level NBD:

```yaml
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: guestkit
    securityContext:
      privileged: true
    volumeMounts:
    - name: dev-nbd
      mountPath: /dev/nbd0
  volumes:
  - name: dev-nbd
    hostPath:
      path: /dev/nbd0
```

---

## Common Error Messages

### "Command failed: qemu-img"

**Cause:** qemu-img not installed or not in PATH

**Solution:**
```bash
# Install qemu-img
sudo dnf install qemu-img

# Verify
which qemu-img
qemu-img --version
```

### "Device or resource busy"

**Cause:** NBD device still in use

**Solution:**
```bash
# Disconnect NBD devices
sudo qemu-nbd --disconnect /dev/nbd0
sudo qemu-nbd --disconnect /dev/nbd1

# Verify
cat /proc/partitions | grep nbd
```

### "Invalid argument"

**Cause:** Incorrect parameters or corrupted disk image

**Solutions:**
1. Verify disk image is valid:
   ```bash
   qemu-img check disk.qcow2
   qemu-img info disk.qcow2
   ```

2. Check parameter types:
   ```rust
   // Ensure paths are strings, not PathBuf
   g.add_drive_ro(path.to_str().unwrap())?;
   ```

### "Filesystem check failed"

**Cause:** Filesystem errors on disk image

**Solution:**
```bash
# Manual fsck
sudo qemu-nbd -c /dev/nbd0 disk.qcow2
sudo fsck /dev/nbd0p1
sudo qemu-nbd -d /dev/nbd0
```

---

## Debugging Tips

### Enable Verbose Logging

```rust
let mut g = Guestfs::new()?;
g.set_verbose(true);
g.set_trace(true);
```

```bash
# Environment variable
export RUST_LOG=debug
guestkit inspect disk.img
```

### Check What's Happening

```rust
// List all mounts
let mounts = g.mounts()?;
eprintln!("Currently mounted: {:?}", mounts);

// List all devices
let devices = g.list_devices()?;
eprintln!("Available devices: {:?}", devices);

// Get detailed stats
let stat = g.stat("/etc/passwd")?;
eprintln!("File stat: {:?}", stat);
```

### Trace System Calls

```bash
# Use strace to see system calls
strace -e trace=open,stat,mount guestkit inspect disk.img 2>&1 | grep -v ENOENT

# Monitor qemu-nbd activity
ps aux | grep qemu-nbd
sudo lsof | grep nbd
```

### Check Disk Image Integrity

```bash
# Check qcow2 image
qemu-img check -r all disk.qcow2

# Get detailed info
qemu-img info --backing-chain disk.qcow2

# Check for corruption
qemu-img dd if=disk.qcow2 of=/dev/null bs=1M
```

### Test in Isolation

Create a minimal test case:

```rust
use guestkit::Guestfs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;

    // Minimal test disk
    g.disk_create("/tmp/test.img", "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro("/tmp/test.img")?;
    g.launch()?;

    println!("Launch successful!");

    g.shutdown()?;
    Ok(())
}
```

---

## Getting Help

### Before Asking for Help

Gather this information:

1. **Environment:**
   - OS and version: `uname -a`
   - Rust version: `rustc --version`
   - guestkit version: `guestkit version`

2. **Error details:**
   - Full error message
   - Stack trace if available
   - Minimal reproduction case

3. **System state:**
   - Loaded modules: `lsmod | grep nbd`
   - Available devices: `ls -la /dev/nbd*`
   - Disk image info: `qemu-img info disk.img`

### Where to Get Help

1. **GitHub Issues:** https://github.com/zyvorai/guestkit/issues
   - Search existing issues first
   - Include environment info and error messages
   - Provide minimal reproduction case

2. **GitHub Discussions:** For questions and troubleshooting

3. **Documentation:**
   - [Contributing Guide](../development/CONTRIBUTING.md)
   - [Examples](../../examples/)

### Reporting Bugs

Use this template:

```markdown
**Environment:**
- OS: Fedora 39
- Rust: 1.75.0
- guestkit: 0.2.0

**Description:**
Brief description of the issue

**Steps to Reproduce:**
1. Command or code that triggers the issue
2. Expected behavior
3. Actual behavior

**Error Message:**
```
Full error message here
```

**Additional Context:**
Any other relevant information
```

---

## Frequently Asked Questions

### Q: Do I need root/sudo to use guestkit?

**A:** It depends:
- **Read-only inspection:** Usually no, unless NBD requires it
- **NBD mounting:** Often yes, or use capabilities
- **LUKS operations:** Yes, requires root
- **LVM operations:** Yes, requires root

### Q: Can I use guestkit in Docker/containers?

**A:** Yes, but requires privileged mode or device access for NBD operations.

### Q: Does guestkit work on Windows/macOS?

**A:** Currently Linux-only. Windows/macOS support is planned for future phases.

### Q: How do I process multiple disk images efficiently?

**A:** Process in batches and reuse the Guestfs handle when possible:

```rust
for disk in disks.chunks(10) {
    for d in disk {
        process_disk(d)?;
    }
    // Brief pause between batches
    std::thread::sleep(Duration::from_millis(100));
}
```

### Q: What's the difference between guestkit and ?

**A:** guestkit is a pure Rust implementation inspired by :
- **Pros:** Memory safe, no C dependencies, better integration with Rust projects
- **Cons:** Not 100% API compatible, some features still in development
- **Coverage:** 76.8% of  APIs implemented

---

## Additional Resources

- [Security Policy](../../SECURITY.md)
- [Roadmap](../development/roadmap.md)
- [Changelog](../development/CHANGELOG.md)
