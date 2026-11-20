// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Bootability prediction engine.

pub mod checks;
pub mod engine;
pub mod report;

#[cfg(feature = "agent")]
pub mod live;

pub use engine::{analyze_bootability, BootTarget};
pub use report::{BootabilityReport, CheckResult, CheckSeverity, Finding};
