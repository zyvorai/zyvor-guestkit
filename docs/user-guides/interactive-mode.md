# GuestCtl Interactive Mode Guide

## Overview

GuestCtl's Interactive Mode provides a powerful REPL (Read-Eval-Print Loop) for exploring VM disk images. Instead of launching the appliance for each command, you launch it once and run multiple commands interactively.

## Why Use Interactive Mode?

**Problem with standard mode:**
```bash
# Each command launches the appliance (slow!)
guestkit list disk.qcow2 /etc
guestkit cat disk.qcow2 /etc/hostname
guestkit list disk.qcow2 /var/log
# 3 separate appliance launches = slow
```

**Solution with interactive mode:**
```bash
# Launch appliance once
guestkit interactive disk.qcow2

# Then run commands instantly
guestkit> ls /etc
guestkit> cat /etc/hostname
guestkit> ls /var/log
# Same appliance, instant responses = fast!
```

## Getting Started

### Launch Interactive Mode

```bash
# Full command
guestkit interactive disk.qcow2

# Short alias
guestkit repl disk.qcow2
```

### Startup Sequence

When you launch interactive mode, you'll see:

```
Initializing GuestCtl Interactive Mode...

  → Loading disk: disk.qcow2
  → Launching appliance...
  → Inspecting disk...

  ✓ Found: linux ubuntu 22.04

Ready! Type 'help' for commands, 'exit' to quit.

guestkit>
```

## Available Commands

### System Information

#### `info` - Show disk and OS information

```bash
guestkit> info

Disk Information:
  Path: /path/to/disk.qcow2

Operating System:
  Type: linux
  Distribution: ubuntu
  Version: 22.04
  Hostname: web-server
  Architecture: x86_64
```

### Filesystem Operations

#### `filesystems` (alias: `fs`) - List available filesystems

```bash
guestkit> filesystems

Available Filesystems:

  /dev/sda1 ext4
  /dev/sda2 swap
```

#### `mount` - Mount a filesystem

```bash
guestkit> mount /dev/sda1 /

✓ Mounted /dev/sda1 at /
```

#### `umount` (alias: `unmount`) - Unmount a filesystem

```bash
guestkit> umount /

✓ Unmounted /
```

#### `mounts` - Show mounted filesystems

```bash
guestkit> mounts

Mounted Filesystems:

  /dev/sda1 → /
```

### File Operations

#### `ls` - List directory contents

```bash
guestkit> ls /etc

  hostname
  hosts
  fstab
  ...

25 entries
```

#### `cat` - Display file contents

```bash
guestkit> cat /etc/hostname

web-server
```

#### `head` - Display first lines of file

```bash
guestkit> head /var/log/syslog 10

Jan 24 10:00:01 web-server systemd[1]: Started Session 1.
Jan 24 10:00:02 web-server kernel: ...
...
```

Usage: `head <path> [lines]` (default: 10 lines)

#### `find` - Find files by pattern

```bash
guestkit> find '*.conf'

  /etc/nginx/nginx.conf
  /etc/ssh/sshd_config
  /etc/systemd/system.conf
  ...

15 matches
```

#### `stat` - Show file information

```bash
guestkit> stat /etc/hostname

File Information:
  Path: /etc/hostname
  Size: 11 bytes
  Mode: 644
  UID: 0
  GID: 0
```

#### `download` (alias: `dl`) - Download file from disk

```bash
guestkit> download /etc/hostname ./hostname.txt

✓ Downloaded /etc/hostname to ./hostname.txt
```

#### `explore` (alias: `ex`) - Interactive file browser 🆕

Launch a visual, interactive file explorer with rich features:

```bash
guestkit> explore /var/log

┌─────────────────────────────────────────────────────────┐
│ 📍 Path: /var/log  📊 Items: 42                         │
├─────────────────────────────────────────────────────────┤
│ ▸ 📁 ..                                           <DIR> │
│   📁 audit                                        <DIR> │
│   📁 journal                                      <DIR> │
│ ▸ 📄 syslog                                    12.4 MB │
│   📄 syslog.1                                   8.2 MB │
│   📦 syslog.2.gz                                2.1 MB │
│                                                         │
│ [v] Preview  [i] Info  [/] Filter  [q] Quit           │
└─────────────────────────────────────────────────────────┘
```

