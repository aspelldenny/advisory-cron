//! Phase 1.2 acceptance: `advisory-cron init` writes default config,
//! refuses overwrite without --force, and produces parseable TOML.

use std::process::Command;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

/// Run `advisory-cron init` with `$HOME` overridden to a tempdir.
/// Returns (exit_code, stdout, stderr).
fn run_init(home: &std::path::Path, force: bool) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.env("HOME", home).arg("init");
    if force {
        cmd.arg("--force");
    }
    let out = cmd.output().expect("failed to spawn binary");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn init_writes_default_config_to_xdg_path() {
    let home = TempDir::new().unwrap();
    let expected = home.path().join(".config/advisory-cron/config.toml");

    let (code, stdout, stderr) = run_init(home.path(), false);
    assert_eq!(code, Some(0), "expected exit 0, stderr={stderr}");
    assert!(
        expected.exists(),
        "config file not written at {}",
        expected.display()
    );
    assert!(
        stdout.contains("wrote default config"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn init_refuses_overwrite_without_force() {
    let home = TempDir::new().unwrap();
    // First write succeeds.
    let (code, _, _) = run_init(home.path(), false);
    assert_eq!(code, Some(0));

    // Second write without --force → exit 2.
    let (code, _, stderr) = run_init(home.path(), false);
    assert_eq!(
        code,
        Some(2),
        "expected exit 2 for existing-file error, stderr={stderr}"
    );
    assert!(
        stderr.contains("--force"),
        "stderr should mention --force, got: {stderr}"
    );
}

#[test]
fn init_overwrites_with_force() {
    let home = TempDir::new().unwrap();
    let (code, _, _) = run_init(home.path(), false);
    assert_eq!(code, Some(0));
    let (code, _, stderr) = run_init(home.path(), true);
    assert_eq!(
        code,
        Some(0),
        "expected exit 0 with --force, stderr={stderr}"
    );
}

#[test]
fn init_creates_parseable_toml() {
    let home = TempDir::new().unwrap();
    let (code, _, _) = run_init(home.path(), false);
    assert_eq!(code, Some(0));
    let path = home.path().join(".config/advisory-cron/config.toml");
    let raw = std::fs::read_to_string(&path).unwrap();
    // Must contain all three section headers.
    assert!(raw.contains("[task]"), "missing [task] section");
    assert!(raw.contains("[schedule]"), "missing [schedule] section");
    assert!(raw.contains("[heartbeat]"), "missing [heartbeat] section");
    // Default schedule is Calendar { hour=9, minute=0 }.
    assert!(
        raw.contains("hour") || raw.contains("cron"),
        "schedule section looks empty: {raw}"
    );
}
