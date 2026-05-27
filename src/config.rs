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
    /// Optional alert config. Absent in Phase 1 configs — deserializes as `None`
    /// via `#[serde(default)]`. Alert is opt-in: `advisory-cron init` does not
    /// write this block. Sếp manually adds `[alert.telegram]` to enable.
    #[serde(default)]
    pub alert: Option<AlertConfig>,
    /// Optional retry config. Absent by default — retry is opt-in.
    /// Old configs (Phase 1 + Phase 2.1) without `[retry]` block deserialize
    /// as `None` (backwards-compat preserved via `#[serde(default)]`).
    /// When absent, behavior is single-fire (Phase 2.1 semantics).
    #[serde(default)]
    pub retry: Option<RetryConfig>,
}

/// `[task]` block — what to run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    /// Optional label identifying this task in heartbeat records.
    /// Distinct from `register --label` (which becomes the launchd plist Label key).
    /// Phase 2 alert may use this to distinguish multiple advisory-cron configs reporting
    /// to the same Telegram chat. Defaults to "advisory-cron" when omitted.
    #[serde(default)]
    pub label: Option<String>,
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

/// `[alert]` block. Optional — alert is opt-in.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertConfig {
    pub telegram: Option<TelegramConfig>,
}

/// `[retry]` block. Optional — retry is opt-in. When absent, behavior is
/// single-fire (1 attempt, no retry), preserving Phase 2.1 semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of fire attempts. `1` = no retry (single attempt).
    /// `≥ 2` = retry up to (max_attempts - 1) times after initial failure.
    /// Validation: must be ≥ 1.
    pub max_attempts: u32,
    /// Seconds to sleep between attempts. `0` = retry immediately.
    /// Validation: must be ≤ 3600 (sanity cap, prevent freezing launchd
    /// job for a day via typo).
    pub backoff_secs: u64,
}

/// `[alert.telegram]` block. Either `bot_token` (inline) OR `bot_token_file`
/// (path to KEY=VAL file with TG_BOT_TOKEN=...). Validated at load time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub chat_id: String,
    pub bot_token: Option<String>,
    pub bot_token_file: Option<PathBuf>,
}

