# Explore Command Quick Start

**Launch the interactive file explorer in 2 ways!**

---

## Method 1: Direct Launch ⚡ (Recommended)

The fastest way to start exploring:

```bash
# Basic usage
guestkit explore disk.qcow2

# Start from specific path
guestkit explore disk.qcow2 /etc

# With verbose output
guestkit explore --verbose disk.qcow2

# Short alias
guestkit ex vm.qcow2 /var/log
```

### When to use:
- Quick file browsing
- One-off exploration
- Finding specific files
- Reviewing configurations

---

## Method 2: From Interactive Shell

Launch from within the guestkit shell:

```bash
# Start shell first
guestkit shell disk.qcow2

# Then launch explorer
guestkit> explore

# Or from specific path
guestkit> explore /etc

# Short alias
guestkit> ex /var/log
```

### When to use:
- Long exploration sessions
- Combining with other shell commands
- Scripting and automation
- When you're already in shell mode

---

## Keyboard Shortcuts (Same for Both Methods)

### Essential Keys

```
↑↓ or k/j    Navigate files
Enter        Open directory or view file
v            View file content
i            Show file information
h or ?       Help menu
q or Esc     Quit explorer
```

### Advanced Keys

```
/            Filter files by name
.            Toggle hidden files
s            Change sort mode (name/size/type)
PgUp/PgDn    Fast scrolling
Backspace    Go to parent directory
```

---

## Quick Examples

### Example 1: Find Large Log Files

```bash
# Direct launch
guestkit explore disk.qcow2 /var/log

# In explorer:
# Press 's' twice to sort by size
# Largest logs appear at top
# Press 'v' to view content
```

### Example 2: Security Audit

```bash
# Launch at /etc
guestkit explore disk.qcow2 /etc

# In explorer:
# Navigate to ssh config
# Press 'v' on sshd_config
# Review security settings
```

### Example 3: Web Server Investigation

```bash
# Start from web root
guestkit ex disk.qcow2 /var/www/html

# In explorer:
# Browse site structure
# Press 'v' to view index.html
# Check file permissions with 'i'
```

### Example 4: User Home Discovery

```bash
# Launch at /home
guestkit explore disk.qcow2 /home

# In explorer:
# Navigate to user directory
# Press '.' to show hidden files
# Check .ssh/authorized_keys
```

---

## Comparison: Direct vs Shell

| Feature | Direct Launch | Shell Mode |
|---------|--------------|------------|
| **Speed** | Instant | Requires shell startup |
| **Use Case** | Quick browsing | Multi-command workflow |
| **Context** | Standalone | Part of shell session |
| **Exit** | Returns to terminal | Returns to shell prompt |
| **Best For** | One task | Multiple tasks |

### Choose Direct Launch when:
- ✅ You just want to explore files
- ✅ Single quick task
- ✅ Fastest startup needed
- ✅ Don't need other shell commands

### Choose Shell Mode when:
- ✅ Using multiple shell commands
- ✅ Long investigation session
- ✅ Combining explore with ls, cat, grep, etc.
- ✅ Running shell scripts

---

## Tips & Tricks

### 1. Start from the Right Path

Instead of navigating through directories, start where you need to be:

```bash
# Direct to config files
guestkit explore disk.qcow2 /etc/nginx

# Direct to logs
guestkit explore disk.qcow2 /var/log/apache2

# Direct to application
guestkit explore disk.qcow2 /opt/myapp
```

### 2. Use Filters Efficiently

```bash
# Launch explorer
guestkit explore disk.qcow2 /etc

# Press '/' and type: .conf
# Shows only .conf files
```

### 3. Sort for Quick Analysis

```bash
# In /var/log directory:
# Press 's' to sort by name (default)
# Press 's' again to sort by size (find large logs)
# Press 's' again to sort by type (group by extension)
```

### 4. Toggle Hidden Files

```bash
# In /home/user directory:
# Press '.' to show .bashrc, .ssh, .config, etc.
# Press '.' again to hide them
```

### 5. View First, Navigate Later

```bash
# When you find an interesting file:
# Press 'v' to view content
# Press 'i' to see metadata
# No need to exit explorer!
```

---

## Common Workflows

