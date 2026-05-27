//! `advisory-cron init` — thin CLI shell over `core::init::run`.
//!
//! Phase 1.2 logic extracted to `src/core/init.rs` in Phase 1.7 (P006).
//! This module handles Args parsing + stdout/stderr + exit code mapping only.

use crate::core::init::{InitArgs, run as core_run};
use anyhow::Result;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Overwrite existing config file if present.
    #[arg(long)]
    pub force: bool,
    /// Override default config path (default: ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
}

/// Returns `Result<u8>` matching dispatch contract.
/// Exit codes:
/// - 0: config written successfully.
/// - 2: config already exists without --force (or other config/IO error).
pub async fn run(args: Args) -> Result<u8> {
    match core_run(InitArgs {
        force: args.force,
        config_path: args.config,
    }) {
        Ok(output) => {
            println!("wrote default config to {}", output.config_path.display());
            Ok(0)
        }
        Err(e) => {
            // Preserve P002 ship behavior: all errors → exit 2.
            // (write_default returns Err on "already exists + no force" AND IO failures.)
            eprintln!("error: {e:#}");
            Ok(2)
        }
    }
}
