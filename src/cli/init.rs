//! `advisory-cron init` — write default config to ~/.config/advisory-cron/config.toml.
//! Phase 1.1 stub. Implementation arrives in Phase 1.2.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Overwrite existing config file if present.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`init` not yet implemented (Phase 1.2)");
}
