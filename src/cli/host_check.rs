// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! `guestkit host-check` — is *this host* ready to run guestkit against disk images?
//!
//! Offline and read-only: it looks for helper tools on `PATH`, kernel
//! modules, privileges and writable directories. The evaluation is a pure
//! function over a [`HostProbe`], so it is unit-tested with a fake host.

use anyhow::{bail, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

/// Kernel module state as seen through `/sys/module`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    /// Loaded or built into the running kernel.
    Present,
    /// Not loaded (it may still be loadable with `modprobe`).
    Absent,
}

/// One row of the report.
#[derive(Debug, Clone, Serialize)]
pub struct HostCheck {
    pub id: String,
    pub status: CheckStatus,
    pub detail: String,
    /// What it is needed for.
    pub needed_for: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Whole report.
#[derive(Debug, Clone, Serialize)]
pub struct HostReport {
    pub checks: Vec<HostCheck>,
    pub ok: usize,
    pub warnings: usize,
    pub failures: usize,
}

impl HostReport {
    fn new(checks: Vec<HostCheck>) -> Self {
        let count = |s| checks.iter().filter(|c| c.status == s).count();
        Self {
            ok: count(CheckStatus::Ok),
            warnings: count(CheckStatus::Warn),
            failures: count(CheckStatus::Fail),
            checks,
        }
    }

    /// Whether the host is usable: no failures (and no warnings when `strict`).
    pub fn passed(&self, strict: bool) -> bool {
        self.failures == 0 && !(strict && self.warnings > 0)
    }

    pub fn render_text(&self) -> String {
        let mut out = String::from("Host readiness\n\n");
        for c in &self.checks {
            let tag = match c.status {
                CheckStatus::Ok => "ok  ",
                CheckStatus::Warn => "warn",
                CheckStatus::Fail => "FAIL",
            };
            out.push_str(&format!("  [{tag}] {:<22} {}\n", c.id, c.detail));
            if let Some(h) = &c.hint {
                out.push_str(&format!("         hint: {h}\n"));
            }
        }
        out.push_str(&format!(
            "\n{} ok, {} warning(s), {} failure(s)\n",
            self.ok, self.warnings, self.failures
        ));
        out
    }
}

/// What the check needs to know about the host.
pub trait HostProbe {
    fn tool_path(&self, name: &str) -> Option<PathBuf>;
    fn module(&self, name: &str) -> ModuleState;
    fn is_root(&self) -> bool;
    fn dir_writable(&self, dir: &Path) -> bool;
    fn cache_dir(&self) -> Option<PathBuf>;
    fn lock_dirs(&self) -> Vec<PathBuf>;
}

/// Required helper tools: (name, needed for).
const REQUIRED_TOOLS: &[(&str, &str)] = &[("qemu-img", "image info, convert, resize, snapshots")];
/// Optional helper tools: (name, needed for).
const OPTIONAL_TOOLS: &[(&str, &str)] = &[
    ("qemu-nbd", "mounting qcow2/vmdk/vhdx images"),
    ("losetup", "mounting raw images"),
    ("mount", "mounting guest filesystems"),
    ("blkid", "filesystem detection"),
];
/// Kernel modules: (name, needed for).
const MODULES: &[(&str, &str)] = &[
    ("nbd", "qemu-nbd attach of non-raw images"),
    ("loop", "loop devices for raw images"),
];

fn check(
    id: &str,
    status: CheckStatus,
    detail: String,
    needed_for: &str,
    hint: Option<&str>,
) -> HostCheck {
    HostCheck {
        id: id.to_string(),
        status,
        detail,
        needed_for: needed_for.to_string(),
        hint: hint.map(str::to_string),
    }
}

/// Evaluate the host. Pure over `probe`.
pub fn evaluate(probe: &dyn HostProbe) -> HostReport {
    let mut checks = Vec::new();

    for (tool, why) in REQUIRED_TOOLS {
        checks.push(match probe.tool_path(tool) {
            Some(p) => check(tool, CheckStatus::Ok, p.display().to_string(), why, None),
            None => check(
                tool,
                CheckStatus::Fail,
                "not found on PATH".into(),
                why,
                Some("install qemu-img (package qemu-utils / qemu-img)"),
            ),
        });
    }
    for (tool, why) in OPTIONAL_TOOLS {
        checks.push(match probe.tool_path(tool) {
            Some(p) => check(tool, CheckStatus::Ok, p.display().to_string(), why, None),
            None => check(
                tool,
                CheckStatus::Warn,
                "not found on PATH".into(),
                why,
                None,
            ),
        });
    }

    for (m, why) in MODULES {
        let id = format!("module:{m}");
        checks.push(match probe.module(m) {
            ModuleState::Present => check(&id, CheckStatus::Ok, "loaded".into(), why, None),
            ModuleState::Absent => check(
                &id,
                CheckStatus::Warn,
                "not loaded".into(),
                why,
                Some(&format!("sudo modprobe {m}")),
            ),
        });
    }

    checks.push(if probe.is_root() {
        check(
            "privileges",
            CheckStatus::Ok,
            "running as root".into(),
            "mount, NBD and loop operations",
            None,
        )
    } else {
        check(
            "privileges",
            CheckStatus::Warn,
            "not root".into(),
            "mount, NBD and loop operations",
            Some("offline reads of raw images work unprivileged; mounts need root"),
        )
    });

    let locks = probe.lock_dirs();
    checks.push(match locks.iter().find(|d| probe.dir_writable(d)) {
        Some(d) => check(
            "nbd-lock-dir",
            CheckStatus::Ok,
            format!("{} is writable", d.display()),
            "serialising NBD device allocation",
            None,
        ),
        None => check(
            "nbd-lock-dir",
            CheckStatus::Warn,
            "no writable lock directory (/run/lock, $XDG_RUNTIME_DIR)".into(),
            "serialising NBD device allocation",
            None,
        ),
    });

    checks.push(match probe.cache_dir() {
        Some(d) if probe.dir_writable(&d) => check(
            "cache-dir",
            CheckStatus::Ok,
            format!("{} is writable", d.display()),
            "inspection result cache",
            None,
        ),
        Some(d) => check(
            "cache-dir",
            CheckStatus::Warn,
            format!("{} is not writable (or does not exist yet)", d.display()),
            "inspection result cache",
            Some("pass --cache-dir DIR, or use --no-cache"),
        ),
        None => check(
            "cache-dir",
            CheckStatus::Warn,
            "no cache directory could be determined".into(),
            "inspection result cache",
            Some("pass --cache-dir DIR"),
        ),
    });

    HostReport::new(checks)
}

/// Probe backed by the real host.
pub struct SystemProbe;

impl HostProbe for SystemProbe {
    fn tool_path(&self, name: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|d| d.join(name))
            .find(|p| is_executable(p))
    }

