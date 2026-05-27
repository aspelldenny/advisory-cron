//! `advisory-cron register` — Phase 1.3 implementation.
//!
//! Loads config, generates launchd plist, writes to `~/Library/LaunchAgents/`,
//! bootstraps via `launchctl`.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::{env, fs, path::PathBuf};

use crate::config::{Config, ScheduleConfig};
use crate::launchd::{
    LaunchctlClient, RealLaunchctl, default_launch_agents_dir, generate_plist, plist_path_for,
};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Cron expression (`M H * * *` daily form) — overrides config.schedule when present.
    /// (V2: relaxed from required `String` to `Option<String>` so config-driven schedule
    /// works without redundant CLI flag.)
    #[arg(long)]
    pub schedule: Option<String>,

    /// Label suffix (full label = com.advisorycron.<label>).
    #[arg(long)]
    pub label: String,

    /// Override default config path (default: ~/.config/advisory-cron/config.toml).
    /// (V2 new — placed inside Args struct, NOT on Commands enum, per newtype dispatch pattern
    /// confirmed by Turn 1 [O1.1].)
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub async fn run(args: Args) -> Result<u8> {
    let home = home_dir().context("failed to resolve $HOME")?;
    let launch_agents_dir = default_launch_agents_dir(&home);
    run_with_deps(args, &RealLaunchctl, &launch_agents_dir, &home).await
}

/// Test-friendly entry — injects LaunchctlClient + LaunchAgents dir.
pub async fn run_with_deps<L: LaunchctlClient>(
    args: Args,
    launchctl: &L,
    launch_agents_dir: &std::path::Path,
    home: &std::path::Path,
) -> Result<u8> {
    // 1. Resolve config path.
    let config_path = args
        .config
        .unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"));

    // 2. Load config (may return exit 2 if invalid).
    let mut config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load config: {e:#}");
            return Ok(2);
        }
    };

    // 3. Apply --schedule CLI override (parse as `M H * * *` simple form).
    if let Some(cron_expr) = &args.schedule {
        config.schedule = ScheduleConfig::Cron {
            cron: cron_expr.clone(),
        };
        // generate_plist will validate via parse_simple_cron.
    }

    // 4. Validate label (defense-in-depth; generate_plist also checks).
    if args.label.is_empty() {
        eprintln!("error: --label must not be empty");
        return Ok(1);
    }

    // 5. Resolve self-exe path.
    let self_exe = env::current_exe().context("failed to resolve current executable path")?;

    // 6. Generate plist XML.
    let plist_xml = match generate_plist(&config, &args.label, &self_exe) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to generate plist: {e:#}");
            return Ok(2);
        }
    };

    // 7. Write plist file.
    fs::create_dir_all(launch_agents_dir)
        .with_context(|| format!("failed to create {}", launch_agents_dir.display()))?;
    let plist_path = plist_path_for(&args.label, launch_agents_dir);
    if let Err(e) = fs::write(&plist_path, &plist_xml) {
        eprintln!(
            "error: failed to write plist to {}: {e:#}",
            plist_path.display()
        );
        return Ok(3);
    }

    // 8. Bootstrap via launchctl.
    if let Err(e) = launchctl.bootstrap(&plist_path) {
        eprintln!("error: launchctl bootstrap failed: {e:#}");
        // Plist file already written; leave in place so user can inspect / retry.
        return Ok(3);
    }

    println!("registered launchd job: com.advisorycron.{}", args.label);
    println!("  plist: {}", plist_path.display());
    Ok(0)
}

fn home_dir() -> Result<PathBuf> {
    let raw = env::var("HOME").ok().filter(|s| !s.is_empty());
    match raw {
        Some(s) => Ok(PathBuf::from(s)),
        None => {
            bail!("$HOME env var is not set; cannot resolve default config / launch_agents path")
        }
    }
}
