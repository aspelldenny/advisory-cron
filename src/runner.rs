//! Phase 1.4 — task runner. Spawns a configured child process via tokio,
//! captures stdout + stderr + exit code + wall-clock duration.
//!
//! Public surface:
//! - `RunResult` — value type returned to caller (cli::run handler)
//! - `fire_task(config)` — async one-shot spawn + capture
//!
//! Design constraints (from P004 phiếu):
//! - Use `tokio::process::Command` (NOT std::process). Cargo.toml tokio "process" feature confirmed.
//! - Captured stdout/stderr lossy-converted to String (non-UTF8 → U+FFFD). Diagnostic-readable
//!   acceptable; advisory-cron is not a byte-precise log collector.
//! - Signal-killed children (no exit code) reported as exit_code = -1.
//! - Spawn failure (binary not found etc.) propagates as anyhow::Error — caller (cli::run)
//!   builds spawn-fail heartbeat per ARCHITECTURE.md §Error handling.

use anyhow::{Context, Result};
use std::time::Instant;
use tokio::process::Command;

use crate::config::Config;

/// Result of one task fire. Returned to `cli::run` handler which builds heartbeat record.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

/// Spawn `config.task.command` with `config.task.args` in `config.task.working_dir`,
/// wait for exit, capture stdout + stderr.
///
/// Errors: spawn failure (binary not found / perm denied / fork failed).
/// Non-zero exit code is NOT an error — it's a `RunResult` with `exit_code != 0`.
pub async fn fire_task(config: &Config) -> Result<RunResult> {
    let started = Instant::now();
    let output = Command::new(&config.task.command)
        .args(&config.task.args)
        .current_dir(&config.task.working_dir)
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to spawn task `{cmd}` with args {args:?} in dir {dir:?}",
                cmd = config.task.command,
                args = config.task.args,
                dir = config.task.working_dir,
            )
        })?;
    let duration_ms = started.elapsed().as_millis() as u64;

    Ok(RunResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, HeartbeatConfig, ScheduleConfig, TaskConfig};
    use std::path::PathBuf;

    fn echo_config(args: Vec<&str>) -> Config {
        Config {
            task: TaskConfig {
                command: "/bin/echo".to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                working_dir: PathBuf::from("/tmp"),
                label: Some("test".to_string()),
            },
            schedule: ScheduleConfig::Calendar { hour: 9, minute: 0 },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/tmp/unused.jsonl"),
            },
            alert: None,
        }
    }

    #[tokio::test]
    async fn fire_task_echo_captures_stdout_exit_zero() {
        let config = echo_config(vec!["hello"]);
        let result = fire_task(&config).await.expect("echo should succeed");
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.stderr, "");
        // duration is non-zero (echo takes >0ms even fast) but small (<1s on any machine)
        assert!(
            result.duration_ms < 1_000,
            "duration_ms = {}",
            result.duration_ms
        );
    }

    #[tokio::test]
    async fn fire_task_nonexistent_binary_returns_err() {
        let config = echo_config(vec![]);
        let mut bogus = config.clone();
        bogus.task.command = "/nonexistent/binary-that-does-not-exist".to_string();
        let result = fire_task(&bogus).await;
        assert!(result.is_err(), "expected spawn-fail error");
    }

    #[tokio::test]
    async fn fire_task_nonzero_exit_returns_ok_with_code() {
        // /bin/sh -c "exit 7" — captured, returned as RunResult.exit_code = 7
        let mut config = echo_config(vec![]);
        config.task.command = "/bin/sh".to_string();
        config.task.args = vec!["-c".to_string(), "exit 7".to_string()];
        let result = fire_task(&config).await.expect("sh -c spawns");
        assert_eq!(result.exit_code, 7);
    }
}
