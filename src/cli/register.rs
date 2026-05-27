//! `advisory-cron register` — thin CLI shell over `core::register::run`.
//!
//! Phase 1.3 logic extracted to `src/core/register.rs` in Phase 1.7 (P006).
//! This module handles Args parsing + stdout/stderr + exit code mapping only.

use crate::core::register::{RegisterArgs, run as core_run};
use crate::launchd::RealLaunchctl;
use anyhow::Result;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Cron expression (`M H * * *` daily form) — overrides config.schedule when present.
    #[arg(long)]
    pub schedule: Option<String>,

    /// Label suffix (full label = com.advisorycron.<label>).
    #[arg(long)]
    pub label: String,

    /// Override default config path (default: ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
}

/// Returns `Result<u8>` matching dispatch contract.
/// Exit codes per ARCHITECTURE.md §CLI surface exit codes:
/// - 0: registered successfully.
/// - 1: invalid label.
/// - 2: config not found / invalid, or cron-parse fail.
/// - 3: plist write / bootstrap fail.
pub async fn run(args: Args) -> Result<u8> {
    // Pre-flight label validation (INV-12 — before delegating to core).
    if args.label.is_empty() {
        eprintln!("error: --label must not be empty");
        return Ok(1);
    }

    let client = RealLaunchctl;
    match core_run(
        RegisterArgs {
            label: args.label,
            schedule: args.schedule,
            config_path: args.config,
        },
        &client,
    ) {
        Ok(output) => {
            println!("registered launchd job: com.advisorycron.{}", output.label);
            println!("  plist: {}", output.plist_path.display());
            Ok(0)
        }
        Err(e) => {
            let msg = format!("{e:#}");
            eprintln!("error: {msg}");
            // Map errors to exit codes per ARCHITECTURE.md:71-77.
            if msg.contains("$HOME") {
                Ok(1)
            } else if msg.contains("load config")
                || msg.contains("generate plist")
                || msg.contains("cron")
                || msg.contains("invalid label")
            {
                Ok(2)
            } else {
                Ok(3)
            }
        }
    }
}
