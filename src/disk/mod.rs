// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Pure Rust disk image handling
//!
//! This module provides pure Rust implementations for reading disk images,
//! parsing partition tables, and detecting filesystems.

pub mod filesystem;
pub mod loop_device;
pub mod nbd;
pub mod partition;
pub mod reader;

pub use filesystem::{FileSystem, FileSystemType};
pub use loop_device::LoopDevice;
pub use nbd::NbdDevice;
pub use partition::{Partition, PartitionTable, PartitionType};
pub use reader::DiskReader;

/// Detect image container format (qcow2, raw, vmdk, …) from magic bytes.
pub fn detect_image_format<P: AsRef<std::path::Path>>(
    path: P,
) -> crate::core::Result<crate::core::DiskFormat> {
    DiskReader::detect_image_format(path)
}

/// Make `path` absolute (without resolving symlinks) before handing it to an
/// external tool. A relative name beginning with `-` would otherwise be parsed
/// as an option by `losetup` / `qemu-nbd` / `qemu-img`, and a name such as
/// `json:{...}` or `nbd:host:port` as a qemu protocol spec.
pub(crate) fn absolute_path(path: &std::path::Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| std::path::Path::new("/").join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::absolute_path;
    use std::path::Path;

    #[test]
    fn absolute_path_never_starts_with_dash_or_protocol() {
        for p in ["-evil.img", "json:{\"x\":1}", "nbd:host:10809", "a/b.raw"] {
            let a = absolute_path(Path::new(p));
            assert!(a.is_absolute(), "{p} -> {}", a.display());
            assert!(a.to_string_lossy().starts_with('/'));
        }
        assert_eq!(absolute_path(Path::new("/x/y")), Path::new("/x/y"));
    }
}
