//! `advisory-cron status` — show next scheduled fire time + last heartbeat.
//! Phase 1.1 stub. Implementation arrives in Phase 1.5.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Emit machine-readable JSON instead of human text.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`status` not yet implemented (Phase 1.5)");
}