**Features:**
- **Visual navigation** with color-coded files and emoji icons
- **File preview** (press `v`) - view file contents with line numbers
- **File information** (press `i`) - detailed metadata and stats
- **Real-time filtering** (press `/`) - live search as you type
- **Hidden files toggle** (press `.`) - show/hide dotfiles
- **Smart sorting** (press `s`) - by name, size, or date
- **Vim-like navigation** - j/k or arrow keys, Enter to open

**Keyboard Shortcuts:**
- `↑↓` / `j k` - Navigate up/down
- `Enter` - Enter directory
- `Backspace` - Go to parent directory
- `v` - Preview file contents
- `i` - Show file information
- `/` - Start real-time filter
- `.` - Toggle hidden files
- `s` - Cycle sort modes
- `?` - Show help
- `q` - Quit explorer

**Usage:** `explore [path]` (defaults to current directory)

**Tip:** Also available directly via CLI: `guestkit explore vm.qcow2 /path`

### System Inspection

#### `packages` (alias: `pkg`) - List installed packages

```bash
guestkit> packages

  nginx 1.18.0 High performance web server
  postgresql-14 14.5-0ubuntu0.22.04.1 Object-relational SQL database
  python3 3.10.6-1~22.04 Interactive high-level object-oriented language
  ...

1847 packages total
```

Filter packages:
```bash
guestkit> packages python

  python3 3.10.6-1~22.04 Interactive high-level object-oriented language
  python3-pip 22.0.2+dfsg-1 Package installer for Python
  ...

42 packages total
```

**Note:** Shows first 50 packages by default. Use filter to narrow results.

#### `services` (alias: `svc`) - List system services

```bash
guestkit> services

Enabled Services:

  ▶ nginx.service
  ▶ postgresql.service
  ▶ ssh.service
  ▶ systemd-resolved.service
  ...

23 services enabled
```

#### `users` - List user accounts

```bash
guestkit> users

User Accounts:

  root (uid: 0, shell: /bin/bash)
  john (uid: 1000, shell: /bin/bash)
  jane (uid: 1001, shell: /bin/zsh)
  www-data (uid: 33, shell: /usr/sbin/nologin)
  ...

15 users
```

**Color coding:**
- Red (bold): root user (UID 0)
- Bright white: Regular users (UID >= 1000)
- Yellow: System users (0 < UID < 1000)

#### `network` (alias: `net`) - Show network configuration

```bash
guestkit> network

Network Interfaces:

  eth0 52:54:00:12:34:56
    → 192.168.1.100
    → 10.0.0.5

  lo 00:00:00:00:00:00
    → 127.0.0.1

DNS Servers:
  8.8.8.8
  8.8.4.4
```

### Other Commands

#### `clear` (alias: `cls`) - Clear screen

```bash
guestkit> clear
```

#### `help` (alias: `?`) - Show help

```bash
guestkit> help

GuestCtl Interactive Commands:

  System Information:
    info  - Show disk and OS information

  Filesystem Operations:
    filesystems, fs  - List available filesystems
    mount <device> <path>  - Mount a filesystem
    umount <path>  - Unmount a filesystem
    mounts  - Show mounted filesystems

  File Operations:
    ls [path]  - List directory contents
    cat <path>  - Display file contents
    head <path> [lines]  - Display first lines of file
    find <pattern>  - Find files by name pattern
    stat <path>  - Show file information
    download <src> <dest>  - Download file from disk
    explore, ex [path]  - Interactive file browser with preview 🆕

  System Inspection:
    packages, pkg [filter]  - List installed packages
    services, svc  - List system services
    users  - List user accounts
    network, net  - Show network configuration

  Other:
    clear, cls  - Clear screen
    help, ?  - Show this help
    exit, quit, q  - Exit interactive mode
```

