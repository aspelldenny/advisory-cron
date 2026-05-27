//! `advisory-cron status` — show next scheduled fire time + last heartbeat.
//!
//! Read-only. Exit code is always 0 unless config load fails (exit 2) or label is
//! invalid (exit 1). "Plist not loaded" and "no heartbeats yet" are valid statuses
//! to report, NOT errors.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::PathBuf;

use crate::config::Config;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::launchd::{LaunchctlClient, LaunchctlPrintOutput, RealLaunchctl};

/// `advisory-cron status` — show next fire time + recent heartbeats.
///
/// Read-only. Exit code is always 0 unless config load fails (exit 2) or label is
/// invalid (exit 1). "Plist not loaded" and "no heartbeats yet" are valid statuses
/// to report, NOT errors.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Label to query (e.g., "advisory-scan-daily"). Falls back to config.task.label
    /// if omitted, then to literal "advisory-cron" if both unset.
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

#[derive(Serialize)]
struct StatusReport {
    label: String,
    plist_loaded: bool,
    next_fire: Option<String>,
    heartbeat_log_path: String,
    last_runs: Vec<HeartbeatRecord>,
}

pub async fn run(args: Args) -> Result<u8> {
    // 1. Resolve config path. Bail! on $HOME unset (P004 Constraint #16 generalized).
    let config_path = match args.config.clone() {
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

    // 3. Resolve label: CLI flag > config.task.label > literal "advisory-cron".
    let label = args
        .label
        .clone()
        .or_else(|| config.task.label.clone())
        .unwrap_or_else(|| "advisory-cron".to_string());

    // 4. Validate label (INV-12 first enforcement point).
    if !is_valid_label(&label) {
        eprintln!("error: invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        return Ok(1);
    }

    // 5. Query launchctl. V2 fix: RealLaunchctl is unit struct, no ::new().
    let client = RealLaunchctl;
    let print_result = match client.print(&label) {
        Ok(o) => o,
        Err(err) => {
            // Real launchctl error (not "not loaded" — that's caught as not_loaded=true).
            eprintln!("warning: launchctl print failed: {err:#}");
            // Still render heartbeats — partial status is better than nothing.
            LaunchctlPrintOutput {
                raw_stdout: String::new(),
                not_loaded: true,
            }
        }
    };

    // 6. Parse next-fire schedule (V2: descriptor Hour/Minute → "daily at HH:MM";
    //    None if not loaded or format unrecognized).
    let next_fire = if print_result.not_loaded {
        None
    } else {
        parse_next_fire(&print_result.raw_stdout)
    };

    // 7. Read recent heartbeats. Empty Vec on missing file (P004 read_last_n contract).
    let heartbeats = match heartbeat::read_last_n(&config.heartbeat.log_path, args.last) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("warning: failed to read heartbeats: {err:#}");
            Vec::new()
        }
    };

    // 8. Render.
    if args.json {
        let report = StatusReport {
            label: label.clone(),
            plist_loaded: !print_result.not_loaded,
            next_fire: next_fire.clone(),
            heartbeat_log_path: config.heartbeat.log_path.display().to_string(),
            last_runs: heartbeats,
        };
        let json = serde_json::to_string_pretty(&report)
            .context("failed to serialize StatusReport to JSON")?;
        println!("{json}");
    } else {
        render_human(
            &label,
            &print_result,
            &next_fire,
            &heartbeats,
            &config.heartbeat.log_path,
        );
    }

    Ok(0)
}

fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Resolve default config path. Bail! when `$HOME` is unset — never silently fall back
/// to `/`. Mirrors `src/cli/run.rs::default_config_path` (P004 Constraint #16).
fn default_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(anyhow::Error::from)
        .context("$HOME environment variable is not set")?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home).join(".config/advisory-cron/config.toml"))
}

/// Parse the configured recurrence from `launchctl print` stdout.
///
/// **V2 strategy (evidence-driven, per Debate Log Turn 1 [O1.2]):** macOS 15
/// `launchctl print` for `StartCalendarInterval` jobs does NOT expose a next-fire
/// timestamp. It DOES expose the configured recurrence as nested:
///
/// ```text
/// event triggers = {
///     com.advisorycron.<label>.<id> => {
///         ...
///         descriptor = {
///             "Minute" => 0
///             "Hour" => 9
///         }
///     }
/// }
/// ```
///
/// This function scans line-by-line for `"Hour" =>` and `"Minute" =>` patterns
/// inside the descriptor block. Returns:
/// - `Some("daily at HH:MM")` when both Hour and Minute found.
/// - `Some("daily at HH:00")` when only Hour found (degenerate plist).
/// - `Some("hourly at :MM")` when only Minute found (degenerate plist).
/// - `None` when neither found (format unrecognized or future macOS drift).
///
/// Caller renders "unknown" for `None` rather than failing — honest > confident-wrong.
fn parse_next_fire(raw_stdout: &str) -> Option<String> {
    let mut hour: Option<u32> = None;
    let mut minute: Option<u32> = None;

    for line in raw_stdout.lines() {
        let trimmed = line.trim();
        // Match patterns like: "Hour" => 9   or   "Minute" => 0
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

fn render_human(
    label: &str,
    print_result: &LaunchctlPrintOutput,
    next_fire: &Option<String>,
    heartbeats: &[HeartbeatRecord],
    heartbeat_log_path: &std::path::Path,
) {
    println!("advisory-cron status — label: {label}");
    let plist_status = if print_result.not_loaded {
        "not loaded"
    } else {
        "loaded"
    };
    println!("  Plist: {plist_status}");
    let next_fire_display = match (print_result.not_loaded, next_fire) {
        (true, _) => "n/a (not loaded)".to_string(),
        (false, Some(s)) => s.clone(),
        (false, None) => "unknown (launchctl format not recognized)".to_string(),
    };
    println!("  Next fire: {next_fire_display}");
    println!();

    if heartbeats.is_empty() {
        println!(
            "Recent heartbeats: No heartbeats yet (no fires recorded at {})",
            heartbeat_log_path.display()
        );
    } else {
        println!("Recent heartbeats (last {}):", heartbeats.len());
        // Render newest-first (read_last_n returns oldest-first; reverse for display).
        for rec in heartbeats.iter().rev() {
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
        // Take first n bytes (snap forward to char boundary).
        let mut end = n;
        while end < s.len() && !s.is_char_boundary(end) {
            end += 1;
        }
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!is_valid_label("foo.bar")); // dot not in allowlist
    }

    /// V2: Authoritative fixture is Worker Turn 1 captured macOS 15 launchctl output.
    /// Trimmed to relevant descriptor block.
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
        // V2 fixture from Debate Log Turn 1 empirical capture.
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
        // Hour 99 invalid → Hour=None, Minute=Some(0) → "hourly at :00"
        assert_eq!(parse_next_fire(sample), Some("hourly at :00".to_string()));
    }

    #[test]
    fn tail_first_n_or_empty_returns_marker_for_empty() {
        assert_eq!(tail_first_n_or_empty("", 80), "(empty)");
    }

    #[test]
    fn tail_first_n_or_empty_truncates_long_strings() {
        let s = "a".repeat(200);
        let result = tail_first_n_or_empty(&s, 80);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 84); // 80 chars + "..."
    }
}
