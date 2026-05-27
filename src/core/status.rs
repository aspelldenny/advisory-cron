//! Core status logic — query launchd state + read heartbeats.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::status` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.
//!
//! `StatusReport` is pub (moved here from `src/cli/status.rs` per P006 Decision 2).

use crate::config::Config;
use crate::core::config_path::default_config_path;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::launchd::{LaunchctlClient, LaunchctlPrintOutput};
use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct StatusArgs {
    /// Label to query. Falls back to config.task.label, then "advisory-cron".
    pub label: Option<String>,
    /// Override default config path.
    pub config_path: Option<PathBuf>,
    /// Number of recent heartbeats to include.
    pub last: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusReport {
    pub label: String,
    pub plist_loaded: bool,
    pub next_fire: Option<String>,
    pub heartbeat_log_path: String,
    pub last_runs: Vec<HeartbeatRecord>,
}

/// V2 (per Architect Turn 1 RESPOND, internal-resolution pattern):
/// - Resolves config path internally via `default_config_path()` if Args.config_path is None.
/// - `&client: &L: LaunchctlClient` injected (needed for launchctl print).
pub fn run<L: LaunchctlClient>(args: StatusArgs, client: &L) -> Result<StatusReport> {
    // 1. Resolve config path.
    let config_path = match args.config_path {
        Some(p) => p,
        None => default_config_path().context("failed to resolve default config path")?,
    };

    // 2. Load config.
    let config = Config::load(&config_path)
        .with_context(|| format!("failed to load config at {}", config_path.display()))?;

    // 3. Resolve label: arg > config.task.label > literal "advisory-cron".
    let label = args
        .label
        .or_else(|| config.task.label.clone())
        .unwrap_or_else(|| "advisory-cron".to_string());

    // 4. Validate label (INV-12).
    if !is_valid_label(&label) {
        anyhow::bail!(
            "invalid label {:?} — must be ASCII alphanumeric + '-' + '_'",
            label
        );
    }

    // 5. Query launchctl.
    let print_result = match client.print(&label) {
        Ok(o) => o,
        Err(_err) => LaunchctlPrintOutput {
            raw_stdout: String::new(),
            not_loaded: true,
        },
    };

    // 6. Parse next-fire schedule.
    let next_fire = if print_result.not_loaded {
        None
    } else {
        parse_next_fire(&print_result.raw_stdout)
    };

    // 7. Read recent heartbeats.
    let last_runs =
        heartbeat::read_last_n(&config.heartbeat.log_path, args.last).unwrap_or_default();

    Ok(StatusReport {
        label,
        plist_loaded: !print_result.not_loaded,
        next_fire,
        heartbeat_log_path: config.heartbeat.log_path.display().to_string(),
        last_runs,
    })
}

/// Validate label: ASCII alphanumeric + '-' + '_', non-empty. (INV-12)
pub(crate) fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Parse configured recurrence from `launchctl print` stdout.
///
/// Scans for `"Hour" =>` and `"Minute" =>` patterns inside the descriptor block.
/// Returns:
/// - `Some("daily at HH:MM")` when both found.
/// - `Some("daily at HH:00")` when only Hour.
/// - `Some("hourly at :MM")` when only Minute.
/// - `None` when neither found.
pub(crate) fn parse_next_fire(raw_stdout: &str) -> Option<String> {
    let mut hour: Option<u32> = None;
    let mut minute: Option<u32> = None;

    for line in raw_stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"Hour\"") {
            let num_str = rest
                .trim_start_matches(|c: char| c == '=' || c == '>' || c.is_whitespace())
                .split_whitespace()
                .next()
                .unwrap_or("");
            if let Ok(h) = num_str.parse::<u32>()
                && h < 24
            {
                hour = Some(h);
            }
        } else if let Some(rest) = trimmed.strip_prefix("\"Minute\"") {
            let num_str = rest
                .trim_start_matches(|c: char| c == '=' || c == '>' || c.is_whitespace())
                .split_whitespace()
                .next()
                .unwrap_or("");
            if let Ok(m) = num_str.parse::<u32>()
                && m < 60
            {
                minute = Some(m);
            }
        }
    }

    match (hour, minute) {
        (Some(h), Some(m)) => Some(format!("daily at {h:02}:{m:02}")),
        (Some(h), None) => Some(format!("daily at {h:02}:00")),
        (None, Some(m)) => Some(format!("hourly at :{m:02}")),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_valid_label ----

    #[test]
    fn is_valid_label_allows_alnum_dash_underscore() {
        assert!(is_valid_label("advisory-scan-daily"));
        assert!(is_valid_label("test_label_1"));
        assert!(is_valid_label("a1b2"));
    }

    #[test]
    fn is_valid_label_rejects_path_traversal_and_empty() {
        assert!(!is_valid_label(""));
        assert!(!is_valid_label("../etc/passwd"));
        assert!(!is_valid_label("foo bar"));
        assert!(!is_valid_label("foo;rm"));
        assert!(!is_valid_label("foo.bar"));
    }

    // ---- parse_next_fire ----

    const MACOS15_FIXTURE: &str = "gui/501/com.advisorycron.probe-p005 = {\n\
        \tstate = not running\n\
        \tevent triggers = {\n\
        \t\tcom.advisorycron.probe-p005.268435522 => {\n\
        \t\t\tstream = com.apple.launchd.calendarinterval\n\
        \t\t\tdescriptor = {\n\
        \t\t\t\t\"Minute\" => 0\n\
        \t\t\t\t\"Hour\" => 9\n\
        \t\t\t}\n\
        \t\t}\n\
        \t}\n\
        }";

    #[test]
    fn parse_next_fire_extracts_daily_from_macos15_descriptor() {
        let result = parse_next_fire(MACOS15_FIXTURE);
        assert_eq!(result, Some("daily at 09:00".to_string()));
    }

    #[test]
    fn parse_next_fire_returns_none_on_unrecognized_format() {
        let sample = "some-totally-unknown launchctl output\nfoo = bar\n";
        assert_eq!(parse_next_fire(sample), None);
    }

    #[test]
    fn parse_next_fire_handles_empty_input() {
        assert_eq!(parse_next_fire(""), None);
    }

    #[test]
    fn parse_next_fire_handles_hour_only() {
        let sample = "descriptor = {\n\t\"Hour\" => 14\n}";
        assert_eq!(parse_next_fire(sample), Some("daily at 14:00".to_string()));
    }

    #[test]
    fn parse_next_fire_handles_minute_only() {
        let sample = "descriptor = {\n\t\"Minute\" => 30\n}";
        assert_eq!(parse_next_fire(sample), Some("hourly at :30".to_string()));
    }

    #[test]
    fn parse_next_fire_zero_pads_single_digit() {
        let sample = "descriptor = {\n\t\"Hour\" => 5\n\t\"Minute\" => 7\n}";
        assert_eq!(parse_next_fire(sample), Some("daily at 05:07".to_string()));
    }

    #[test]
    fn parse_next_fire_rejects_out_of_range_hour() {
        let sample = "descriptor = {\n\t\"Hour\" => 99\n\t\"Minute\" => 0\n}";
        assert_eq!(parse_next_fire(sample), Some("hourly at :00".to_string()));
    }
}
