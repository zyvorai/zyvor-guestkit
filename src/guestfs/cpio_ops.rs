// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CPIO archive operations for disk image manipulation
//!
//! This implementation provides CPIO archive creation and extraction.

use crate::core::{Error, Result};
use crate::guestfs::security_utils::PathValidator;
use crate::guestfs::Guestfs;
use std::process::{Command, Stdio};

/// List the member names of a CPIO archive.
///
/// The archive is passed with `-I` instead of being piped through stdin:
/// writing the whole archive to a child whose stdout is also piped deadlocks
/// once the name list exceeds the pipe buffer (typical for initramfs images).
pub(crate) fn list_cpio(archive: &std::path::Path) -> Result<Vec<String>> {
    let output = Command::new("cpio")
        .arg("-t")
        .arg("-I")
        .arg(archive)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute cpio: {}", e)))?;

    if !output.status.success() {
        return Err(Error::CommandFailed(format!(
            "cpio list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect())
}

/// Reject archive member names that would be written outside the extraction
/// directory (absolute names or any `..` component).
pub(crate) fn validate_cpio_member_names(names: &[String]) -> Result<()> {
    for name in names {
        if name.starts_with('/') || name.split('/').any(|c| c == "..") {
            return Err(Error::SecurityViolation(format!(
                "CPIO archive member escapes extraction directory: {}",
                name
            )));
        }
    }
    Ok(())
}

/// Extract `archive` into `host_dir`, refusing members that escape it.
pub(crate) fn extract_cpio_confined(
    archive: &std::path::Path,
    host_dir: &std::path::Path,
) -> Result<()> {
    validate_cpio_member_names(&list_cpio(archive)?)?;

    let output = Command::new("cpio")
        .arg("-idm")
        .arg("--no-absolute-filenames")
        .arg("-D")
        .arg(host_dir)
        .arg("-I")
        .arg(archive)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute cpio: {}", e)))?;

    if !output.status.success() {
        return Err(Error::CommandFailed(format!(
            "cpio extraction failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

impl Guestfs {
    /// Extract CPIO archive to directory
    ///
    /// Additional functionality for CPIO support
    pub fn cpio_extract(&mut self, archive: &str, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cpio_extract {} {}", archive, directory);
        }

        let host_dir = self.resolve_guest_path(directory)?;

        extract_cpio_confined(std::path::Path::new(archive), &host_dir)?;

        Ok(())
    }

    /// Create CPIO archive from directory
    ///
    /// Additional functionality for CPIO support
    pub fn cpio_create(&mut self, directory: &str, archive: &str, format: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cpio_create {} {} {}", directory, archive, format);
        }

        // Validate inputs to prevent command injection
        PathValidator::validate_fs_path(directory)?;
        PathValidator::validate_fs_path(archive)?;
        PathValidator::validate_cpio_format(format)?;

        let host_dir = self.resolve_guest_path(directory)?;

        // Use piped commands instead of shell to prevent injection
        // Start find process
        let mut find_child = Command::new("find")
            .arg(".")
            .current_dir(&host_dir)
            .arg("-print")
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute find: {}", e)))?;

        let find_stdout = find_child
            .stdout
            .take()
            .ok_or_else(|| Error::CommandFailed("Failed to capture find output".to_string()))?;

        // Start cpio process with find output as stdin
        let cpio_child = Command::new("cpio")
            .arg("-o")
            .arg("-H")
            .arg(format)
            .stdin(find_stdout)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute cpio: {}", e)))?;

        // Get cpio output
        let cpio_output = cpio_child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cpio: {}", e)))?;

        // Wait for find child to prevent process leak
        let _ = find_child.wait();

        if !cpio_output.status.success() {
            return Err(Error::CommandFailed(format!(
                "cpio creation failed: {}",
                String::from_utf8_lossy(&cpio_output.stderr)
            )));
        }

        // Write cpio output to archive file
        std::fs::write(archive, &cpio_output.stdout).map_err(Error::Io)?;

        Ok(())
    }

    /// List contents of CPIO archive
    ///
    /// Additional functionality for CPIO support
    pub fn cpio_list(&mut self, archive: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cpio_list {}", archive);
        }

        let files = list_cpio(std::path::Path::new(archive))?;

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpio_member_names_must_stay_inside_extraction_dir() {
        let ok = vec![
            ".".to_string(),
            "etc/passwd".to_string(),
            "a/b..c/d".to_string(),
        ];
        assert!(validate_cpio_member_names(&ok).is_ok());
        for bad in ["/etc/passwd", "../x", "a/../../x", "a/.."] {
            assert!(
                validate_cpio_member_names(&[bad.to_string()]).is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn test_cpio_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
