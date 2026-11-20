// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Root directory for guestkit's on-disk caches.
//!
//! Resolution order: the global `--cache-dir DIR` flag (exported by the CLI as
//! `GUESTKIT_CACHE_DIR`), then the platform cache directory
//! (`$XDG_CACHE_HOME` or `~/.cache`) plus `guestkit`.
//! Subsystem-specific overrides (`GUESTKIT_AI_MEMORY_DIR`,
//! `GUESTKIT_FLEET_BASELINE_DIR`, `GUESTKIT_CLOUD_CACHE_DIR`) still win for
//! their own cache.

use std::ffi::OsStr;
use std::path::PathBuf;

/// Environment variable set by `--cache-dir`.
pub const CACHE_DIR_ENV: &str = "GUESTKIT_CACHE_DIR";

/// Pure resolution: `override_dir` (when non-empty) is used as-is, otherwise
/// `<platform_cache>/guestkit`.
pub fn resolve_cache_root(
    override_dir: Option<&OsStr>,
    platform_cache: Option<PathBuf>,
) -> Option<PathBuf> {
    match override_dir {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => platform_cache.map(|p| p.join("guestkit")),
    }
}

/// The guestkit cache root for this process (not created on disk).
pub fn guestkit_cache_root() -> Option<PathBuf> {
    resolve_cache_root(
        std::env::var_os(CACHE_DIR_ENV).as_deref(),
        dirs::cache_dir(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_is_used_verbatim() {
        let r = resolve_cache_root(
            Some(OsStr::new("/srv/gk")),
            Some(PathBuf::from("/home/u/.cache")),
        );
        assert_eq!(r, Some(PathBuf::from("/srv/gk")));
    }

    #[test]
    fn empty_override_falls_back_to_platform_dir() {
        let r = resolve_cache_root(Some(OsStr::new("")), Some(PathBuf::from("/home/u/.cache")));
        assert_eq!(r, Some(PathBuf::from("/home/u/.cache/guestkit")));
    }

    #[test]
    fn no_override_appends_guestkit() {
        let r = resolve_cache_root(None, Some(PathBuf::from("/x")));
        assert_eq!(r, Some(PathBuf::from("/x/guestkit")));
    }

    #[test]
    fn nothing_available_is_none() {
        assert_eq!(resolve_cache_root(None, None), None);
    }
}
