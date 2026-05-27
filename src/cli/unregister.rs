//! `advisory-cron unregister` — Phase 1.3 implementation.
//!
//! Idempotent: succeeds even if label not currently loaded or plist file already absent.
//! Exit 3 only on real launchctl failure paired with plist removal failure.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::{env, fs, io, path::PathBuf};

use crate::launchd::{LaunchctlClient, RealLaunchctl, default_launch_agents_dir, plist_path_for};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Label suffix (full label = com.advisorycron.<label>).
    #[arg(long)]
    pub label: String,

    /// (V2 reserved — currently unused; declared for CLI symmetry with `register` per Heads-up #2.
    /// Placed inside Args struct, NOT on Commands enum, per newtype dispatch confirmed Turn 1 [O1.1].)
    #[arg(long)]
    pub _config: Option<PathBuf>,
}

pub async fn run(args: Args) -> Result<u8> {
    let home = home_dir().context("failed to resolve $HOME")?;
    let launch_agents_dir = default_launch_agents_dir(&home);
    run_with_deps(args, &RealLaunchctl, &launch_agents_dir).await
}

/// Test-friendly entry — injects LaunchctlClient + LaunchAgents dir.
pub async fn run_with_deps<L: LaunchctlClient>(
    args: Args,
    launchctl: &L,
    launch_agents_dir: &std::path::Path,
) -> Result<u8> {
    if args.label.is_empty() {
        eprintln!("error: --label must not be empty");
        return Ok(1);
    }

    // 1. Try launchctl bootout. If fails (likely "not loaded"), warn but continue.
    //    V2 (Anchor #17 empirical): expected error message is "Boot-out failed: 3: No such process"
    //    when label was never bootstrapped. Do NOT branch on substring — any Err goes through warn.
    let bootout_result = launchctl.bootout(&args.label);
    if let Err(ref e) = bootout_result {
        eprintln!(
            "warning: launchctl bootout: {e:#} (label may not be loaded; proceeding to remove plist)"
        );
    }

    // 2. Try plist file removal. NotFound → warn, continue. Other IO → potential exit 3.
    let plist_path = plist_path_for(&args.label, launch_agents_dir);
    let remove_result = fs::remove_file(&plist_path);
    match remove_result {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            eprintln!(
                "warning: plist file already absent at {}",
                plist_path.display()
            );
        }
        Err(e) => {
            eprintln!(
                "error: failed to remove plist at {}: {e:#}",
                plist_path.display()
            );
            // Hard failure on plist removal — exit 3 regardless of bootout result.
            return Ok(3);
        }
    }

    println!("unregistered launchd job: com.advisorycron.{}", args.label);
    Ok(0)
}

fn home_dir() -> Result<PathBuf> {
    let raw = env::var("HOME").ok().filter(|s| !s.is_empty());
    match raw {
        Some(s) => Ok(PathBuf::from(s)),
        None => bail!("$HOME env var is not set"),
    }
}
