//! TOML config schema for advisory-cron.
//!
//! Schema (per docs/ARCHITECTURE.md §Config schema):
//!
//! ```toml
//! [task]
//! command = "claude"
//! args = ["-p", "/advisory-scan"]
//! working_dir = "/Users/<user>"
//!
//! [schedule]
//! # Either cron expression:
//! cron = "0 9 * * *"
//! # Or launchd-friendly calendar:
//! # hour = 9
//! # minute = 0
//!
//! [heartbeat]
//! log_path = "/Users/<user>/.local/state/advisory-cron/heartbeat.jsonl"
//! ```

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Top-level config struct. Maps to the full TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub task: TaskConfig,
    pub schedule: ScheduleConfig,
    pub heartbeat: HeartbeatConfig,
}

/// `[task]` block — what to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
}

/// `[schedule]` block — when to run.
///
/// Two mutually exclusive shapes (untagged enum — serde discriminates by field presence):
/// - Cron shape: `cron = "0 9 * * *"`
/// - Calendar shape: `hour = 9` + `minute = 0` (launchd `StartCalendarInterval`-friendly)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScheduleConfig {
    /// Standard cron expression (5-field: min hour dom mon dow).
    Cron { cron: String },
    /// Launchd-friendly hour/minute pair. `hour` ∈ 0..=23, `minute` ∈ 0..=59.
    Calendar { hour: u8, minute: u8 },
}

/// `[heartbeat]` block — where to write execution records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    pub log_path: PathBuf,
}

impl Config {
    /// Load and validate config from a TOML file.
    ///
    /// Errors map to exit code 2 ("Config not found / invalid") at the CLI boundary —
    /// per docs/ARCHITECTURE.md §CLI surface exit codes.
    ///
    /// Called by Phase 1.3 (`register`) and Phase 1.4 (`run`); forward-declared here.
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("failed to parse TOML config at {}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate logical invariants beyond what serde's structural check can enforce.
    fn validate(&self) -> Result<()> {
        if self.task.command.trim().is_empty() {
            bail!("config.task.command must not be empty");
        }
        if let ScheduleConfig::Calendar { hour, minute } = &self.schedule {
            if *hour > 23 {
                bail!("config.schedule.hour must be 0..=23 (got {hour})");
            }
            if *minute > 59 {
                bail!("config.schedule.minute must be 0..=59 (got {minute})");
            }
        }
        Ok(())
    }

    /// Build sane defaults. Accepts an explicit `home` path for testability —
    /// callers pass `std::env::var("HOME")` resolution; tests pass a tempdir.
    pub fn default_for_home(home: &Path) -> Self {
        Config {
            task: TaskConfig {
                command: "claude".to_string(),
                args: vec!["-p".to_string(), "/advisory-scan".to_string()],
                working_dir: home.to_path_buf(),
            },
            schedule: ScheduleConfig::Calendar { hour: 9, minute: 0 },
            heartbeat: HeartbeatConfig {
                log_path: home.join(".local/state/advisory-cron/heartbeat.jsonl"),
            },
        }
    }

    /// Write a default config file to `path`, creating parent dirs as needed.
    ///
    /// If `path` exists and `force` is false → returns error (caller maps to exit 2).
    pub fn write_default(path: &Path, home: &Path, force: bool) -> Result<()> {
        if path.exists() && !force {
            bail!(
                "config already exists at {} (use --force to overwrite)",
                path.display()
            );
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent dir {}", parent.display()))?;
        }
        let cfg = Config::default_for_home(home);
        let serialized =
            toml::to_string_pretty(&cfg).context("failed to serialize default config to TOML")?;
        fs::write(path, serialized)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ---------------------------------------------------------------------------
    // ScheduleConfig parsing — Anchor #14 verification (untagged enum)
    // ---------------------------------------------------------------------------

    #[test]
    fn schedule_parses_cron_shape() {
        let raw = r#"
            [task]
            command = "claude"
            args = []
            working_dir = "/tmp"

            [schedule]
            cron = "0 9 * * *"

            [heartbeat]
            log_path = "/tmp/hb.jsonl"
        "#;
        let cfg: Config = toml::from_str(raw).expect("cron-shape must parse");
        assert!(matches!(cfg.schedule, ScheduleConfig::Cron { .. }));
    }

    #[test]
    fn schedule_parses_calendar_shape() {
        let raw = r#"
            [task]
            command = "claude"
            args = []
            working_dir = "/tmp"

            [schedule]
            hour = 9
            minute = 0

            [heartbeat]
            log_path = "/tmp/hb.jsonl"
        "#;
        let cfg: Config = toml::from_str(raw).expect("calendar-shape must parse");
        assert!(matches!(cfg.schedule, ScheduleConfig::Calendar { .. }));
    }

    // ---------------------------------------------------------------------------
    // Config::load — error paths
    // ---------------------------------------------------------------------------

    #[test]
    fn load_rejects_missing_required_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        // [heartbeat] block omitted entirely.
        fs::write(
            &path,
            r#"
            [task]
            command = "claude"
            args = []
            working_dir = "/tmp"

            [schedule]
            hour = 9
            minute = 0
        "#,
        )
        .unwrap();
        let err = Config::load(&path).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("parse"),
            "expected parse error mention, got: {msg}"
        );
    }

