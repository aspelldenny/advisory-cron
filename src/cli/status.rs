//! `advisory-cron status` — thin CLI shell over `core::status::run`.
//!
//! Phase 1.5 logic extracted to `src/core/status.rs` in Phase 1.7 (P006).
//! This module handles Args parsing + stdout/stderr rendering + exit code mapping.
//!
//! `StatusReport` and parsing helpers live in `src/core/status.rs`.

use crate::core::status::{StatusArgs, StatusReport, run as core_run};
use crate::scheduler::PlatformScheduler;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Label to query. Falls back to config.task.label, then "advisory-cron".
    #[arg(long)]
    pub label: Option<String>,

    /// Path to config file (overrides default ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Output as JSON (machine-readable). Default: human-readable text.
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Number of recent heartbeats to show. Default: 5.
    #[arg(long, default_value_t = 5)]
    pub last: usize,
}

/// Returns `Result<u8>` matching dispatch contract.
/// Exit codes per ARCHITECTURE.md §CLI surface exit codes:
/// - 0: status rendered successfully.
/// - 1: invalid label.
/// - 2: config not found / invalid.
pub async fn run(args: Args) -> Result<u8> {
    let scheduler = PlatformScheduler;
    match core_run(
        StatusArgs {
            label: args.label,
            config_path: args.config,
            last: args.last,
        },
        &scheduler,
    ) {
        Ok(report) => {
            if args.json {
                let json = serde_json::to_string_pretty(&report)
                    .map_err(|e| anyhow::anyhow!("failed to serialize StatusReport: {e}"))?;
                println!("{json}");
            } else {
                render_human(&report);
            }
            Ok(0)
        }
        Err(e) => {
            let msg = format!("{e:#}");
            eprintln!("error: {msg}");
            if msg.contains("invalid label") {
                Ok(1)
            } else {
                Ok(2)
            }
        }
    }
}

fn render_human(report: &StatusReport) {
    println!("advisory-cron status — label: {}", report.label);
    let plist_status = if report.plist_loaded {
        "loaded"
    } else {
        "not loaded"
    };
    println!("  Plist: {plist_status}");
    let next_fire_display = match (report.plist_loaded, &report.next_fire) {
        (false, _) => "n/a (not loaded)".to_string(),
        (true, Some(s)) => s.clone(),
        (true, None) => "unknown (launchctl format not recognized)".to_string(),
    };
    println!("  Next fire: {next_fire_display}");
    println!();

    if report.last_runs.is_empty() {
        println!(
            "Recent heartbeats: No heartbeats yet (no fires recorded at {})",
            report.heartbeat_log_path
        );
    } else {
        println!("Recent heartbeats (last {}):", report.last_runs.len());
        for rec in report.last_runs.iter().rev() {
            println!(
                "  [{ts}] exit={exit} duration={dur}ms",
                ts = rec.ts.to_rfc3339(),
                exit = rec.exit_code,
                dur = rec.duration_ms,
            );
            println!(
                "      stdout: {}",
                tail_first_n_or_empty(&rec.stdout_tail, 80)
            );
            println!(
                "      stderr: {}",
                tail_first_n_or_empty(&rec.stderr_tail, 80)
            );
        }
    }
}

fn tail_first_n_or_empty(s: &str, n: usize) -> String {
    if s.is_empty() {
        "(empty)".to_string()
    } else if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while end < s.len() && !s.is_char_boundary(end) {
            end += 1;
        }
        format!("{}...", &s[..end])
    }
}

// Keep render_human helper tests here since they are UI-layer concerns.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_first_n_or_empty_returns_marker_for_empty() {
        assert_eq!(tail_first_n_or_empty("", 80), "(empty)");
    }

    #[test]
    fn tail_first_n_or_empty_truncates_long_strings() {
        let s = "a".repeat(200);
        let result = tail_first_n_or_empty(&s, 80);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 84);
    }
}
