//! `advisory-cron register` — generate launchd plist + bootstrap into user session.
//! Phase 1.1 stub. Implementation arrives in Phase 1.3.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Cron-style schedule expression (e.g., "0 9 * * *").
    #[arg(long)]
    pub schedule: String,

    /// Label for the launchd job (becomes plist filename component).
    #[arg(long)]
    pub label: String,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`register` not yet implemented (Phase 1.3)");
}