### Workflow 1: Security Review

```bash
# Launch
guestkit explore disk.qcow2 /etc

# Navigate to SSH config
# Press 'v' on sshd_config
# Check settings
# Press any key to return
# Navigate to other configs
# Repeat
```

### Workflow 2: Log Analysis

```bash
# Launch with verbose
guestkit explore --verbose disk.qcow2 /var/log

# Sort by size ('s' twice)
# View largest log ('v')
# Check for errors
# Navigate to related logs
```

### Workflow 3: Application Audit

```bash
# Start from app directory
guestkit explore disk.qcow2 /opt/myapp

# Browse structure
# Check config files
# Review permissions ('i')
# View startup scripts ('v')
```

### Workflow 4: Combined Shell + Explorer

```bash
# Start shell
guestkit shell disk.qcow2

# Use traditional commands
guestkit> cat /etc/hosts
guestkit> grep "error" /var/log/syslog

# Launch explorer when needed
guestkit> explore /etc/nginx

# Exit explorer, back to shell
guestkit> ls -la /var/www
```

---

## Troubleshooting

### Can't Launch Explorer

**Problem:**
```bash
$ guestkit explore disk.qcow2
Command 'explore' not found
```

**Solution:** Make sure you're using the latest version of guestkit:
```bash
guestkit --version  # Should be v0.3.1 or later
```

### No OS Found

**Problem:**
```
Error: No operating systems found in disk image
```

**Solution:** The image may be:
- Corrupted or incomplete
- Not a bootable VM image
- Using unsupported filesystem

Try with verbose to see more details:
```bash
guestkit explore --verbose disk.qcow2
```

### Colors Not Showing

**Problem:** Black and white output

**Solution:** Check terminal color support:
```bash
echo $TERM
# Should be: xterm-256color

# If not, set it:
export TERM=xterm-256color
```

### Slow Loading

**Problem:** Explorer takes long to load

**Solution:** Start from a specific subdirectory instead of root:
```bash
# Instead of:
guestkit explore disk.qcow2 /

# Try:
guestkit explore disk.qcow2 /etc
```

---

## Help Menu (Press 'h' in Explorer)

Once inside the explorer, press `h` or `?` to see:

```
╔════════════════════ Explorer Help ═══════════════════╗
║                                                       ║
║ 📖 Navigation                                        ║
║   ↑/↓ or k/j    - Move selection up/down             ║
║   PgUp/PgDn     - Page up/down                       ║
║   Enter         - Enter directory / view file        ║
║   Backspace     - Go to parent directory             ║
║                                                       ║
║ ⚡ Actions                                            ║
║   v             - View file content                  ║
║   i             - Show file info                     ║
║   /             - Filter files                       ║
║   .             - Toggle hidden files                ║
║   s             - Cycle sort mode                    ║
║                                                       ║
║ 🔧 General                                            ║
║   h or ?        - Show this help                     ║
║   q or Esc      - Exit explorer                      ║
║   Ctrl+C        - Force exit                         ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝
```

---

## Summary

### Direct Launch (Fastest!)

```bash
guestkit explore disk.qcow2 [path]
```

**Pros:**
- ⚡ Instant start
- 🎯 Direct to your target
- 🚀 Fastest method
- ✨ No shell overhead

### Shell Mode

```bash
guestkit shell disk.qcow2
guestkit> explore [path]
```

**Pros:**
- 🔄 Combine with other commands
- 📜 Shell history
- 🎨 Full shell features
- 🛠️ Scripting support

---

## Next Steps

1. **Try it now:**
   ```bash
   guestkit explore your-vm.qcow2
   ```

2. **Explore common paths:**
   - `/etc` - Configuration files
   - `/var/log` - Log files
   - `/home` - User directories
   - `/var/www` - Web content

3. **Learn shortcuts:**
   - Remember: `h` for help inside explorer
   - Practice: `↑↓`, `v`, `i`, `/`, `s`

4. **Read full docs:**
   - See `EXPLORE-COMMAND.md` for complete guide
   - Check examples and use cases

---

**Happy Exploring!** 🚀📂

*Last Updated: 2026-01-30*
*Version: 1.1 (Added Direct Launch)*