#### `exit` (aliases: `quit`, `q`) - Exit interactive mode

```bash
guestkit> exit
Goodbye!
```

## Features

### Command History

Use arrow keys to navigate command history:
- **↑ (Up)**: Previous command
- **↓ (Down)**: Next command

History is maintained for the duration of the session.

### Keyboard Shortcuts

- **Ctrl+C**: Cancel current input (doesn't exit)
- **Ctrl+D**: Exit interactive mode
- **Tab**: Command completion (future feature)

### Colorized Output

Interactive mode uses colors to make output more readable:
- **Cyan**: Commands, labels, prompts
- **Green**: Success messages, enabled items
- **Red**: Errors, root user
- **Yellow**: Warnings, system users, filesystem types
- **White/Bright**: Important values, file names
- **Dimmed**: Less important information

### Auto-Inspection

On startup, the disk is automatically inspected:
- OS type detected
- Distribution identified
- Version determined
- Displayed in the welcome message

This means you can immediately use inspection commands like `packages`, `services`, and `users` without manual setup.

### Persistent Session

The appliance remains running for the duration of your interactive session:
- ✅ Fast command execution (no repeated appliance launches)
- ✅ Mounted filesystems persist between commands
- ✅ Session state maintained
- ✅ Automatic cleanup on exit

## Common Workflows

### Exploring a New Disk

```bash
# 1. Start interactive mode
guestkit interactive unknown-disk.qcow2

# 2. Check what OS it is
guestkit> info

# 3. See filesystems
guestkit> filesystems

# 4. Mount root filesystem
guestkit> mount /dev/sda1 /

# 5. Look around
guestkit> ls /

# 6. Check installed software
guestkit> packages | head -20

# 7. Check running services
guestkit> services

# 8. Done
guestkit> exit
```

### Extracting Configuration Files

```bash
guestkit interactive web-server.qcow2

# Mount filesystem
guestkit> mount /dev/sda1 /

# Find config files
guestkit> find '*.conf'

# Download what you need
guestkit> download /etc/nginx/nginx.conf ./nginx.conf
guestkit> download /etc/ssh/sshd_config ./sshd_config

guestkit> exit
```

### Security Audit

```bash
guestkit interactive server.qcow2

# Check OS and version
guestkit> info

# List all users
guestkit> users

# Check enabled services
guestkit> services

# Look for suspicious packages
guestkit> packages | grep -i hack
guestkit> packages | grep -i backdoor

# Check SSH config
guestkit> cat /etc/ssh/sshd_config

# Check cron jobs
guestkit> ls /etc/cron.d
guestkit> ls /var/spool/cron

guestkit> exit
```

### Troubleshooting Boot Issues

```bash
guestkit interactive broken-vm.qcow2

guestkit> mount /dev/sda1 /

# Check boot configuration
guestkit> cat /etc/fstab
guestkit> cat /boot/grub/grub.cfg

# Check for kernel panics
guestkit> find '/var/log/kern*'
guestkit> cat /var/log/kern.log

# Check systemd
guestkit> cat /etc/systemd/system.conf

guestkit> exit
```

## Tips & Tricks

### 1. Use Aliases

Shorter commands = faster workflow:
- `fs` instead of `filesystems`
- `pkg` instead of `packages`
- `svc` instead of `services`
- `net` instead of `network`
- `dl` instead of `download`

### 2. Filter Package Lists

Instead of scrolling through thousands of packages:
```bash
# Bad: lists all 2000 packages
guestkit> packages

# Good: filter to what you need
guestkit> packages nginx
guestkit> packages python
guestkit> packages kernel
```

### 3. Use `find` for Discovery

Don't know exact paths? Use find:
```bash
guestkit> find '*.log'
guestkit> find '*nginx*'
guestkit> find '*.key'
```

### 4. Combine with Shell

Interactive mode output can be piped:
```bash
guestkit interactive disk.qcow2 << EOF
packages
EOF | grep python
```

### 5. Download Multiple Files

```bash
# Start session
guestkit> mount /dev/sda1 /

# Download batch of files
guestkit> download /etc/passwd ./passwd
guestkit> download /etc/shadow ./shadow
guestkit> download /etc/group ./group

# All downloaded, exit
guestkit> exit
```

## Comparison with Standard Mode

| Feature | Standard Mode | Interactive Mode |
|---------|---------------|------------------|
| Appliance Launch | Every command | Once per session |
| Speed | Slow | Fast |
| Command History | No | Yes ✅ |
| Persistent Mounts | No | Yes ✅ |
| Multi-command workflow | Difficult | Easy ✅ |
| Exploration | Tedious | Natural ✅ |
| Best for | Single operations | Investigation |

## Troubleshooting

### Session Won't Start

```
Error: Failed to launch guestfs
```

**Solutions:**
1. Check if disk image exists and is readable
2. Ensure you have proper permissions
3. Check if NBD/qemu modules are loaded
4. Try with a different disk image to verify

### Commands Fail After Mount

```
Error: Failed to list directory: /etc
```

**Solutions:**
1. Verify filesystem is mounted: `mounts`
2. Check correct mountpoint: `ls /` should work
3. Try mounting root filesystem explicitly:
   ```bash
   guestkit> filesystems
   guestkit> mount /dev/sda1 /
   ```

### Can't See Files

```
guestkit> ls /etc
0 entries
```

**Likely cause:** Filesystem not mounted

**Solution:**
```bash
guestkit> filesystems  # See available filesystems
guestkit> mount /dev/sda1 /  # Mount root
guestkit> ls /etc  # Now works
```

## Limitations

1. **Read-Only by Default**: Disks are mounted read-only. This is a safety feature.
2. **No Tab Completion Yet**: Command completion coming in future release
3. **Single Disk**: Can only work with one disk at a time
4. **Command Parsing**: Simple space-based splitting (doesn't handle quoted arguments yet)

## Future Enhancements

Planned features for future releases:

- [ ] Tab completion for commands and paths
- [ ] Command aliases customization
- [ ] History persistence across sessions
- [ ] Scriptable batch mode
- [ ] Multi-disk support
- [ ] Read-write mode (with warnings)
- [ ] Built-in text editor
- [ ] Diff command for comparing files

## Examples

### Quick Inspection

```bash
$ guestkit interactive ubuntu-22.04.qcow2

Initializing GuestCtl Interactive Mode...
  → Loading disk: ubuntu-22.04.qcow2
  → Launching appliance...
  → Inspecting disk...
  ✓ Found: linux ubuntu 22.04

Ready! Type 'help' for commands, 'exit' to quit.

guestkit> info

Disk Information:
  Path: ubuntu-22.04.qcow2

Operating System:
  Type: linux
  Distribution: ubuntu
  Version: 22.04
  Hostname: ubuntu-server
  Architecture: x86_64

guestkit> packages python

  python3 3.10.6-1~22.04
  python3-pip 22.0.2+dfsg-1
  ...

guestkit> exit
Goodbye!
```

### File Extraction Session

```bash
$ guestkit repl production-db.qcow2

guestkit> filesystems

Available Filesystems:
  /dev/sda1 ext4

guestkit> mount /dev/sda1 /

✓ Mounted /dev/sda1 at /

guestkit> find '*postgres*conf'

  /etc/postgresql/14/main/postgresql.conf
  /etc/postgresql/14/main/pg_hba.conf

guestkit> download /etc/postgresql/14/main/postgresql.conf ./postgresql.conf

✓ Downloaded /etc/postgresql/14/main/postgresql.conf to ./postgresql.conf

guestkit> download /etc/postgresql/14/main/pg_hba.conf ./pg_hba.conf

✓ Downloaded /etc/postgresql/14/main/pg_hba.conf to ./pg_hba.conf

guestkit> exit
Goodbye!
```

## See Also

- [CLI Guide](cli-guide.md) - Standard CLI usage
- [Quick Start](getting-started.md) - Getting started with GuestCtl
- [Troubleshooting](troubleshooting.md) - Common issues

---

**Last Updated:** 2026-01-24
**Status:** Production Ready ✅
