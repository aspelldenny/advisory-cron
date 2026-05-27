//! `advisory-cron run` — thin CLI shell over `core::run::run`.
//!
//! Phase 1.4 logic extracted to `src/core/run.rs` in Phase 1.7 (P006).
//! This module handles Args parsing + stdout/stderr + exit code mapping only.

use crate::core::run::{RunArgs, run as core_run};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Path to config file (overrides default ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
}

/// Returns `Result<u8>` matching dispatch contract.
/// Exit codes per ARCHITECTURE.md §CLI surface exit codes:
/// - 0: task fired and exited 0.
/// - 2: config not found / invalid OR $HOME unset.
/// - 4: task fired non-zero OR spawn failed.
pub async fn run(args: Args) -> Result<u8> {
    match core_run(RunArgs {
        config_path: args.config,
    })
    .await
    {
        Ok(output) => {
            let exit = if output.exit_code == 0 { 0u8 } else { 4u8 };
            Ok(exit)
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            Ok(2)
        }
    }
}
