//! Phase 1.3 acceptance: register/unregister flow with TempDir HOME override + binary spawn.
//!
//! CRITICAL: integration tests spawn the compiled binary, so they CANNOT inject NoopLaunchctl
//! across the CLI boundary. They exercise the END-TO-END flow with real RealLaunchctl invocation.
//! Pollution mitigation: unique label suffix per test (PID-based) + best-effort tearDown cleanup.
//! Unit tests inside src/launchd.rs #[cfg(test)] mod are the only place NoopLaunchctl is wired.

use std::path::Path;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

fn write_default_config(home: &Path) -> std::path::PathBuf {
    let config_dir = home.join(".config/advisory-cron");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    // Use Calendar schedule to avoid needing --schedule CLI arg.
    let toml = format!(
        r#"
[task]
command = "claude"
args = ["-p", "/advisory-scan"]
working_dir = "{}"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{}/.local/state/advisory-cron/heartbeat.jsonl"
"#,
        home.display(),
        home.display()
    );
    std::fs::write(&config_path, toml).unwrap();
    config_path
}

#[test]
fn register_writes_plist_to_launch_agents_dir() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());
    let label = format!("test-p003-{}", std::process::id());

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label")
        .arg(&label)
        .output()
        .expect("spawn failed");

    let plist_path = home
        .path()
        .join("Library/LaunchAgents")
        .join(format!("com.advisorycron.{label}.plist"));

    // Plist MUST be written regardless of launchctl bootstrap outcome (write happens before bootstrap).
    assert!(
        plist_path.exists(),
        "plist not written at {} (exit={:?} stdout={} stderr={})",
        plist_path.display(),
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // Plist content sanity: contains label.
    let plist_content = std::fs::read_to_string(&plist_path).unwrap();
    assert!(plist_content.contains(&format!("com.advisorycron.{label}")));
    assert!(plist_content.contains("<key>Hour</key><integer>9</integer>"));

    // Cleanup: best-effort launchctl bootout + file removal (test pollution mitigation).
    let _ = std::process::Command::new("launchctl")
        .arg("bootout")
        .arg(format!("gui/{}/com.advisorycron.{label}", uid()))
        .output();
    let _ = std::fs::remove_file(&plist_path);
}

#[test]
fn register_with_cron_simple_form_works() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());
    let label = format!("test-p003-cron-{}", std::process::id());

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label")
        .arg(&label)
        .arg("--schedule")
        .arg("30 14 * * *")
        .output()
        .expect("spawn failed");

    let plist_path = home
        .path()
        .join("Library/LaunchAgents")
        .join(format!("com.advisorycron.{label}.plist"));

    assert!(
        plist_path.exists(),
        "plist not written (exit={:?} stderr={})",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let plist_content = std::fs::read_to_string(&plist_path).unwrap();
    assert!(plist_content.contains("<key>Hour</key><integer>14</integer>"));
    assert!(plist_content.contains("<key>Minute</key><integer>30</integer>"));

    let _ = std::process::Command::new("launchctl")
        .arg("bootout")
        .arg(format!("gui/{}/com.advisorycron.{label}", uid()))
        .output();
    let _ = std::fs::remove_file(&plist_path);
}

#[test]
fn register_complex_cron_exits_2() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label")
        .arg("test-p003-bad")
        .arg("--schedule")
        .arg("*/5 * * * 1-5")
        .output()
        .expect("spawn failed");

    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for complex cron, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn register_missing_config_exits_2() {
    let home = TempDir::new().unwrap();
    // NO write_default_config — config absent.

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label")
        .arg("test-p003-noconfig")
        .arg("--schedule")
        .arg("0 9 * * *")
        .output()
        .expect("spawn failed");

    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unregister_nonexistent_label_exits_0_idempotent() {
    let home = TempDir::new().unwrap();
    // No prior register.

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("unregister")
        .arg("--label")
        .arg("test-p003-never-existed")
        .output()
        .expect("spawn failed");

    // Idempotent: exit 0 even if label never loaded + plist file never existed.
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for idempotent unregister, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning") || stderr.contains("absent") || stderr.contains("not loaded"),
        "expected warning in stderr, got: {stderr}"
    );
}

#[test]
fn register_then_unregister_round_trip() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());
    let label = format!("test-p003-rt-{}", std::process::id());

    // Register
    let _ = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label")
        .arg(&label)
        .output()
        .expect("register spawn failed");

    let plist_path = home
        .path()
        .join("Library/LaunchAgents")
        .join(format!("com.advisorycron.{label}.plist"));
    assert!(plist_path.exists(), "plist should exist after register");

    // Unregister
    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("unregister")
        .arg("--label")
        .arg(&label)
        .output()
        .expect("unregister spawn failed");

    assert_eq!(out.status.code(), Some(0));
    assert!(
        !plist_path.exists(),
        "plist should be removed after unregister"
    );
}

/// Best-effort UID helper for cleanup (mirrors src/launchd.rs::current_uid logic).
fn uid() -> u32 {
    let out = std::process::Command::new("id").arg("-u").output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}
