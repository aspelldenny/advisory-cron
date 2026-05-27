//! Integration: `advisory-cron run` with Telegram alert configured.
//!
//! Subprocess invokes the binary; wiremock mocks Telegram endpoint; asserts
//! POST received (or not) depending on task exit code + config.
//!
//! V2: API base override via `ADVISORY_CRON_TG_API_BASE` env var. Test sets
//! this env var on the subprocess via `Command::env(...)` so the child
//! `core::run::run` reads it at the call site and routes POST to the mock
//! server. `alert.rs` itself is env-free — no env setup needed for its unit
//! tests of `send_with_base`.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

/// Write a config with optional [alert.telegram] block.
fn write_config_with_alert(
    dir: &Path,
    command: &str,
    heartbeat_path: &Path,
    alert_token: Option<&str>,
    alert_chat_id: Option<&str>,
) -> std::path::PathBuf {
    let config_path = dir.join("config.toml");
    let alert_block = match (alert_token, alert_chat_id) {
        (Some(token), Some(chat_id)) => {
            format!("\n[alert.telegram]\nchat_id = \"{chat_id}\"\nbot_token = \"{token}\"\n")
        }
        _ => String::new(),
    };
    let contents = format!(
        r#"[task]
command = "{command}"
args = []
working_dir = "/tmp"
label = "p008-integration"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{hb}"
{alert}
"#,
        hb = heartbeat_path.display(),
        alert = alert_block,
    );
    fs::write(&config_path, &contents).expect("write config");
    config_path
}

#[tokio::test]
async fn run_failing_task_posts_to_telegram() {
    // 1. Start wiremock MockServer.
    let server = MockServer::start().await;

    // 2. Configure Mock: POST /bot<token>/sendMessage → 200.
    Mock::given(method("POST"))
        .and(path_regex(r"/bottesttoken/sendMessage"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"ok":true,"result":{"message_id":42}}"#),
        )
        .expect(1) // exactly 1 POST for the failing task
        .mount(&server)
        .await;

    // 3. Write config with alert + heartbeat.
    let tmp = TempDir::new().expect("tempdir");
    let hb_path = tmp.path().join("hb/heartbeat.jsonl");
    let config_path = write_config_with_alert(
        tmp.path(),
        "false", // always exits 1
        &hb_path,
        Some("testtoken"),
        Some("123456"),
    );

    // 4. Spawn subprocess with ADVISORY_CRON_TG_API_BASE pointing at mock.
    let output = Command::new(BIN)
        .env("ADVISORY_CRON_TG_API_BASE", server.uri())
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("spawn advisory-cron");

    // 5. Assert exit code 4 (task fire failed).
    assert_eq!(
        output.status.code().unwrap_or(-1),
        4,
        "expected exit 4 (task fail), got: {}. stderr: {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );

    // 6. Assert heartbeat written with exit_code != 0.
    assert!(hb_path.exists(), "heartbeat file must exist");
    let hb_content = fs::read_to_string(&hb_path).expect("read heartbeat");
    assert!(
        hb_content.contains("\"exit_code\":1") || hb_content.contains("\"exit_code\":-1"),
        "heartbeat must record non-zero exit: {hb_content}"
    );

    // 7. Mock verifies exactly 1 POST was received (wiremock auto-asserts on drop).
}

#[tokio::test]
async fn run_failing_task_without_alert_config_sends_no_post() {
    // Alert config absent → no POST even if ADVISORY_CRON_TG_API_BASE is set.
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_regex(r"/bot.*/sendMessage"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // 0 POSTs expected
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tempdir");
    let hb_path = tmp.path().join("hb/heartbeat.jsonl");
    // No alert block — pass None for token/chat_id.
    let config_path = write_config_with_alert(tmp.path(), "false", &hb_path, None, None);

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
        "expected exit 4 (task fail)"
    );
    // Mock auto-asserts 0 POSTs on drop.
}

#[tokio::test]
async fn run_successful_task_sends_no_post() {
    // Task exits 0 → no POST, even with alert configured.
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_regex(r"/bot.*/sendMessage"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // 0 POSTs expected for successful task
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tempdir");
    let hb_path = tmp.path().join("hb/heartbeat.jsonl");
    let config_path = write_config_with_alert(
        tmp.path(),
        "true", // exits 0
        &hb_path,
        Some("testtoken"),
        Some("123456"),
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
        "expected exit 0 (success), got: {}",
        output.status.code().unwrap_or(-1)
    );
    // Mock auto-asserts 0 POSTs on drop.
}
