# Contributing to GuestKit

Thank you for your interest in contributing to GuestKit! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Submitting Changes](#submitting-changes)
- [Adding New APIs](#adding-new-apis)

## Code of Conduct

This project follows the Rust Code of Conduct. Please be respectful and constructive in all interactions.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/guestkit.git
   cd guestkit
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/zyvorai/guestkit.git
   ```

## Development Setup

### Prerequisites

- A current stable Rust toolchain (edition 2021; no `rust-version` is pinned) and the libsystemd development headers
- System dependencies:
  - qemu-img (for disk operations)
  - qemu-nbd (for NBD mounting)
  - cryptsetup (for LUKS support)
  - lvm2 (for LVM operations)

### Installation

```bash
# Install system dependencies (Fedora/RHEL)
sudo dnf install qemu-img cryptsetup lvm2

# Build the project
cargo build

# Run tests
cargo test

# Build documentation
cargo doc --open
```

## Project Structure

```
guestkit/
├── src/
│   ├── lib.rs              # Library root
│   ├── core/               # Core types and utilities
│   │   ├── error.rs        # Error types
│   │   ├── retry.rs        # Retry logic
│   │   └── mod.rs
│   ├── disk/               # Low-level disk access
│   │   ├── reader.rs       # Disk reader
│   │   ├── format.rs       # Format detection
│   │   └── mod.rs
│   └── guestfs/            # GuestFS-compatible API
│       ├── handle.rs       # Main Guestfs struct
│       ├── mount.rs        # Mount operations
│       ├── file_ops.rs     # File operations
│       ├── archive.rs      # Archive operations
│       ├── luks.rs         # LUKS encryption
│       ├── lvm.rs          # LVM management
│       ├── filesystem.rs   # Filesystem operations
│       ├── partition.rs    # Partition management
│       ├── inspect.rs      # OS detection
│       └── ... (73+ modules)
├── examples/               # Example programs
├── tests/                  # Integration tests
└── docs/                   # Additional documentation
```

## Coding Standards

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common mistakes
- Write idiomatic Rust code

### Code Organization

1. **Modules should be focused**: Each module should handle a specific area of functionality
2. **Functions should be small**: Aim for functions under 50 lines when possible
3. **Error handling**: Use `Result<T, Error>` for fallible operations
4. **Documentation**: Every public function must have documentation

### Naming Conventions

- **Functions**: Use snake_case, be descriptive
  - Good: `inspect_get_hostname()`, `luks_open()`
  - Bad: `get_hn()`, `open()`

- **Structs**: Use PascalCase
  - Good: `Guestfs`, `DiskReader`
  - Bad: `guestfs`, `disk_reader`

- **Constants**: Use SCREAMING_SNAKE_CASE
  - Good: `MAX_RETRIES`, `DEFAULT_TIMEOUT`

### Comments

```rust
// Use single-line comments for brief explanations

/// Use doc comments for public APIs
///
/// # Arguments
///
/// * `device` - The device path (e.g., "/dev/sda1")
/// * `format` - The filesystem format (e.g., "ext4", "xfs")
///
/// # Returns
///
/// Returns `Ok(())` on success
///
/// # Errors
///
/// Returns an error if the filesystem creation fails
///
/// # Example
///
/// ```no_run
/// # use guestkit::Guestfs;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut g = Guestfs::new()?;
/// g.mkfs("ext4", "/dev/sda1")?;
/// # Ok(())
/// # }
/// ```
pub fn mkfs(&mut self, fstype: &str, device: &str) -> Result<()> {
    // Implementation
}
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run integration tests only
cargo test --test '*'
```

### Writing Tests

1. **Unit tests**: Place in the same file as the code
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_function() {
           assert_eq!(2 + 2, 4);
       }
   }
   ```

2. **Integration tests**: Place in `tests/` directory
   ```rust
   // tests/integration_test.rs
   use guestkit::Guestfs;

   #[test]
   fn test_workflow() -> Result<(), Box<dyn std::error::Error>> {
       let mut g = Guestfs::new()?;
       // Test code
       Ok(())
   }
   ```

3. **Example tests**: Examples in `examples/` should compile
   ```bash
   cargo test --examples
   ```

### Test Coverage

- Aim for at least 70% code coverage
- All public APIs should have tests
- Test both success and error cases

## Documentation

### Documentation Requirements

1. **Public APIs**: Must have doc comments
2. **Modules**: Should have module-level documentation
3. **Examples**: Complex functions should have usage examples
4. **Error conditions**: Document when errors occur

### Building Documentation

```bash
# Build and open documentation
cargo doc --open

# Build documentation with private items
cargo doc --document-private-items
```

### Documentation Style

```rust
//! Module-level documentation goes here
//!
//! This module provides...

/// Brief one-line summary
///
/// More detailed explanation can go here. Explain what
/// the function does, any important details, etc.
///
/// # Arguments
///
/// * `param1` - Description of first parameter
/// * `param2` - Description of second parameter
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// Describe error conditions
///
/// # Examples
///
/// ```no_run
/// # use guestkit::Guestfs;
/// let mut g = Guestfs::new()?;
/// g.some_function("param")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn some_function(&mut self, param1: &str) -> Result<String> {
    // Implementation
}
```

## Submitting Changes

### Commit Messages

Write clear, descriptive commit messages:

```
Short summary (50 chars or less)

More detailed explanation if needed. Wrap at 72 characters.
Explain what changed and why, not how.

- Bullet points are okay
- Use present tense: "Add feature" not "Added feature"
- Reference issues: "Fixes #123"
```

Examples:
```
Add support for btrfs subvolume operations

Implement create, delete, and list operations for btrfs
subvolumes. Adds new module src/guestfs/btrfs.rs with
comprehensive subvolume management.

Fixes #45
```

### Pull Request Process

1. **Update documentation**: Ensure docs are updated
2. **Add tests**: New features must have tests
3. **Run checks**:
   ```bash
   cargo fmt --check
   cargo clippy
   cargo test
   cargo build --release
   ```
4. **Update CHANGELOG.md**: Add entry for your changes
5. **Create pull request**: With clear description
6. **Address review comments**: Make requested changes

### Pull Request Template

```markdown
## Description
Brief description of changes

## Motivation
Why is this change needed?

## Changes
- List of specific changes
- Another change

## Testing
How has this been tested?

## Checklist
- [ ] Tests pass (`cargo test`)
- [ ] Code formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Examples added/updated if needed
```

## Adding New APIs

### Adding a New Module

1. **Create the module file**: `src/guestfs/newmodule.rs`

```rust
// SPDX-License-Identifier: Apache-2.0
//! Brief module description
//!
//! Detailed module documentation

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Brief function description
    ///
    /// Detailed documentation
    pub fn new_operation(&mut self, param: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: new_operation {}", param);
        }

        // Implementation
        let output = Command::new("some_tool")
            .arg(param)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_operation_api() {
        let mut g = Guestfs::new().unwrap();
        // Basic API test
    }
}
```

2. **Register the module**: Add to `src/guestfs/mod.rs`

```rust
pub mod newmodule;
```

3. **Update documentation**: Add to `GUESTFS_IMPLEMENTATION_STATUS.md`

4. **Add example**: Create `examples/newmodule_example.rs`

5. **Run tests**:
```bash
cargo test
cargo check
```

###  Compatibility

When implementing -compatible APIs:

1. **Match function signatures** as closely as possible
2. **Use same parameter names** for clarity
3. **Preserve behavior** - match  semantics
4. **Document differences** - note any deviations

### Error Handling

Use the standard error types:

```rust
use crate::core::Error;

// Command execution failed
Err(Error::CommandFailed(format!("Command failed: {}", e)))

// File system error
Err(Error::FileSystem(format!("Mount failed: {}", e)))

// Not ready error
Err(Error::NotReady("Must call launch() first".into()))

// Invalid input
Err(Error::InvalidInput(format!("Invalid device: {}", device)))
```

## Additional Resources

- Rust API reference: `cargo doc --open`
- [Examples](../../examples/) - Example programs
- [CLI reference](../user-guides/cli-guide.md) - Command and flag summary
- [Architecture overview](../architecture/overview.md)

## Questions?

- Open an issue on GitHub
- Check existing issues and discussions
- Read the documentation

## License

By contributing, you agree that your contributions will be licensed under the Apache-2.0 license and that ZyvorAI Labs Private Limited may include them in GuestKit.
