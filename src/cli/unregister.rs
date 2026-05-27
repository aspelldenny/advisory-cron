//! `advisory-cron unregister` — remove launchd plist + bootout from user session.
//! Phase 1.1 stub. Implementation arrives in Phase 1.3.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Label of the launchd job to remove.
    #[arg(long)]
    pub label: String,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`unregister` not yet implemented (Phase 1.3)");
}
