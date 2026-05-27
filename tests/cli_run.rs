//! Integration tests for `advisory-cron run` (Phase 1.4).
//!
//! Pattern follows P002 `tests/cli_init.rs` + P003 `tests/cli_register.rs`:
//! spawn the compiled binary with a temp config + temp heartbeat path,
//! assert on exit code + filesystem side effects.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

fn write_config(
    dir: &Path,
    command: &str,
    args: &[&str],
    heartbeat_path: &Path,
) -> std::path::PathBuf {
    let config_path = dir.join("config.toml");
    let args_toml: String = args
        .iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    let contents = format!(
        r#"[task]
command = "{command}"
args = [{args_toml}]
working_dir = "/tmp"
label = "p004-integration"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{}"
"#,
        heartbeat_path.display()
    );
    fs::write(&config_path, contents).expect("write config");
    config_path
}

#[test]
fn run_with_echo_task_exits_zero_and_writes_one_heartbeat() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb/heartbeat.jsonl");
    let config_path = write_config(tmp.path(), "/bin/echo", &["hello-p004"], &heartbeat_path);

    let output = Command::new(BIN)
        .args(["run", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Heartbeat file must exist (parent auto-created)
    assert!(heartbeat_path.exists(), "heartbeat file should be created");
    let contents = fs::read_to_string(&heartbeat_path).expect("read heartbeat");
    assert_eq!(
        contents.lines().count(),
        1,
        "expect exactly 1 heartbeat line"
    );

    // Parse the line as JSON, confirm all 6 schema fields present
    let line = contents.lines().next().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
    assert!(parsed.get("ts").is_some());
    assert!(parsed.get("label").is_some());
    assert_eq!(
        parsed.get("label").and_then(|v| v.as_str()),
        Some("p004-integration")
    );
    assert_eq!(parsed.get("exit_code").and_then(|v| v.as_i64()), Some(0));
    assert!(parsed.get("duration_ms").is_some());
    let stdout_tail = parsed
        .get("stdout_tail")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(stdout_tail.contains("hello-p004"));
    assert_eq!(parsed.get("stderr_tail").and_then(|v| v.as_str()), Some(""));
}

#[test]
fn run_with_failing_task_exits_four_and_writes_heartbeat() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("heartbeat.jsonl");
    let config_path = write_config(tmp.path(), "/bin/sh", &["-c", "exit 7"], &heartbeat_path);

    let output = Command::new(BIN)
        .args(["run", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code(),
        Some(4),
        "expect exit 4 for task non-zero"
    );
    assert!(heartbeat_path.exists());
    let line = fs::read_to_string(&heartbeat_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).expect("valid JSON");
    assert_eq!(parsed.get("exit_code").and_then(|v| v.as_i64()), Some(7));
}

#[test]
fn run_with_nonexistent_binary_exits_four_and_writes_spawn_fail_heartbeat() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("heartbeat.jsonl");
    let config_path = write_config(
        tmp.path(),
        "/this/binary/definitely/does/not/exist",
        &[],
        &heartbeat_path,
    );

    let output = Command::new(BIN)
        .args(["run", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code(),
        Some(4),
        "expect exit 4 for spawn-fail"
    );
    assert!(heartbeat_path.exists());
    let line = fs::read_to_string(&heartbeat_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).expect("valid JSON");
    assert_eq!(
        parsed.get("exit_code").and_then(|v| v.as_i64()),
        Some(-1),
        "spawn-fail heartbeat uses exit_code = -1 (no real child exit code available)"
    );
    let stderr_tail = parsed
        .get("stderr_tail")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        stderr_tail.contains("spawn failed"),
        "stderr_tail should describe spawn failure, got: {stderr_tail}"
    );
}

#[test]
fn run_with_missing_config_exits_two() {
    let tmp = TempDir::new().expect("tempdir");
    let bogus_config = tmp.path().join("does-not-exist.toml");

    let output = Command::new(BIN)
        .args(["run", "--config", bogus_config.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expect exit 2 for missing config per ARCHITECTURE.md:74"
    );
}
