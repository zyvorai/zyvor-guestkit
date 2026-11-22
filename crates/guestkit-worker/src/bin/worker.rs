// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0
//! Guestkit Worker CLI
//!
//! Command-line interface for the guestkit-worker distributed job processing system.
//!
//! # Commands
//!
//! - `daemon` - Start the worker daemon
//! - `submit` - Submit a job to the worker
//! - `status` - Get job status
//! - `result` - Get job result
//! - `list` - List all jobs
//! - `capabilities` - Get worker capabilities
//! - `health` - Check worker health

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    guestkit_worker::cli::run().await
}
