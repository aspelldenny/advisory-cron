//! `advisory-cron init` — write default config to ~/.config/advisory-cron/config.toml.
//!
//! Phase 1.2 — first real subcommand implementation. Wires `Config::write_default`.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::{env, path::PathBuf};

use crate::config::Config;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Overwrite existing config file if present.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: Args) -> Result<u8> {
    let home = home_dir().context("failed to resolve $HOME")?;
    let config_path = home.join(".config/advisory-cron/config.toml");

    match Config::write_default(&config_path, &home, args.force) {
        Ok(()) => {
            println!("wrote default config to {}", config_path.display());
            Ok(0)
        }
        Err(e) => {
            // "config exists without --force" + parse/IO failures → exit 2
            // per ARCHITECTURE.md §CLI surface exit codes.
            eprintln!("error: {e:#}");
            Ok(2)
        }
    }
}

/// Resolve `$HOME` from env. Returns error if unset (rare on macOS / Linux dev shells).
fn home_dir() -> Result<PathBuf> {
    let raw = env::var("HOME").ok().filter(|s| !s.is_empty());
    match raw {
        Some(s) => Ok(PathBuf::from(s)),
        None => bail!("$HOME env var is not set; cannot resolve default config path"),
    }
}
