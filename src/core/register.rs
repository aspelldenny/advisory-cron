//! Core register logic — build RegisterIntent + delegate to Scheduler.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::register` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.
//!
//! V2 (per Architect Turn 1 RESPOND [O1.3] ACCEPT):
//! - Resolves `home`, `launch_agents_dir`, `self_exe` ALL internally.
//! - ONLY `&S: Scheduler` is injected (preserves testability via NoopScheduler).
//!
//! Phase 3.1 (P012): generic `<L: LaunchctlClient>` → `<S: Scheduler>`. Plist generation
//! moved into `MacosScheduler::register`; this module builds `RegisterIntent` + delegates.

use crate::config::{Config, ScheduleConfig};
use crate::core::config_path::home_dir;
use crate::scheduler::{RegisterIntent, Scheduler};
use anyhow::{Context, Result};
use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct RegisterArgs {
    /// Label suffix (full label = com.advisorycron.<label>).
    pub label: String,
    /// Cron expression (`M H * * *` daily form) — overrides config.schedule when present.
    pub schedule: Option<String>,
    /// Override default config path.
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegisterOutput {
    pub plist_path: PathBuf,
    pub label: String,
    pub bootstrapped: bool,
}

/// Validate label allowlist (INV-12 enforcement point in core).
pub fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// V2 (per Architect Turn 1 RESPOND [O1.3] ACCEPT):
/// - Resolves home internally.
/// - Resolves launch_agents_dir internally (inside scheduler impl).
/// - Resolves self_exe internally.
/// - Returns typed error; CLI/MCP layer maps to exit codes.
///
/// Phase 3.1: builds `RegisterIntent` and delegates to `scheduler.register()`.
pub fn run<S: Scheduler>(args: RegisterArgs, scheduler: &S) -> Result<RegisterOutput> {
    // 1. Validate label (INV-12 first enforcement in core).
    if !is_valid_label(&args.label) {
        anyhow::bail!(
            "invalid label {:?} — must be ASCII alphanumeric + '-' + '_'",
            args.label
        );
    }

    // 2. Resolve home internally (V2 [O1.3]).
    let home = home_dir().context("failed to resolve $HOME")?;

    // 3. Resolve config path.
    let config_path = args
        .config_path
        .unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"));

    // 4. Load config.
    let mut config = Config::load(&config_path)
        .with_context(|| format!("failed to load config at {}", config_path.display()))?;

    // 5. Apply --schedule CLI override.
    if let Some(cron_expr) = args.schedule {
        config.schedule = ScheduleConfig::Cron { cron: cron_expr };
    }

    // 6. Resolve hour/minute from config.schedule.
    let (hour, minute) = match &config.schedule {
        ScheduleConfig::Calendar { hour, minute } => (*hour, *minute),
        ScheduleConfig::Cron { cron } => parse_daily_cron(cron)?,
    };

    // 7. Resolve self_exe internally (V2 [O1.3]).
    let self_exe = env::current_exe().context("failed to resolve current executable path")?;

    // 8. Build intent and delegate to scheduler.
    let intent = RegisterIntent {
        label: args.label.clone(),
        hour,
        minute,
        self_exe,
        working_dir: config.task.working_dir.clone(),
    };

    let report = scheduler
        .register(&intent)
        .context("scheduler register failed")?;

    Ok(RegisterOutput {
        plist_path: report.plist_path.unwrap_or_default(),
        label: args.label,
        bootstrapped: true,
    })
}

/// Parse cron expression in simple `M H * * *` daily form → (hour, minute) tuple.
///
/// Domain logic: TOML `ScheduleConfig::Cron` → `(u8, u8)` for scheduler intent.
/// Lives in core::register (not scheduler trait) because this is config-domain parsing,
/// not scheduler-domain logic. Phase 3.2 P013 adds `parse_cron_next_fire` independently.
fn parse_daily_cron(expr: &str) -> Result<(u8, u8)> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        anyhow::bail!(
            "cron expression must be 5 fields (got {}): {expr:?}",
            parts.len()
        );
    }
    if parts[2] != "*" || parts[3] != "*" || parts[4] != "*" {
        anyhow::bail!(
            "Phase 3.1: launchd cron support requires day/month/dow all `*` (daily fire)"
        );
    }
    let minute: u8 = parts[0]
        .parse()
        .with_context(|| format!("cron minute must be numeric (got {:?})", parts[0]))?;
    let hour: u8 = parts[1]
        .parse()
        .with_context(|| format!("cron hour must be numeric (got {:?})", parts[1]))?;
    if hour > 23 || minute > 59 {
        anyhow::bail!(
            "cron hour must be 0..=23 and minute 0..=59 (got hour={hour} minute={minute})"
        );
    }
    Ok((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::NoopScheduler;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: write a minimal valid config to a temp path.
    fn write_minimal_config(path: &std::path::Path, home: &std::path::Path) {
        use crate::config::Config;
        Config::write_default(path, home, false).unwrap();
    }

    #[test]
    fn run_registers_successfully_with_noop_scheduler() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        unsafe {
            std::env::set_var("HOME", home);
        }

        // Write config to expected location.
        let config_path = home.join(".config/advisory-cron/config.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        write_minimal_config(&config_path, home);

        let scheduler = NoopScheduler::default();
        let result = run(
            RegisterArgs {
                label: "test-label".to_string(),
                schedule: None,
                config_path: Some(config_path),
            },
            &scheduler,
        );
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let output = result.unwrap();
        assert_eq!(output.label, "test-label");
        assert!(output.bootstrapped);
        // Verify scheduler was called once.
        assert_eq!(scheduler.register_calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn run_rejects_invalid_label() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let scheduler = NoopScheduler::default();
        let result = run(
            RegisterArgs {
                label: "bad label!".to_string(),
                schedule: None,
                config_path: None,
            },
            &scheduler,
        );
        assert!(result.is_err());
    }

    // INV-12 specific attack-class tests — verify pre-flight rejection BEFORE
    // Scheduler invocation (register_calls must be 0 on rejection).

    #[test]
    fn register_rejects_label_with_whitespace() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let scheduler = NoopScheduler::default();
        let result = run(
            RegisterArgs {
                label: "foo bar".to_string(),
                schedule: None,
                config_path: None,
            },
            &scheduler,
        );
        assert!(
            result.is_err(),
            "label with whitespace must be rejected at pre-flight (INV-12)"
        );
        assert_eq!(
            scheduler.register_calls.lock().unwrap().len(),
            0,
            "pre-flight rejection must occur before Scheduler invocation"
        );
    }

    #[test]
    fn register_rejects_label_with_path_separator() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let scheduler = NoopScheduler::default();
        let result = run(
            RegisterArgs {
                label: "foo/bar".to_string(),
                schedule: None,
                config_path: None,
            },
            &scheduler,
        );
        assert!(
            result.is_err(),
            "label with `/` must be rejected — path traversal vector (INV-12)"
        );
        assert_eq!(
            scheduler.register_calls.lock().unwrap().len(),
            0,
            "pre-flight rejection must occur before Scheduler invocation"
        );
    }

    #[test]
    fn register_rejects_label_with_shell_metachar() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let scheduler = NoopScheduler::default();
        let result = run(
            RegisterArgs {
                label: "foo;rm".to_string(),
                schedule: None,
                config_path: None,
            },
            &scheduler,
        );
        assert!(
            result.is_err(),
            "label with `;` must be rejected — shell metachar (INV-12)"
        );
        assert_eq!(
            scheduler.register_calls.lock().unwrap().len(),
            0,
            "pre-flight rejection must occur before Scheduler invocation"
        );
    }
}
