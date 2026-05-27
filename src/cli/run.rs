//! `advisory-cron run` — fire the configured task once + append heartbeat.
//!
//! Exit codes (see docs/ARCHITECTURE.md §CLI surface exit codes):
//! - 0: task fired and exited 0
//! - 2: config not found / invalid OR $HOME unset (default path unresolvable)
//! - 4: task fired non-zero OR spawn failed (heartbeat distinguishes via exit_code field:
//!   -1 for spawn-fail / signal-kill, >0 for task non-zero exit)

use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::path::PathBuf;

use crate::config::Config;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::runner;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Path to config file (overrides default ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub async fn run(args: Args) -> Result<u8> {
    // 1. Resolve config path. If --config not given, default to ~/.config/advisory-cron/config.toml.
    //    bail! on $HOME unset (mirror src/cli/register.rs home_dir() pattern; never silently
    //    fall back to "/" — that would either write to filesystem root or fail with a cryptic
    //    permission error). See P004 V2 Turn 1 Architect Response.
    let config_path = match args.config {
        Some(p) => p,
        None => match default_config_path() {
            Ok(p) => p,
            Err(err) => {
                eprintln!("error: failed to resolve default config path: {err:#}");
                return Ok(2);
            }
        },
    };

    // 2. Load config — exit 2 on failure per ARCHITECTURE.md:74.
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: failed to load config {config_path:?}: {err:#}");
            return Ok(2);
        }
    };

    // 3. Resolve heartbeat label (defaults to "advisory-cron" if config.task.label unset).
    let label = config
        .task
        .label
        .clone()
        .unwrap_or_else(|| "advisory-cron".to_string());

    // 4. Fire task. Both Ok and Err build a heartbeat record.
    //    started_for_spawn_fail is a separate timer for the spawn-fail case because
    //    runner::fire_task returns Err before measuring duration internally.
    let started_for_spawn_fail = std::time::Instant::now();
    let fire_result = runner::fire_task(&config).await;

    let record = match &fire_result {
        Ok(rr) => HeartbeatRecord {
            ts: Utc::now(),
            label: label.clone(),
            exit_code: rr.exit_code,
            duration_ms: rr.duration_ms,
            stdout_tail: heartbeat::tail_utf8(&rr.stdout, 1024),
            stderr_tail: heartbeat::tail_utf8(&rr.stderr, 1024),
        },
        Err(spawn_err) => HeartbeatRecord {
            ts: Utc::now(),
            label: label.clone(),
            exit_code: -1,
            duration_ms: started_for_spawn_fail.elapsed().as_millis() as u64,
            stdout_tail: String::new(),
            stderr_tail: format!("spawn failed: {spawn_err:#}"),
        },
    };

    // 5. Append heartbeat. Per ARCHITECTURE.md:269, heartbeat write fail is a warning,
    //    NOT a run failure — task already ran, operator needs the exit code regardless.
    if let Err(hb_err) = heartbeat::append(&config.heartbeat.log_path, &record) {
        eprintln!("warning: heartbeat write failed: {hb_err:#}");
    }

    // 6. Resolve exit code per phiếu §Giải pháp module 3 step 6.
    //    Exit 4 collapses "task ran but failed" + "task could not be spawned" — both are
    //    "fire failed from operator perspective". Heartbeat exit_code field distinguishes:
    //    -1 = spawn-fail/signal-kill, >0 = task non-zero exit.
    let exit = match &fire_result {
        Ok(rr) if rr.exit_code == 0 => 0u8,
        Ok(_) => 4u8,  // task ran but exited non-zero
        Err(_) => 4u8, // spawn failed
    };
    Ok(exit)
}

/// Resolve default config path. Bail! when `$HOME` is unset or empty — never silently fall back
/// to `/`. Mirrors `src/cli/register.rs` `home_dir()` helper pattern (per P004 V2
/// Turn 1 Architect Response).
fn default_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(anyhow::Error::from)
        .context("$HOME environment variable is not set")?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home).join(".config/advisory-cron/config.toml"))
}