    // ---------------------------------------------------------------------------
    // validate() — logical invariants
    // ---------------------------------------------------------------------------

    #[test]
    fn validate_rejects_empty_command() {
        let raw = r#"
            [task]
            command = "   "
            args = []
            working_dir = "/tmp"

            [schedule]
            hour = 9
            minute = 0

            [heartbeat]
            log_path = "/tmp/hb.jsonl"
        "#;
        let cfg: Config = toml::from_str(raw).unwrap();
        let err = cfg.validate().unwrap_err();
        assert!(format!("{err:#}").contains("command"));
    }

    #[test]
    fn validate_rejects_invalid_hour() {
        let cfg = Config {
            task: TaskConfig {
                command: "claude".into(),
                args: vec![],
                working_dir: PathBuf::from("/tmp"),
            },
            schedule: ScheduleConfig::Calendar {
                hour: 25,
                minute: 0,
            },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/tmp/hb.jsonl"),
            },
        };
        let err = cfg.validate().unwrap_err();
        assert!(format!("{err:#}").contains("hour"));
    }

    #[test]
    fn validate_rejects_invalid_minute() {
        let cfg = Config {
            task: TaskConfig {
                command: "claude".into(),
                args: vec![],
                working_dir: PathBuf::from("/tmp"),
            },
            schedule: ScheduleConfig::Calendar {
                hour: 9,
                minute: 60,
            },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/tmp/hb.jsonl"),
            },
        };
        let err = cfg.validate().unwrap_err();
        assert!(format!("{err:#}").contains("minute"));
    }

    // ---------------------------------------------------------------------------
    // write_default — round-trip + overwrite behaviour
    // ---------------------------------------------------------------------------

    #[test]
    fn write_default_creates_parent_dirs_and_file() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a/b/c/config.toml");
        let home = dir.path();
        Config::write_default(&nested, home, false).unwrap();
        assert!(nested.exists());
        // Round-trip: load what we just wrote.
        let cfg = Config::load(&nested).unwrap();
        assert_eq!(cfg.task.command, "claude");
        assert!(matches!(
            cfg.schedule,
            ScheduleConfig::Calendar { hour: 9, minute: 0 }
        ));
    }

    #[test]
    fn write_default_refuses_overwrite_without_force() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let home = dir.path();
        Config::write_default(&path, home, false).unwrap();
        let err = Config::write_default(&path, home, false).unwrap_err();
        assert!(format!("{err:#}").contains("--force"));
    }

    #[test]
    fn write_default_overwrites_with_force() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let home = dir.path();
        Config::write_default(&path, home, false).unwrap();
        // Corrupt the file.
        fs::write(&path, "garbage").unwrap();
        // Overwrite with --force.
        Config::write_default(&path, home, true).unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.task.command, "claude");
    }
}
