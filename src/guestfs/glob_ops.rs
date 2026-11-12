// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Glob and wildcard operations for disk image manipulation
//!
//! This implementation provides glob pattern matching functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

/// Convert a host glob hit to a guest path, dropping hits that reach outside
/// the guest root through symlinked directories.
fn glob_match_to_guest(root: &std::path::Path, hit: &std::path::Path) -> Option<String> {
    let parent = hit.parent()?;
    if !parent.canonicalize().ok()?.starts_with(root) {
        return None;
    }
    let rel = hit.strip_prefix(root).ok()?;
    Some(format!("/{}", rel.to_str()?))
}

impl Guestfs {
    /// Expand glob pattern
    ///
    pub fn glob_expand(&mut self, pattern: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: glob_expand {}", pattern);
        }

        // Patterns are always interpreted relative to the guest root (never the
        // host cwd), and may not climb out of it.
        if pattern.split('/').any(|c| c == "..") {
            return Err(Error::InvalidOperation(format!(
                "Glob pattern must not contain '..': {}",
                pattern
            )));
        }
        let root = std::path::PathBuf::from(self.find_root_mountpoint()?)
            .canonicalize()
            .map_err(|e| Error::InvalidState(format!("Failed to canonicalize mount root: {e}")))?;
        let host_pattern = format!(
            "{}/{}",
            glob::Pattern::escape(&root.to_string_lossy()),
            pattern.trim_start_matches('/')
        );

        let mut matches = Vec::new();
        match glob::glob(&host_pattern) {
            Ok(paths) => {
                for path in paths.flatten() {
                    if let Some(g) = glob_match_to_guest(&root, &path) {
                        matches.push(g);
                    }
                }
            }
            Err(e) => {
                return Err(Error::CommandFailed(format!("Glob pattern error: {}", e)));
            }
        }
        matches.sort();

        Ok(matches)
    }

    /// List files matching pattern
    ///
    pub fn ls0(&mut self, dir: &str, filenames: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ls0 {} {}", dir, filenames);
        }

        let entries = self.ls(dir)?;

        // Write to file with NUL separators
        let host_file = self.resolve_guest_path(filenames)?;

        let mut content = String::new();
        for entry in entries {
            content.push_str(&entry);
            content.push('\0');
        }

        std::fs::write(&host_file, content.as_bytes()).map_err(Error::Io)?;

        Ok(())
    }

    /// Recursive file listing
    ///
    pub fn find0_impl(&mut self, directory: &str, files: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: find0 {} {}", directory, files);
        }

        let all_files = self.find(directory)?;

        let host_file = self.resolve_guest_path(files)?;

        let mut content = String::new();
        for file in all_files {
            content.push_str(&file);
            content.push('\0');
        }

        std::fs::write(&host_file, content.as_bytes()).map_err(Error::Io)?;

        Ok(())
    }

    /// Match files against pattern
    ///
    pub fn grep_lines(&mut self, regex: &str, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: grep_lines {} {}", regex, path);
        }

        let content = self.cat(path)?;

        let re = regex::Regex::new(regex)
            .map_err(|e| Error::InvalidFormat(format!("Invalid regex: {}", e)))?;

        let matching_lines: Vec<String> = content
            .lines()
            .filter(|line| re.is_match(line))
            .map(|line| line.to_string())
            .collect();

        Ok(matching_lines)
    }

    /// Read directory entries
    ///
    pub fn readdir(&mut self, dir: &str) -> Result<Vec<(String, u8)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: readdir {}", dir);
        }

        let host_path = self.resolve_guest_path(dir)?;

        let mut entries = Vec::new();

        for entry in std::fs::read_dir(&host_path).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let name = entry.file_name().to_string_lossy().to_string();

            let file_type = entry.file_type().map_err(Error::Io)?;

            let type_code = if file_type.is_file() {
                b'r' // Regular file
            } else if file_type.is_dir() {
                b'd' // Directory
            } else if file_type.is_symlink() {
                b'l' // Symlink
            } else {
                b'u' // Unknown
            };

            entries.push((name, type_code));
        }

        Ok(entries)
    }

    /// Case insensitive path lookup
    ///
    pub fn case_sensitive_path(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: case_sensitive_path {}", path);
        }

        // Check if path exists as-is
        if self.exists(path)? {
            return Ok(path.to_string());
        }

        // Try to find case-insensitive match
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = String::from("/");

        for part in parts {
            if let Ok(entries) = self.ls(&current) {
                // Look for case-insensitive match
                if let Some(matched) = entries
                    .iter()
                    .find(|e| e.to_lowercase() == part.to_lowercase())
                {
                    if current == "/" {
                        current = format!("/{}", matched);
                    } else {
                        current = format!("{}/{}", current, matched);
                    }
                } else {
                    return Err(Error::NotFound(format!("Path not found: {}", path)));
                }
            } else {
                return Err(Error::NotFound(format!("Path not found: {}", path)));
            }
        }

        Ok(current)
    }

    /// List extended attributes with values
    ///
    pub fn lxattrlist(&mut self, path: &str, names: &[&str]) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lxattrlist {} {:?}", path, names);
        }

        let mut results = Vec::new();

        for name in names {
            let file_path = if path.ends_with('/') {
                format!("{}{}", path, name)
            } else {
                format!("{}/{}", path, name)
            };

            if let Ok(xattrs) = self.lgetxattrs(&file_path) {
                for attr in xattrs {
                    results.push(format!("{}: {}", file_path, attr));
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }

    fn fake(root: &std::path::Path) -> Guestfs {
        use crate::guestfs::handle::GuestfsState;
        let mut g = Guestfs::new().unwrap();
        g.state = GuestfsState::Ready;
        g.mount_root = Some(root.to_path_buf());
        g.mounted
            .insert("/dev/fake1".into(), root.to_string_lossy().into_owned());
        g
    }

    fn detach(mut g: Guestfs) {
        use crate::guestfs::handle::GuestfsState;
        g.mounted.clear();
        g.mount_root = None;
        g.state = GuestfsState::Config;
    }

    #[test]
    fn glob_returns_guest_paths_confined_to_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(root.join("etc/a.conf"), b"").unwrap();
        std::fs::write(outside.join("secret.conf"), b"").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("etc/link")).unwrap();

        let mut g = fake(&root);
        let r = g.glob_expand("/etc/*.conf");
        let up = g.glob_expand("/../outside/*");
        let via_link = g.glob_expand("/etc/link/*.conf");
        let rel = g.glob_expand("etc/*.conf");
        detach(g);

        assert_eq!(r.unwrap(), vec!["/etc/a.conf".to_string()]);
        assert!(up.is_err());
        assert!(via_link.unwrap().is_empty());
        assert_eq!(rel.unwrap(), vec!["/etc/a.conf".to_string()]);
    }
}
