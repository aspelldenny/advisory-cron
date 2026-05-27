//! Integration: `advisory-cron run` with retry config and various task outcomes.
//! Subprocess invokes the binary; wiremock mocks Telegram endpoint.
//!
//! Test matrix per BACKLOG Phase 2.2 acceptance criteria + INV-20:
//! - Failing task with retry config retries up to max_attempts
//! - Successful task within retries → no alert
//! - Final failure after retries → exactly 1 alert (single-alert-per-invocation)
//! - SIGTERM-like exit (signal-killed) → no retry, single attempt
//! - Each retry attempt logs 1 heartbeat
//! - Backwards-compat: no [retry] block → single-fire behavior preserved

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

/// Write a config with [retry] block and optional [alert.telegram] block.
#[allow(clippy::too_many_arguments)]
fn write_retry_config(
    dir: &Path,
    command: &str,
    args: &[&str],
    heartbeat_path: &Path,
    max_attempts: u32,
    backoff_secs: u64,
    alert_token: Option<&str>,
    label: &str,
) -> std::path::PathBuf {
    let config_path = dir.join("config.toml");
    let args_toml = args
        .iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    let alert_block = match alert_token {
        Some(token) => {
            format!("\n[alert.telegram]\nchat_id = \"123456\"\nbot_token = \"{token}\"\n")
        }
        None => String::new(),
    };
    let contents = format!(
        r#"[task]
command = "{command}"
args = [{args_toml}]
working_dir = "/tmp"
label = "{label}"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{hb}"
{alert}
[retry]
max_attempts = {max_attempts}
backoff_secs = {backoff_secs}
"#,
        hb = heartbeat_path.display(),
        alert = alert_block,
    );
    fs::write(&config_path, &contents).expect("write config");
    config_path
}

/// Count JSONL lines in heartbeat file.
fn heartbeat_line_count(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

#[tokio::test]
async fn retry_succeeds_on_attempt_2_no_alert() {
    // Task fails on attempt 1 (counter file absent), succeeds on attempt 2 (counter exists).
    // Expected: binary exits 0, 2 heartbeat lines, ZERO alert POSTs.

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"/bot.*/sendMessage"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"ok":true,"result":{"message_id":1}}"#),
        )
        .expect(0) // ZERO POSTs — task succeeded on attempt 2
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tempdir");
    let hb_path = tmp.path().join("hb/heartbeat.jsonl");
    let counter_file = tmp.path().join("attempt_counter");

    // Shell script: exit 1 on first call (counter absent), exit 0 on subsequent calls.
    let script = format!(
        r#"if [ ! -f "{counter}" ]; then touch "{counter}"; exit 1; else exit 0; fi"#,
        counter = counter_file.display()
    );

    let config_path = write_retry_config(
        tmp.path(),
        "bash",
        &["-c", &script],
        &hb_path,
        3, // max_attempts=3 (will succeed on attempt 2)
        0, // backoff_secs=0 (fast test)
        Some("testtoken"),
        "retry-flaky",
    );

    let output = Command::new(BIN)
        .env("ADVISORY_CRON_TG_API_BASE", server.uri())
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "expected exit 0 (success on attempt 2), got: {}. stderr: {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );

    // 2 heartbeat lines: attempt 1 (exit_code=1) + attempt 2 (exit_code=0).
    let line_count = heartbeat_line_count(&hb_path);
    assert_eq!(
        line_count, 2,
        "expected 2 heartbeat lines (1 per attempt), got: {line_count}"
    );

    // Verify attempt 1 is exit_code=1 and attempt 2 is exit_code=0.
    let hb_content = fs::read_to_string(&hb_path).expect("read heartbeat");
    let lines: Vec<&str> = hb_content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(
        lines[0].contains("\"exit_code\":1"),
        "first heartbeat must be exit_code=1: {}",
        lines[0]
    );
    assert!(
        lines[1].contains("\"exit_code\":0"),
        "second heartbeat must be exit_code=0: {}",
        lines[1]
    );

    // wiremock auto-asserts 0 POSTs on drop.
}

