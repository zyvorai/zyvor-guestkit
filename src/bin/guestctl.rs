// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0
//! guestctl binary — alias entry point for guestkit.

fn main() -> anyhow::Result<()> {
    guestkit::cli::run()
}
