# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.x     | :white_check_mark: |
| < 1.0   | :x:                |

## Security Considerations

### General Security

GuestKit is designed for disk image inspection and manipulation. When using this tool:

1. **Untrusted Disk Images**: Exercise caution when processing disk images from untrusted sources
   - Malicious disk images could potentially exploit vulnerabilities in parsing code
   - Run GuestKit in isolated environments when processing untrusted images
   - Consider using containers or VMs for additional isolation

2. **Privilege Requirements**: Some operations require elevated privileges
   - NBD mounting may require root or appropriate capabilities
   - LUKS operations require access to /dev/mapper
   - LVM operations require access to device mapper

3. **Data Integrity**: Always work on copies of important data
   - GuestKit can modify disk images
   - Use read-only mode when inspection is the only goal
   - Keep backups of critical data

### Known Security Boundaries

#### Safe Operations (Read-Only Mode)

When using `add_drive_ro()`, the following operations are safe:
- Disk format detection
- Partition table reading
- Filesystem inspection
- OS detection
- File reading (when mounted read-only)
- Package listing
- Configuration inspection

#### Potentially Dangerous Operations

The following operations modify data and should be used carefully:
- `write()`, `write_append()` - Modify files
- `mkfs()` - Destroys existing filesystem data
- `part_del()` - Deletes partitions
- `luks_format()` - Destroys existing encryption
- `lvcreate()`, `lvremove()` - Modifies LVM configuration
- `dd()`, `zero_device()` - Overwrites device data

### Input Validation

GuestKit performs basic input validation but relies on underlying tools for detailed validation:

