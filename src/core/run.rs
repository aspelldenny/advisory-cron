//! Core run logic — fire the configured task (with optional retry loop) + append heartbeat.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::run` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.
//!
//! Phase 2.2 (P009): retry loop wraps `runner::fire_task`. 1 heartbeat per attempt.
//! Alert fires AT MOST ONCE per invocation, AFTER the retry loop (INV-20).

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
    /// Whether the last attempt's heartbeat was successfully appended.
    /// Under retry, reflects the LAST attempt only (Tầng 2 Worker decision — see
    /// docs/discoveries/P009.md). `true` does NOT guarantee all prior-attempt
    /// heartbeats were written; each attempt's append result is independent.
    pub heartbeat_appended: bool,
}

/// Retry decision predicate. Per BACKLOG Phase 2.2 spec:
/// - `exit_code ∈ 1..=127` → retryable (normal process error exit)
/// - `exit_code ≥ 128` → NOT retryable (signal-killed: 130=SIGINT, 137=SIGKILL,
///   143=SIGTERM; convention `128 + signal_num`). Signal kills are operator
///   actions or OOM, not transient errors — retry would fight the operator.
/// - `exit_code == 0` → success (caller checks before calling this fn)
/// - `exit_code == -1` (spawn failure sentinel per INV-14 + P004 contract) →
///   NOT retryable. Spawn failure = command path missing / not executable =
///   deploy/config bug. Retry won't help; surface immediately.
fn is_retryable(exit_code: i32) -> bool {
    (1..=127).contains(&exit_code)
}

