//! Core run logic — fire the configured task once + append heartbeat.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::run` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.

use crate::config::Config;
use crate::core::config_path::default_config_path;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::runner;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RunArgs {
    /// Override default config path.
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunOutput {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub heartbeat_appended: bool,
}

/// V2 (per Architect Turn 1 RESPOND, internal-resolution pattern):
/// - Resolves config path via `default_config_path()` if Args.config_path is None.
/// - No LaunchctlClient injection needed (run touches filesystem + child process only).
pub async fn run(args: RunArgs) -> Result<RunOutput> {
    // 1. Resolve config path. Bail! on $HOME unset (Constraint #16).
    let config_path = match args.config_path {
        Some(p) => p,
        None => default_config_path().context("failed to resolve default config path")?,
    };

    // 2. Load config.
    let config = Config::load(&config_path)
        .with_context(|| format!("failed to load config at {}", config_path.display()))?;

    // 3. Resolve heartbeat label.
    let label = config
        .task
        .label
        .clone()
        .unwrap_or_else(|| "advisory-cron".to_string());

    // 4. Fire task. Both Ok and Err build a heartbeat record.
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

    // 5. Append heartbeat. Heartbeat write fail is a warning, not a run failure.
    let heartbeat_appended = heartbeat::append(&config.heartbeat.log_path, &record).is_ok();

    // 6. Build RunOutput.
    let (exit_code, stdout_tail, stderr_tail, duration_ms) = match fire_result {
        Ok(rr) => (
            rr.exit_code,
            heartbeat::tail_utf8(&rr.stdout, 1024),
            heartbeat::tail_utf8(&rr.stderr, 1024),
            rr.duration_ms,
        ),
        Err(_) => (
            -1,
            String::new(),
            record.stderr_tail.clone(),
            record.duration_ms,
        ),
    };

    // Phase 2.1 — Telegram alert on fail (best-effort, INV-19 boundary).
    // Insertion point V2 (Option A): AFTER `match fire_result` (all vars in scope),
    // BEFORE `Ok(RunOutput)` return.
    if exit_code != 0
        && let Some(alert_cfg) = config.alert.as_ref().and_then(|a| a.telegram.as_ref())
    {
        match crate::alert::TelegramAlert::from_config(Some(alert_cfg)) {
            Ok(Some(alert)) => {
                // Env-var-at-call-site (V2 — Worker Turn 1 recommendation, Architect ACCEPT).
                // `ADVISORY_CRON_TG_API_BASE` is a TEST-ONLY seam. Production: env var
                // unset → default base. `alert.rs` itself stays env-free for
                // unit-testability (INV-19 §Implementation).
                let api_base = std::env::var("ADVISORY_CRON_TG_API_BASE")
                    .unwrap_or_else(|_| "https://api.telegram.org".to_string());
                let msg = crate::alert::format_failure_message(
                    &label,       // bound from line 44
                    exit_code,    // bound from match fire_result
                    duration_ms,  // bound from match fire_result
                    &stderr_tail, // bound from match fire_result
                );
                if let Err(e) = alert.send_with_base(&api_base, &msg).await {
                    tracing::warn!(
                        error = %e,
                        "telegram alert send failed (best-effort, swallowing)"
                    );
                }
            }
            Ok(None) => {} // unreachable given outer Some check, but defensive
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "telegram alert config invalid (best-effort, swallowing)"
                );
            }
        }
    }

    Ok(RunOutput {
        exit_code,
        duration_ms,
        stdout_tail,
        stderr_tail,
        heartbeat_appended,
    })
}