    fn module(&self, name: &str) -> ModuleState {
        if Path::new("/sys/module").join(name).exists() {
            ModuleState::Present
        } else {
            ModuleState::Absent
        }
    }

    fn is_root(&self) -> bool {
        // SAFETY: geteuid() is a safe, read-only syscall.
        unsafe { libc::geteuid() == 0 }
    }

    fn dir_writable(&self, dir: &Path) -> bool {
        match std::fs::metadata(dir) {
            Ok(m) if m.is_dir() => {
                let c = match std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()) {
                    Ok(c) => c,
                    Err(_) => return false,
                };
                // SAFETY: `c` is a valid NUL-terminated path; access(2) only reads it.
                unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
            }
            _ => false,
        }
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        match std::env::var_os("GUESTKIT_CACHE_DIR") {
            Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
            _ => dirs::cache_dir().map(|d| d.join("guestkit")),
        }
    }

    fn lock_dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![PathBuf::from("/run/lock")];
        if let Some(x) = std::env::var_os("XDG_RUNTIME_DIR") {
            if !x.is_empty() {
                v.push(PathBuf::from(x));
            }
        }
        v
    }
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// CLI entry point.
pub fn host_check_command(output: &str, strict: bool) -> Result<()> {
    let report = evaluate(&SystemProbe);
    match output {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        "yaml" => print!("{}", serde_yaml::to_string(&report)?),
        "text" => print!("{}", report.render_text()),
        other => bail!("unknown output format '{other}' (text, json, yaml)"),
    }
    if !report.passed(strict) {
        bail!(
            "host check failed: {} failure(s), {} warning(s){}",
            report.failures,
            report.warnings,
            if strict { " (--strict)" } else { "" }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[derive(Default)]
    struct FakeHost {
        tools: HashSet<&'static str>,
        modules: HashSet<&'static str>,
        root: bool,
        writable: Vec<PathBuf>,
        cache: Option<PathBuf>,
    }

    impl HostProbe for FakeHost {
        fn tool_path(&self, name: &str) -> Option<PathBuf> {
            self.tools
                .contains(name)
                .then(|| PathBuf::from("/usr/bin").join(name))
        }
        fn module(&self, name: &str) -> ModuleState {
            if self.modules.contains(name) {
                ModuleState::Present
            } else {
                ModuleState::Absent
            }
        }
        fn is_root(&self) -> bool {
            self.root
        }
        fn dir_writable(&self, dir: &Path) -> bool {
            self.writable.iter().any(|d| d == dir)
        }
        fn cache_dir(&self) -> Option<PathBuf> {
            self.cache.clone()
        }
        fn lock_dirs(&self) -> Vec<PathBuf> {
            vec![PathBuf::from("/run/lock")]
        }
    }

    fn full_host() -> FakeHost {
        FakeHost {
            tools: ["qemu-img", "qemu-nbd", "losetup", "mount", "blkid"]
                .into_iter()
                .collect(),
            modules: ["nbd", "loop"].into_iter().collect(),
            root: true,
            writable: vec![PathBuf::from("/run/lock"), PathBuf::from("/c")],
            cache: Some(PathBuf::from("/c")),
        }
    }

    fn status<'a>(r: &'a HostReport, id: &str) -> &'a CheckStatus {
        &r.checks.iter().find(|c| c.id == id).unwrap().status
    }

    #[test]
    fn fully_ready_host_passes_even_strict() {
        let r = evaluate(&full_host());
        assert_eq!(r.failures, 0);
        assert_eq!(r.warnings, 0);
        assert!(r.passed(true));
    }

    #[test]
    fn missing_qemu_img_is_a_failure() {
        let mut h = full_host();
        h.tools.remove("qemu-img");
        let r = evaluate(&h);
        assert_eq!(*status(&r, "qemu-img"), CheckStatus::Fail);
        assert!(!r.passed(false));
    }

    #[test]
    fn optional_gaps_warn_and_only_fail_under_strict() {
        let mut h = full_host();
        h.tools.remove("qemu-nbd");
        h.modules.remove("nbd");
        h.root = false;
        let r = evaluate(&h);
        assert_eq!(*status(&r, "qemu-nbd"), CheckStatus::Warn);
        assert_eq!(*status(&r, "module:nbd"), CheckStatus::Warn);
        assert_eq!(*status(&r, "privileges"), CheckStatus::Warn);
        assert!(r.passed(false));
        assert!(!r.passed(true));
    }

    #[test]
    fn unwritable_cache_and_lock_dirs_warn() {
        let mut h = full_host();
        h.writable.clear();
        let r = evaluate(&h);
        assert_eq!(*status(&r, "cache-dir"), CheckStatus::Warn);
        assert_eq!(*status(&r, "nbd-lock-dir"), CheckStatus::Warn);
    }

    #[test]
    fn counts_match_checks_and_json_shape() {
        let r = evaluate(&FakeHost::default());
        assert_eq!(r.ok + r.warnings + r.failures, r.checks.len());
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["checks"][0]["id"], "qemu-img");
        assert_eq!(v["checks"][0]["status"], "fail");
        assert!(v["failures"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn text_report_lists_hints() {
        let t = evaluate(&FakeHost::default()).render_text();
        assert!(t.contains("[FAIL] qemu-img"));
        assert!(t.contains("sudo modprobe nbd"));
    }

    #[test]
    fn real_probe_finds_sh_and_rejects_nonsense() {
        assert!(SystemProbe.tool_path("sh").is_some());
        assert!(SystemProbe
            .tool_path("definitely-not-a-real-tool-xyz")
            .is_none());
        assert!(SystemProbe.dir_writable(&std::env::temp_dir()));
        assert!(!SystemProbe.dir_writable(Path::new("/nonexistent-dir-xyz")));
    }
}