/// V2 (per Architect Turn 1 RESPOND, internal-resolution pattern):
/// - Resolves config path via `default_config_path()` if Args.config_path is None.
/// - No LaunchctlClient injection needed (run touches filesystem + child process only).
///
/// Phase 2.2: wraps `runner::fire_task` in a bounded retry loop when `[retry]` is
/// configured. When `[retry]` is absent, behaves identically to Phase 2.1 (single
/// attempt, alert on fail). INV-20 governs the retry boundary.
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

    // 4. Phase 2.2 — Retry loop (wraps single-fire `runner::fire_task` from Phase 1.4).
    //
    // `max_attempts == 1` (or [retry] absent) = single-fire behavior preserved.
    // Backwards-compat: None → unwrap_or((1, 0)) → 1 attempt, 0s backoff.
    //
    // Per BACKLOG Phase 2.2: exit code 1-127 retryable; ≥128 (signal) and -1 (spawn-fail) NOT.
    // Heartbeat schema unchanged — 1 record per attempt (P009 Architect decision §Giải pháp item 3).
    // Alert wiring moved OUTSIDE loop — 1 alert max per `run` invocation regardless of attempt count.
    let (max_attempts, backoff_secs) = config
        .retry
        .as_ref()
        .map(|r| (r.max_attempts.max(1), r.backoff_secs))
        .unwrap_or((1, 0));

    let mut final_exit_code: i32 = 0;
    let mut final_stdout_tail = String::new();
    let mut final_stderr_tail = String::new();
    let mut final_duration_ms: u64 = 0;
    let mut heartbeat_appended = false;

    for attempt in 1..=max_attempts {
        let started_for_spawn_fail = std::time::Instant::now();
        let fire_result = runner::fire_task(&config).await;

        // TWO-MATCH HEARTBEAT-COMPLETENESS INVARIANT (P009 V2 / Constraint #12):
        // First match borrows `fire_result` to build HeartbeatRecord (including
        // synthesizing exit_code=-1 for Err spawn-fail per INV-14). THEN append
        // heartbeat BETWEEN the two matches. THEN second match consumes `fire_result`
        // to extract the output quadruple. This ensures spawn-fail iterations STILL
        // write a heartbeat JSONL line (with exit_code=-1). DO NOT short-circuit on
        // fire_result.is_err() — that would silently lose heartbeat data on spawn-fail.
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

        // Append heartbeat per attempt (schema unchanged — 1 JSONL line per fire).
        // Heartbeat write fail is a warning, not a run failure — matches P004 behavior.
        heartbeat_appended = heartbeat::append(&config.heartbeat.log_path, &record).is_ok();

        // Extract output variables. Spawn-fail (Err arm) → exit_code=-1 (INV-14 sentinel).
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

        // Capture as "final" — overwritten each iteration; post-loop values
        // reflect the last attempt that ran.
        final_exit_code = exit_code;
        final_stdout_tail = stdout_tail;
        final_stderr_tail = stderr_tail;
        final_duration_ms = duration_ms;

        // Loop exit decisions:
        if exit_code == 0 {
            // Success — stop retrying.
            break;
        }
        if !is_retryable(exit_code) {
            // Signal-killed (≥128) or spawn-failure (-1) — retry won't help.
            tracing::warn!(
                attempt,
                exit_code,
                "task fire produced non-retryable exit code, not retrying"
            );
            break;
        }
        if attempt == max_attempts {
            // Exhausted — stop and let post-loop alert fire.
            break;
        }
        // Transient failure — backoff then retry.
        tracing::info!(
            attempt,
            next_attempt = attempt + 1,
            backoff_secs,
            exit_code,
            "task fire failed with retryable exit code, sleeping before retry"
        );
        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
    }

    // Phase 2.1 alert — re-located by P009 from INSIDE single-fire body to AFTER
    // the retry loop. Invariant (INV-20 sub-rule 4): EXACTLY 1 alert max per
    // `advisory-cron run` invocation, regardless of retry count.
    // - Task succeeded on some attempt → final_exit_code == 0 → ZERO alerts.
    // - All retries exhausted OR signal-killed early → ONE alert.
    // Env-var-at-call-site (V2 — Worker Turn 1 recommendation, Architect ACCEPT).
    // `ADVISORY_CRON_TG_API_BASE` is a TEST-ONLY seam; `alert.rs` itself stays
    // env-free for unit-testability (INV-19 §Implementation).
    if final_exit_code != 0
        && let Some(alert_cfg) = config.alert.as_ref().and_then(|a| a.telegram.as_ref())
    {
        match crate::alert::TelegramAlert::from_config(Some(alert_cfg)) {
            Ok(Some(alert)) => {
                let api_base = std::env::var("ADVISORY_CRON_TG_API_BASE")
                    .unwrap_or_else(|_| "https://api.telegram.org".to_string());
                let msg = crate::alert::format_failure_message(
                    &label,
                    final_exit_code,
                    final_duration_ms,
                    &final_stderr_tail,
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
        exit_code: final_exit_code,
        duration_ms: final_duration_ms,
        stdout_tail: final_stdout_tail,
        stderr_tail: final_stderr_tail,
        heartbeat_appended,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // is_retryable — boundary tests (Task 2, P009)
    // ---------------------------------------------------------------------------

    #[test]
    fn is_retryable_exit_1_true() {
        assert!(is_retryable(1));
    }

    #[test]
    fn is_retryable_exit_127_true() {
        assert!(is_retryable(127));
    }

    #[test]
    fn is_retryable_exit_0_false() {
        // Success — caller checks exit_code == 0 before retry decision, but predicate is false.
        assert!(!is_retryable(0));
    }

    #[test]
    fn is_retryable_exit_128_false() {
        // Signal boundary: 128 = lowest signal-killed code.
        assert!(!is_retryable(128));
    }

    #[test]
    fn is_retryable_exit_130_false() {
        // SIGINT (Ctrl+C): 128 + 2 = 130.
        assert!(!is_retryable(130));
    }

    #[test]
    fn is_retryable_exit_137_false() {
        // SIGKILL: 128 + 9 = 137.
        assert!(!is_retryable(137));
    }

    #[test]
    fn is_retryable_exit_143_false() {
        // SIGTERM: 128 + 15 = 143.
        assert!(!is_retryable(143));
    }

    #[test]
    fn is_retryable_exit_neg1_false() {
        // Spawn-failure sentinel (INV-14 / P004 contract).
        assert!(!is_retryable(-1));
    }
}