1. **Path Traversal**: Since v1.2.11 guest paths are resolved chroot-style inside the guest root (see [Untrusted disk images](#untrusted-disk-images-v1211)). Absolute guest symlinks restart at the guest root, `..` never climbs above it, and `mount` rejects `..` in mount points. Callers still decide which guest paths they are willing to touch
2. **Device Names**: Device names are passed to system commands - validate externally if from untrusted sources
3. **Command Injection**: Arguments to guest commands should be sanitized by callers

### Encryption

#### LUKS Support

- Passwords/keys are passed as command-line arguments to cryptsetup
  - Keys may be visible in process lists temporarily
  - Consider using key files instead of passwords in production
- LUKS operations require appropriate privileges

#### Security Best Practices

```rust
// DO: Use read-only mode for inspection
g.add_drive_ro("/path/to/untrusted.img")?;

// DON'T: Write mode on untrusted images
g.add_drive_opts("/path/to/untrusted.img", None, false)?; // risky!

// DO: Validate paths against your own policy before use. GuestKit confines
// guest paths to the guest root (v1.2.11+) but cannot know which files your
// application should be allowed to read or write.
let path = user_input.trim();
if !path.starts_with("/") {
    return Err("Invalid path".into());
}

// DO: Use timeouts for operations
use std::time::Duration;
// Future: g.set_timeout(Duration::from_secs(30))?;
```

## Reporting a Vulnerability

### Where to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please send a report to:
- Email: ssahani@gmail.com
- Subject: "[SECURITY] GuestKit Vulnerability Report"

### What to Include

Please include the following information:

1. **Description**: Brief description of the vulnerability
2. **Impact**: Potential impact and severity assessment
3. **Reproduction**: Step-by-step instructions to reproduce
4. **Affected Versions**: Which versions are affected
5. **Suggested Fix**: If you have one (optional)
6. **Disclosure Timeline**: Your expected disclosure timeline

### Example Report

```
Subject: [SECURITY] GuestKit Vulnerability Report

Description:
Command injection vulnerability in xyz() function when processing
untrusted device names.

Impact:
An attacker with control over device names could execute arbitrary
commands with the privileges of the GuestKit process.

Affected Versions:
0.2.0 and earlier

Reproduction:
1. Create malicious device name: `"; rm -rf / #"`
2. Call g.some_function() with this device name
3. Observe command execution

Suggested Fix:
Add shell escaping for device names before passing to Command::new()

Disclosure Timeline:
Requesting 90 days for fix before public disclosure
```

### Response Timeline

- **Initial Response**: Within 48 hours
- **Assessment**: Within 1 week
- **Fix Development**: Depends on severity
  - Critical: 7 days
  - High: 14 days
  - Medium: 30 days
  - Low: 60 days
- **Public Disclosure**: After fix is released

### Disclosure Policy

We follow a coordinated disclosure process:

1. **Private Report**: Vulnerability reported privately
2. **Acknowledgment**: We acknowledge receipt within 48 hours
3. **Investigation**: We assess severity and impact
4. **Fix Development**: We develop and test a fix
5. **Release**: We release a patched version
6. **Public Disclosure**: We publish security advisory
7. **Credit**: We credit the reporter (if desired)

### Security Advisories

Published security advisories will be available at:
- GitHub Security Advisories
- CHANGELOG.md with [SECURITY] tags
- Release notes

## Security Features

### Untrusted disk images (v1.2.11)

Release 1.2.11 closed several ways a hostile image could reach the host. Details are in the [changelog](CHANGELOG.md#1211---2026-11-12) and the [CLI reference](docs/user-guides/cli-guide.md#safety-behaviour-you-should-know-about).

- Guest filesystems are mounted `nodev,nosuid`, so planted device nodes cannot open host devices.
- Guest paths resolve chroot-style inside the guest root; `..` in mount points is rejected; tar/archive directories are confined to the guest root; `cpio` extraction refuses absolute and `..` member names.
- `read_file` / `cat` read at most 1 GiB and refuse FIFOs, device nodes and directories.
- `grep`, `egrep` and `fgrep` pass the pattern after `-e` and `--`, so a pattern beginning with `-` is not a `grep` option.
- Image paths are made absolute before `losetup`, `qemu-nbd` and `qemu-img` see them.
- The NBD allocation lock lives in `/run/lock` or the per-user runtime directory (never `/tmp`), opened with `O_NOFOLLOW` and mode 0600; the binary cache directory is owner-only.
- Worker job ids must be a single safe path component (`[A-Za-z0-9._-]`, 8 to 128 characters, no leading `.`).
- GPT headers with overflowing fields and malformed `/proc/mdstat` lines return errors instead of panicking.

Still true: `guestkit execute` runs the guest's own binaries via `chroot` on the host as root, and mounting any image needs root. Process images you do not trust inside a container or VM.

### Memory Safety

- Written in pure Rust (except external tool calls)
- `unsafe` is limited to libc/FFI calls (for example `geteuid`, `flock`, libhivex, journal and vsock bindings) rather than parsing code
- Bounds checking on all array/slice access
- Protection against common memory errors (use-after-free, double-free, buffer overflows)

### Command Execution Safety

GuestKit executes external commands for some operations:
- Uses `std::process::Command` for safe command execution
- Arguments are properly separated (no shell parsing)
- Output is captured and parsed safely

### Privilege Separation

- Core library runs with user privileges
- External tools (qemu-nbd, cryptsetup, lvm) may require elevation
- Users should apply principle of least privilege

## Secure Usage Guidelines

### For End Users

1. **Verify Checksums**: Verify package checksums before installation
   ```bash
   sha256sum target/release/guestkit
   ```

2. **Use Read-Only Mode**: When possible, use read-only mode
   ```rust
   g.add_drive_ro(disk_path)?;
   ```

3. **Validate Input**: Always validate untrusted input
   ```rust
   fn safe_read_file(g: &mut Guestfs, path: &str) -> Result<String> {
       // Validate path
       if !path.starts_with("/") || path.contains("..") {
           return Err("Invalid path".into());
       }
       g.cat(path)
   }
   ```

4. **Limit Privileges**: Run with minimum required privileges
   ```bash
   # Use capabilities instead of full root
   sudo setcap cap_sys_admin+ep target/release/guestkit
   ```

5. **Isolate Workloads**: Process untrusted images in containers
   ```bash
   podman run --rm -it \
       --device /dev/nbd0 \
       -v /path/to/images:/images:ro \
       guestkit inspect /images/untrusted.qcow2
   ```

### For Developers

1. **Input Sanitization**: Always sanitize user input
   ```rust
   // Bad
   g.command(&[&format!("cat {}", user_input)])?;

   // Good
   g.cat(&validate_path(user_input)?)?;
   ```

2. **Error Messages**: Don't leak sensitive information in errors
   ```rust
   // Bad
   Err(format!("Failed with key: {}", secret_key))

   // Good
   Err("Authentication failed".into())
   ```

3. **Resource Limits**: Set appropriate resource limits
   ```rust
   // Future API
   g.set_memory_limit(512 * 1024 * 1024)?; // 512MB
   g.set_timeout(Duration::from_secs(300))?; // 5 minutes
   ```

4. **Audit Logging**: Log security-relevant operations
   ```rust
   log::info!("Opening LUKS device: {}", device);
   g.luks_open(device, key, mapname)?;
   ```

## Dependencies

GuestKit relies on several external tools and libraries. Security of these dependencies:

### System Tools
- **qemu-img**: Disk format conversion and inspection
- **qemu-nbd**: NBD server for mounting
- **cryptsetup**: LUKS encryption
- **lvm2**: Logical volume management

Users should keep these tools updated via their package manager.

### Rust Crates

Dependencies are regularly audited using:
```bash
cargo audit
```

We strive to:
- Minimize dependencies
- Use well-maintained crates
- Regularly update dependencies
- Review dependency changes

## Compliance

### License Compatibility

GuestKit is licensed under Apache-2.0 by ZyvorAI Labs Private Limited, ensuring:
- Security fixes can be shared
- Modifications carry Apache 2.0 terms when redistributed
- Commercial use is permitted

### Standards

We aim to follow:
- Rust API Guidelines
- Secure coding practices
- OWASP secure coding guidelines

## Security Updates

Security updates will be released as:
- Patch versions (0.2.x) for backward-compatible security fixes
- Minor versions (0.x.0) if security fix requires API changes

All security updates will be clearly marked in:
- Release notes
- CHANGELOG.md
- GitHub releases

## Questions?

For security questions that aren't vulnerabilities:
- Open a discussion on GitHub
- Email: ssahani@gmail.com

Thank you for helping keep GuestKit secure!
