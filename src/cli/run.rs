//! `advisory-cron run` — fire the configured task once and write heartbeat.
//! Phase 1.1 stub. Implementation arrives in Phase 1.4.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`run` not yet implemented (Phase 1.4)");
}