#[tokio::test]
async fn retry_exhausts_max_attempts_single_alert() {
    // Task always fails (command: false = exits 1).
    // max_attempts=3 → 3 heartbeat lines, EXACTLY 1 alert POST (INV-20).

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"/bottesttoken/sendMessage"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"ok":true,"result":{"message_id":2}}"#),
        )
        .expect(1) // EXACTLY 1 POST (single-alert-per-invocation, INV-20)
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tempdir");
    let hb_path = tmp.path().join("hb/heartbeat.jsonl");

    let config_path = write_retry_config(
        tmp.path(),
        "false", // always exits 1
        &[],
        &hb_path,
        3, // max_attempts=3
        0, // backoff_secs=0 (fast)
        Some("testtoken"),
        "retry-always-fail",
    );

    let output = Command::new(BIN)
        .env("ADVISORY_CRON_TG_API_BASE", server.uri())
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("spawn advisory-cron");

    // Exit code 4: task fire failed (same as Phase 2.1 single-fire fail contract).
    assert_eq!(
        output.status.code().unwrap_or(-1),
        4,
        "expected exit 4 (task fail after retries exhausted), got: {}. stderr: {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );

    // 3 heartbeat lines: 1 per attempt, all exit_code=1.
    let line_count = heartbeat_line_count(&hb_path);
    assert_eq!(
        line_count, 3,
        "expected 3 heartbeat lines (1 per attempt), got: {line_count}"
    );

    let hb_content = fs::read_to_string(&hb_path).expect("read heartbeat");
    assert!(
        hb_content.contains("\"exit_code\":1"),
        "all heartbeat lines must be exit_code=1: {hb_content}"
    );

    // wiremock auto-asserts exactly 1 POST on drop.
}

#[tokio::test]
async fn signal_exit_not_retried_single_attempt() {
    // Task exits 143 (SIGTERM convention: 128+15). Non-retryable (≥128).
    // Expected: EXACTLY 1 heartbeat line (no retry), 1 alert POST.

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"/bottesttoken/sendMessage"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"ok":true,"result":{"message_id":3}}"#),
        )
        .expect(1) // 1 POST (final failure surfaces — INV-20)
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tempdir");
    let hb_path = tmp.path().join("hb/heartbeat.jsonl");

    let config_path = write_retry_config(
        tmp.path(),
        "bash",
        &["-c", "exit 143"],
        &hb_path,
        3, // max_attempts=3 — but signal exit ≥128 breaks out immediately
        0, // backoff_secs=0
        Some("testtoken"),
        "retry-sigterm",
    );

    let output = Command::new(BIN)
        .env("ADVISORY_CRON_TG_API_BASE", server.uri())
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code().unwrap_or(-1),
        4,
        "expected exit 4 (task fail — signal exit), got: {}",
        output.status.code().unwrap_or(-1)
    );

    // EXACTLY 1 heartbeat line (no retry on signal-like exit ≥128).
    let line_count = heartbeat_line_count(&hb_path);
    assert_eq!(
        line_count, 1,
        "expected 1 heartbeat line (signal exit NOT retried), got: {line_count}"
    );

    let hb_content = fs::read_to_string(&hb_path).expect("read heartbeat");
    assert!(
        hb_content.contains("\"exit_code\":143"),
        "heartbeat must record exit_code=143: {hb_content}"
    );

    // wiremock auto-asserts 1 POST on drop.
}

#[tokio::test]
async fn no_retry_block_preserves_phase21_single_fire() {
    // Config WITHOUT [retry] block → single-fire Phase 2.1 behavior preserved.
    // Task exits 1 → 1 heartbeat line, 1 alert POST (no retry).

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"/bottesttoken/sendMessage"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"ok":true,"result":{"message_id":4}}"#),
        )
        .expect(1) // 1 POST (single-fire, fails immediately)
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tempdir");
    let hb_path = tmp.path().join("hb/heartbeat.jsonl");

    // Write config WITHOUT [retry] block.
    let config_path = tmp.path().join("config.toml");
    let contents = format!(
        r#"[task]
command = "false"
args = []
working_dir = "/tmp"
label = "no-retry-regression"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{hb}"

[alert.telegram]
chat_id = "123456"
bot_token = "testtoken"
"#,
        hb = hb_path.display(),
    );
    fs::write(&config_path, &contents).expect("write config");

    let output = Command::new(BIN)
        .env("ADVISORY_CRON_TG_API_BASE", server.uri())
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code().unwrap_or(-1),
        4,
        "expected exit 4 (task fail), got: {}",
        output.status.code().unwrap_or(-1)
    );

    // EXACTLY 1 heartbeat line (single-fire — no retry block = Phase 2.1 behavior).
    let line_count = heartbeat_line_count(&hb_path);
    assert_eq!(
        line_count, 1,
        "expected 1 heartbeat line (no retry block = single-fire), got: {line_count}"
    );

    // wiremock auto-asserts 1 POST on drop.
}
