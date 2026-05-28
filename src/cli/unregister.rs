//! `advisory-cron unregister` — thin CLI shell over `core::unregister::run`.
//!
//! Phase 1.3 logic extracted to `src/core/unregister.rs` in Phase 1.7 (P006).
//! This module handles Args parsing + stdout/stderr + exit code mapping only.

use crate::core::unregister::{UnregisterArgs, run as core_run};
use crate::scheduler::PlatformScheduler;
use anyhow::Result;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Label suffix (full label = com.advisorycron.<label>).
    #[arg(long)]
    pub label: String,

    /// (Reserved — currently unused; declared for CLI symmetry with `register`.)
    #[arg(long)]
    pub config: Option<PathBuf>,
}

/// Returns `Result<u8>` matching dispatch contract.
/// Exit codes per ARCHITECTURE.md §CLI surface exit codes:
/// - 0: unregistered (or idempotently absent).
/// - 1: invalid label.
/// - 3: plist removal failed (hard IO error).
pub async fn run(args: Args) -> Result<u8> {
    // Pre-flight label validation (INV-12 — before delegating to core).
    if args.label.is_empty() {
        eprintln!("error: --label must not be empty");
        return Ok(1);
    }

    let scheduler = PlatformScheduler;
    match core_run(
        UnregisterArgs {
            label: args.label.clone(),
            config_path: args.config,
        },
        &scheduler,
    ) {
        Ok(output) => {
            if !output.was_loaded {
                eprintln!(
                    "warning: launchctl bootout: label may not be loaded; proceeding to remove plist"
                );
            }
            if !output.plist_existed {
                eprintln!(
                    "warning: plist file already absent for label {}",
                    args.label
                );
            }
            println!("unregistered launchd job: com.advisorycron.{}", args.label);
            Ok(0)
        }
        Err(e) => {
            let msg = format!("{e:#}");
            eprintln!("error: {msg}");
            if msg.contains("invalid label") || msg.contains("$HOME") {
                Ok(1)
            } else {
                Ok(3)
            }
        }
    }
}
