// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0
//! Zyvor VM Tools — in-guest agent entry point (virtio-serial).

#[cfg(feature = "agent")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    guestkit::agent::cli::run_agent(guestkit::agent::cli::AgentArgs {
        channel: guestkit::agent::cli::AgentChannel::Virtio,
        device: None,
        socket: None,
        user: None,
    })
    .await
}

#[cfg(not(feature = "agent"))]
fn main() {
    eprintln!("zyvor-guest-agent requires building with --features agent");
    std::process::exit(1);
}