impl Config {
    /// Load and validate config from a TOML file.
    ///
    /// Errors map to exit code 2 ("Config not found / invalid") at the CLI boundary —
    /// per docs/ARCHITECTURE.md §CLI surface exit codes.
    ///
    /// Called by Phase 1.3 (`register`) and Phase 1.4 (`run`); forward-declared here.
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
        // Alert config validation (Phase 2.1).
        if let Some(alert) = &self.alert
            && let Some(tg) = &alert.telegram
        {
            if tg.chat_id.trim().is_empty() {
                bail!("[alert.telegram].chat_id is empty");
            }
            match (&tg.bot_token, &tg.bot_token_file) {
                (Some(_), Some(_)) => bail!(
                    "[alert.telegram]: specify either `bot_token` or `bot_token_file`, not both"
                ),
                (None, None) => {
                    bail!("[alert.telegram]: missing both `bot_token` and `bot_token_file`")
                }
                (Some(t), None) if t.trim().is_empty() => {
                    bail!("[alert.telegram].bot_token is empty")
                }
                _ => {}
            }
        }
        // Retry config validation (Phase 2.2).
        if let Some(retry) = &self.retry {
            if retry.max_attempts < 1 {
                anyhow::bail!(
                    "[retry].max_attempts must be ≥ 1 (got {})",
                    retry.max_attempts
                );
            }
            if retry.backoff_secs > 3600 {
                anyhow::bail!(
                    "[retry].backoff_secs sanity cap exceeded — got {} (max 3600 = 1 hour). \
                     Use a shorter backoff or disable retry by removing the [retry] block.",
                    retry.backoff_secs
                );
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
                label: Some("advisory-cron".to_string()),
            },
            schedule: ScheduleConfig::Calendar { hour: 9, minute: 0 },
            heartbeat: HeartbeatConfig {
                log_path: home.join(".local/state/advisory-cron/heartbeat.jsonl"),
            },
            // Alert is opt-in. `advisory-cron init` does NOT write [alert] block.
            // Sếp adds it manually after creating the bot token.
            alert: None,
            // Retry is opt-in. `advisory-cron init` does NOT write [retry] block.
            // Sếp adds it manually when transient failures need retry handling.
            retry: None,
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
                label: None,
            },
            schedule: ScheduleConfig::Calendar {
                hour: 25,
                minute: 0,
            },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/tmp/hb.jsonl"),
            },
            alert: None,
            retry: None,
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
                label: None,
            },
            schedule: ScheduleConfig::Calendar {
                hour: 9,
                minute: 60,
            },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/tmp/hb.jsonl"),
            },
            alert: None,
            retry: None,
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

    // ---------------------------------------------------------------------------
    // P004 — task.label field (backward compat + default_for_home)
    // ---------------------------------------------------------------------------

    #[test]
    fn task_label_absent_in_toml_deserializes_to_none() {
        // Old config without label field must still parse cleanly (backward compat).
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
        let cfg: Config =
            toml::from_str(raw).expect("old config must still parse with #[serde(default)]");
        assert_eq!(
            cfg.task.label, None,
            "missing label field should deserialize to None"
        );
    }

    #[test]
    fn task_label_present_in_toml_deserializes_correctly() {
        let raw = r#"
            [task]
            command = "claude"
            args = []
            working_dir = "/tmp"
            label = "my-task"

            [schedule]
            hour = 9
            minute = 0

            [heartbeat]
            log_path = "/tmp/hb.jsonl"
        "#;
        let cfg: Config = toml::from_str(raw).expect("config with label must parse");
        assert_eq!(cfg.task.label, Some("my-task".to_string()));
    }

    #[test]
    fn default_for_home_includes_advisory_cron_label() {
        let dir = TempDir::new().unwrap();
        let cfg = Config::default_for_home(dir.path());
        assert_eq!(cfg.task.label, Some("advisory-cron".to_string()));
    }

    // ---------------------------------------------------------------------------
    // P008 — alert config (backwards compat + validation)
    // ---------------------------------------------------------------------------

    fn base_toml() -> &'static str {
        r#"
        [task]
        command = "claude"
        args = []
        working_dir = "/tmp"

        [schedule]
        hour = 9
        minute = 0

        [heartbeat]
        log_path = "/tmp/hb.jsonl"
        "#
    }

    #[test]
    fn load_without_alert_block_gives_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, base_toml()).unwrap();
        let cfg = Config::load(&path).unwrap();
        assert!(
            cfg.alert.is_none(),
            "old config without [alert] block must deserialize alert as None"
        );
    }

    #[test]
    fn load_with_alert_inline_token() {
        let raw = format!(
            r#"{}
[alert.telegram]
chat_id = "12345"
bot_token = "my-bot-token"
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        cfg.validate().unwrap();
        let tg = cfg
            .alert
            .unwrap()
            .telegram
            .expect("telegram block must be Some");
        assert_eq!(tg.chat_id, "12345");
        assert_eq!(tg.bot_token.as_deref(), Some("my-bot-token"));
        assert!(tg.bot_token_file.is_none());
    }

    #[test]
    fn load_with_alert_file_token() {
        let raw = format!(
            r#"{}
[alert.telegram]
chat_id = "99999"
bot_token_file = "/tmp/secrets.env"
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        cfg.validate().unwrap();
        let tg = cfg
            .alert
            .unwrap()
            .telegram
            .expect("telegram block must be Some");
        assert_eq!(tg.chat_id, "99999");
        assert!(tg.bot_token.is_none());
        assert!(tg.bot_token_file.is_some());
    }

    #[test]
    fn validate_alert_both_set_returns_err() {
        let raw = format!(
            r#"{}
[alert.telegram]
chat_id = "123"
bot_token = "tok"
bot_token_file = "/tmp/secrets.env"
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        let err = cfg.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("not both"), "got: {msg}");
    }

    #[test]
    fn validate_alert_neither_set_returns_err() {
        let raw = format!(
            r#"{}
[alert.telegram]
chat_id = "123"
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        let err = cfg.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("missing both"), "got: {msg}");
    }

    #[test]
    fn validate_alert_empty_chat_id_returns_err() {
        let raw = format!(
            r#"{}
[alert.telegram]
chat_id = "   "
bot_token = "tok"
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        let err = cfg.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("chat_id"), "got: {msg}");
    }

    // ---------------------------------------------------------------------------
    // P009 — retry config (backwards compat + validation)
    // ---------------------------------------------------------------------------

    #[test]
    fn load_without_retry_block_gives_none() {
        // Old config without [retry] block must still parse cleanly (backwards-compat).
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, base_toml()).unwrap();
        let cfg = Config::load(&path).unwrap();
        assert!(
            cfg.retry.is_none(),
            "old config without [retry] block must deserialize retry as None"
        );
    }

    #[test]
    fn load_with_retry_block() {
        let raw = format!(
            r#"{}
[retry]
max_attempts = 3
backoff_secs = 5
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        cfg.validate().unwrap();
        let retry = cfg.retry.expect("retry block must be Some");
        assert_eq!(retry.max_attempts, 3);
        assert_eq!(retry.backoff_secs, 5);
    }

    #[test]
    fn validate_retry_zero_attempts_returns_err() {
        let raw = format!(
            r#"{}
[retry]
max_attempts = 0
backoff_secs = 5
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        let err = cfg.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("max_attempts"),
            "expected max_attempts error, got: {msg}"
        );
    }

    #[test]
    fn validate_retry_excessive_backoff_returns_err() {
        let raw = format!(
            r#"{}
[retry]
max_attempts = 2
backoff_secs = 7200
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        let err = cfg.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("backoff_secs"),
            "expected backoff_secs error, got: {msg}"
        );
    }

    #[test]
    fn load_with_retry_and_alert() {
        // Both [retry] and [alert.telegram] blocks present — no interference.
        let raw = format!(
            r#"{}
[alert.telegram]
chat_id = "123"
bot_token = "testtoken"

[retry]
max_attempts = 2
backoff_secs = 10
"#,
            base_toml()
        );
        let cfg: Config = toml::from_str(&raw).unwrap();
        cfg.validate().unwrap();
        assert!(cfg.alert.is_some(), "alert must be Some");
        let retry = cfg.retry.expect("retry must be Some");
        assert_eq!(retry.max_attempts, 2);
        assert_eq!(retry.backoff_secs, 10);
    }
}
