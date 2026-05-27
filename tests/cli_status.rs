//! Integration tests for `advisory-cron status` (Phase 1.5).
//!
//! Pattern mirrors `tests/cli_run.rs` (P004): spawn the compiled binary with a temp
//! config + temp heartbeat path, assert on exit code + stdout/stderr.
//!
//! These tests do NOT exercise `RealLaunchctl::print` against a real loaded plist
//! (would require side-effect on `~/Library/LaunchAgents/`). Real launchctl path
//! tested manually per Verification Trace Sub-mech A rows.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn binary_path() -> String {
    env!("CARGO_BIN_EXE_advisory-cron").to_string()
}

fn write_config(dir: &Path, heartbeat_path: &Path) -> std::path::PathBuf {
    let config_path = dir.join("config.toml");
    let contents = format!(
        r#"[task]
command = "/bin/echo"
args = ["hello"]
working_dir = "/tmp"
label = "p005-status-test"

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

fn write_heartbeat_line(path: &Path, exit_code: i32, label: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create heartbeat dir");
    }
    // Manually compose JSON (avoids depending on internal HeartbeatRecord struct from test crate).
    // Schema must match exactly — if this drifts, P005 test breaks loudly (good signal).
    let line = format!(
        r#"{{"ts":"2026-05-27T02:00:00Z","label":"{label}","exit_code":{exit_code},"duration_ms":100,"stdout_tail":"hello","stderr_tail":""}}{}"#,
        "\n"
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open heartbeat");
    file.write_all(line.as_bytes()).expect("write heartbeat");
}

#[test]
fn status_with_heartbeats_and_unloaded_plist_exits_zero_human() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb/heartbeat.jsonl");
    write_heartbeat_line(&heartbeat_path, 0, "p005-status-test");
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "definitely-not-loaded-label-p005",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("not loaded"),
        "expected 'not loaded' in output:\n{stdout}"
    );
    assert!(
        stdout.contains("exit=0"),
        "expected heartbeat exit=0 line:\n{stdout}"
    );
}

#[test]
fn status_with_no_heartbeats_exits_zero_with_friendly_message() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("does-not-exist.jsonl");
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "any-label-p005",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(output.status.success(), "expected exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No heartbeats yet"),
        "expected 'No heartbeats yet' in output:\n{stdout}"
    );
}

#[test]
fn status_json_mode_produces_valid_json() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb.jsonl");
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "any-label-p005",
            "--json",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(output.status.success(), "expected exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("status --json output is not valid JSON: {e}\nstdout: {stdout}")
    });
    assert!(parsed.get("label").is_some());
    assert!(parsed.get("plist_loaded").is_some());
    assert!(parsed.get("last_runs").is_some());
    assert_eq!(
        parsed
            .get("last_runs")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(0)
    );
}

#[test]
fn status_last_flag_clamps_heartbeat_count() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb.jsonl");
    // Write 5 heartbeats.
    for i in 0..5 {
        write_heartbeat_line(&heartbeat_path, i, "p005-test");
    }
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "any-label-p005",
            "--last",
            "3",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(output.status.success(), "expected exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Count occurrences of "exit=" — should appear exactly 3 times in human render
    // (one per heartbeat line).
    let count = stdout.matches("exit=").count();
    assert_eq!(
        count, 3,
        "expected exactly 3 heartbeat lines (--last 3), got {count}\nstdout: {stdout}"
    );
}

#[test]
fn status_with_missing_config_exits_two() {
    let tmp = TempDir::new().expect("tempdir");
    let bogus_config = tmp.path().join("does-not-exist.toml");

    let output = Command::new(binary_path())
        .args(["status", "--config", bogus_config.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expect exit 2 for missing config per ARCHITECTURE.md:74"
    );
}
