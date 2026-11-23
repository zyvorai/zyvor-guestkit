// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

//! Confined, capped, read-only access to a mounted guest root.

use std::fs;
use std::io::Read;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

/// Resource caps applied to every read. Defaults are deliberately small: the
/// audited files (configs, account databases, boot entries) are tiny, so a
/// huge one is either corrupt or hostile.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Largest file that will be read.
    pub max_file_bytes: u64,
    /// Largest directory listing that will be returned.
    pub max_dir_entries: usize,
    /// Maximum `Include` / `@includedir` nesting.
    pub max_include_depth: usize,
    /// Maximum number of files parsed by one rule group (include fan-out).
    pub max_files_per_group: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 1024 * 1024,
            max_dir_entries: 4096,
            max_include_depth: 8,
            max_files_per_group: 256,
        }
    }
}

/// Result of reading one guest file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    Missing,
    /// Not a regular file (directory, FIFO, device, socket) or unresolvable
    /// (symlink loop). The string says why.
    Refused(String),
    TooLarge(u64),
    Io(String),
    Text(String),
}

impl ReadOutcome {
    pub fn text(self) -> Option<String> {
        match self {
            Self::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// A mounted guest root on the host (read-only mount or test fixture).
#[derive(Debug, Clone)]
pub struct GuestRoot {
    root: PathBuf,
    limits: Limits,
}

impl GuestRoot {
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::with_limits(root, Limits::default())
    }

    pub fn with_limits(root: impl AsRef<Path>, limits: Limits) -> std::io::Result<Self> {
        Ok(Self {
            root: root.as_ref().canonicalize()?,
            limits,
        })
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Resolve a guest path to a host path inside the root. Symlinks are
    /// followed as the guest would (absolute targets restart at the guest
    /// root, `..` cannot climb above it); loops return `None`.
    fn resolve(&self, guest_path: &str) -> Option<PathBuf> {
        crate::guestfs::file_ops::resolve_in_root(&self.root, guest_path).ok()
    }

    pub fn exists(&self, guest_path: &str) -> bool {
        self.resolve(guest_path)
            .map(|p| fs::symlink_metadata(p).is_ok())
            .unwrap_or(false)
    }

    pub fn is_dir(&self, guest_path: &str) -> bool {
        self.resolve(guest_path)
            .and_then(|p| fs::metadata(p).ok())
            .map(|m| m.is_dir())
            .unwrap_or(false)
    }

    /// Size of a regular file (after symlink resolution).
    pub fn size(&self, guest_path: &str) -> Option<u64> {
        let m = fs::metadata(self.resolve(guest_path)?).ok()?;
        m.is_file().then_some(m.len())
    }

    /// Permission bits (`st_mode & 0o7777`) of the resolved path.
    pub fn mode(&self, guest_path: &str) -> Option<u32> {
        let m = fs::metadata(self.resolve(guest_path)?).ok()?;
        Some(m.mode() & 0o7777)
    }

    /// Is the *final path component* a symlink whose target does not exist
    /// inside the guest? (A dangling boot-file link.)
    pub fn is_dangling_symlink(&self, guest_path: &str) -> bool {
        // Resolve the parent, keep the leaf unresolved.
        let p = Path::new(guest_path);
        let (Some(parent), Some(leaf)) = (p.parent(), p.file_name()) else {
            return false;
        };
        let Some(parent_host) = self.resolve(&parent.to_string_lossy()) else {
            return false;
        };
        let leaf_host = parent_host.join(leaf);
        match fs::symlink_metadata(&leaf_host) {
            Ok(m) if m.file_type().is_symlink() => !self.exists(guest_path),
            _ => false,
        }
    }

    /// Read a regular file as lossy UTF-8, honouring all caps.
    pub fn read_text(&self, guest_path: &str) -> ReadOutcome {
        let Some(host) = self.resolve(guest_path) else {
            return ReadOutcome::Refused("symlink loop or unresolvable path".into());
        };
        let meta = match fs::metadata(&host) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ReadOutcome::Missing,
            Err(e) => return ReadOutcome::Io(e.to_string()),
        };
        let ft = meta.file_type();
        if !ft.is_file() {
            let kind = if ft.is_dir() {
                "directory"
            } else if ft.is_fifo() {
                "fifo"
            } else if ft.is_socket() {
                "socket"
            } else {
                "device or special file"
            };
            return ReadOutcome::Refused(format!("not a regular file ({kind})"));
        }
        if meta.len() > self.limits.max_file_bytes {
            return ReadOutcome::TooLarge(meta.len());
        }
        let file = match fs::File::open(&host) {
            Ok(f) => f,
            Err(e) => return ReadOutcome::Io(e.to_string()),
        };
        let mut buf = Vec::new();
        // The file may grow between stat and read; bound the read itself.
        if let Err(e) = file
            .take(self.limits.max_file_bytes + 1)
            .read_to_end(&mut buf)
        {
            return ReadOutcome::Io(e.to_string());
        }
        if buf.len() as u64 > self.limits.max_file_bytes {
            return ReadOutcome::TooLarge(buf.len() as u64);
        }
        ReadOutcome::Text(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Sorted entry names of a directory. The bool is `true` when the listing
    /// was truncated at `max_dir_entries`.
    pub fn list_dir(&self, guest_path: &str) -> (Vec<String>, bool) {
        let Some(host) = self.resolve(guest_path) else {
            return (Vec::new(), false);
        };
        let Ok(rd) = fs::read_dir(host) else {
            return (Vec::new(), false);
        };
        let mut names = Vec::new();
        let mut truncated = false;
        for entry in rd {
            let Ok(entry) = entry else { continue };
            if names.len() >= self.limits.max_dir_entries {
                truncated = true;
                break;
            }
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        (names, truncated)
    }
}

/// Join a guest directory and an entry name without allowing the name to
/// introduce a path separator (hostile names such as `..` or `a/b`).
pub fn join_guest(dir: &str, name: &str) -> Option<String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        return None;
    }
    Some(format!("{}/{}", dir.trim_end_matches('/'), name))
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;
    use std::os::unix::fs::symlink;

    /// A throwaway guest tree.
    pub struct Tree {
        pub dir: tempfile::TempDir,
    }

    impl Tree {
        pub fn new() -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
            }
        }
        pub fn file(&self, path: &str, content: &str) -> &Self {
            let p = self.dir.path().join(path.trim_start_matches('/'));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, content).unwrap();
            self
        }
        pub fn bytes(&self, path: &str, content: &[u8]) -> &Self {
            let p = self.dir.path().join(path.trim_start_matches('/'));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, content).unwrap();
            self
        }
        pub fn link(&self, path: &str, target: &str) -> &Self {
            let p = self.dir.path().join(path.trim_start_matches('/'));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            symlink(target, p).unwrap();
            self
        }
        pub fn mode(&self, path: &str, mode: u32) -> &Self {
            use std::os::unix::fs::PermissionsExt;
            let p = self.dir.path().join(path.trim_start_matches('/'));
            fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
            self
        }
        pub fn root(&self) -> GuestRoot {
            GuestRoot::new(self.dir.path()).unwrap()
        }
        pub fn root_with(&self, limits: Limits) -> GuestRoot {
            GuestRoot::with_limits(self.dir.path(), limits).unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::Tree;
    use super::*;

    #[test]
    fn reads_text_and_reports_missing() {
        let t = Tree::new();
        t.file("/etc/hostname", "box\n");
        let r = t.root();
        assert_eq!(
            r.read_text("/etc/hostname"),
            ReadOutcome::Text("box\n".into())
        );
        assert_eq!(r.read_text("/etc/nope"), ReadOutcome::Missing);
    }

    #[test]
    fn absolute_symlink_escape_stays_inside_guest() {
        let t = Tree::new();
        t.file("/etc/real", "guest-data");
        // Host /etc/passwd must never be read: the link restarts at guest root.
        t.link("/etc/escape", "/etc/passwd");
        t.link("/etc/dotdot", "../../../../../../etc/passwd");
        t.link("/etc/ok", "/etc/real");
        let r = t.root();
        assert_eq!(r.read_text("/etc/escape"), ReadOutcome::Missing);
        assert_eq!(r.read_text("/etc/dotdot"), ReadOutcome::Missing);
        assert_eq!(
            r.read_text("/etc/ok"),
            ReadOutcome::Text("guest-data".into())
        );
    }

    #[test]
    fn symlink_loop_is_refused_not_hung() {
        let t = Tree::new();
        t.link("/etc/a", "/etc/b");
        t.link("/etc/b", "/etc/a");
        let r = t.root();
        assert!(matches!(r.read_text("/etc/a"), ReadOutcome::Refused(_)));
        assert!(!r.exists("/etc/a"));
    }

    #[test]
    fn oversized_file_is_not_read() {
        let t = Tree::new();
        t.bytes("/etc/big", &vec![b'a'; 4096]);
        let r = t.root_with(Limits {
            max_file_bytes: 1024,
            ..Limits::default()
        });
        assert_eq!(r.read_text("/etc/big"), ReadOutcome::TooLarge(4096));
    }

    #[test]
    fn fifo_is_refused_without_blocking() {
        let t = Tree::new();
        std::fs::create_dir_all(t.dir.path().join("etc")).unwrap();
        let p = std::ffi::CString::new(t.dir.path().join("etc/pipe").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(p.as_ptr(), 0o600) }, 0);
        let r = t.root();
        match r.read_text("/etc/pipe") {
            ReadOutcome::Refused(m) => assert!(m.contains("fifo")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn directory_read_is_refused() {
        let t = Tree::new();
        t.file("/etc/d/x", "1");
        assert!(matches!(
            t.root().read_text("/etc/d"),
            ReadOutcome::Refused(_)
        ));
    }

    #[test]
    fn dir_fanout_is_truncated() {
        let t = Tree::new();
        for i in 0..50 {
            t.file(&format!("/etc/x/f{i:03}"), "");
        }
        let r = t.root_with(Limits {
            max_dir_entries: 10,
            ..Limits::default()
        });
        let (names, truncated) = r.list_dir("/etc/x");
        assert_eq!(names.len(), 10);
        assert!(truncated);
    }

    #[test]
    fn invalid_utf8_is_lossy() {
        let t = Tree::new();
        t.bytes("/etc/b", &[0x66, 0xff, 0x6f]);
        assert!(t.root().read_text("/etc/b").text().unwrap().contains('f'));
    }

    #[test]
    fn dangling_symlink_detected() {
        let t = Tree::new();
        t.link("/boot/vmlinuz", "vmlinuz-1.0");
        t.file("/boot/ok-target", "x");
        t.link("/boot/ok", "ok-target");
        let r = t.root();
        assert!(r.is_dangling_symlink("/boot/vmlinuz"));
        assert!(!r.is_dangling_symlink("/boot/ok"));
        assert!(!r.is_dangling_symlink("/boot/missing"));
    }

    #[test]
    fn join_guest_rejects_hostile_names() {
        assert_eq!(join_guest("/etc/", "a").as_deref(), Some("/etc/a"));
        assert!(join_guest("/etc", "..").is_none());
        assert!(join_guest("/etc", "a/b").is_none());
        assert!(join_guest("/etc", "").is_none());
    }
}
