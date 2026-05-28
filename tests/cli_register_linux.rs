//! Integration tests for `CrontabScheduler` happy-path flows.
//! Gated `#[cfg(target_os = "linux")]`. Mock `crontab` binary in PATH.

#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

/// Create a mock `crontab` binary that proxies state via a file.
/// Returns (mock_dir, state_file_path).
fn make_mock_crontab(td: &TempDir) -> (PathBuf, PathBuf) {
    let state = td.path().join("mock_crontab.state");
    let script = td.path().join("crontab");

    // POSIX shell mock: handles `crontab -l` and `crontab -`.
    let body = format!(
        r#"#!/bin/sh
case "$1" in
  -l)
    if [ -f "{state}" ]; then
      cat "{state}"
    else
      echo "no crontab for $USER" >&2
      exit 1
    fi
    ;;
  -)
    cat > "{state}"
    ;;
  *)
    echo "mock_crontab: unknown arg $1" >&2
    exit 2
    ;;
esac
"#,
        state = state.display(),
    );
    fs::write(&script, body).expect("write mock crontab");
    let mut perm = fs::metadata(&script).unwrap().permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&script, perm).unwrap();
    (td.path().to_path_buf(), state)
}

/// Prepend `dir` to PATH for a Command invocation.
fn with_mock_path(cmd: &mut Command, mock_dir: &std::path::Path) {
    let existing = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{}:{}", mock_dir.display(), existing));
}

fn write_minimal_config(td: &TempDir, label: &str) -> PathBuf {
    // Write a minimal valid config so `advisory-cron register` can load it.
    let cfg_path = td.path().join("config.toml");
    let cfg = format!(
        r#"
[task]
command = "echo"
args = ["hello"]
working_dir = "{wd}"
label = "{label}"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{hb}"
"#,
        wd = td.path().display(),
        hb = td.path().join("heartbeat.jsonl").display(),
        label = label,
    );
    fs::write(&cfg_path, cfg).expect("write test config");
    cfg_path
}

#[test]
fn register_writes_one_tagged_line() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test1");

    let mut cmd = Command::new(BIN);
    cmd.args(["register", "--label", "p013-test1", "--config"])
        .arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    let out = cmd.output().expect("spawn advisory-cron");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let written = fs::read_to_string(&state).expect("state written");
    let tagged: Vec<&str> = written
        .lines()
        .filter(|l| l.contains("# advisory-cron: p013-test1"))
        .collect();
    assert_eq!(
        tagged.len(),
        1,
        "expected exactly 1 tagged line, got: {written}"
    );
}

#[test]
fn unregister_removes_tagged_line() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test2");

    // Pre-populate state with a tagged line + an unrelated user line.
    fs::write(
        &state,
        "0 12 * * * /usr/bin/echo user-line\n0 9 * * * /bin/advisory-cron run # advisory-cron: p013-test2\n",
    )
    .unwrap();

    let mut cmd = Command::new(BIN);
    cmd.args(["unregister", "--label", "p013-test2"])
        .arg("--config")
        .arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    let out = cmd.output().expect("spawn advisory-cron");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after = fs::read_to_string(&state).expect("state written");
    assert!(
        !after.contains("p013-test2"),
        "tagged line still present: {after}"
    );
    assert!(
        after.contains("user-line"),
        "user line was clobbered: {after}"
    );
}

#[test]
fn idempotent_re_register_replaces_not_duplicates() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test3");

    // Register twice with same label.
    for _ in 0..2 {
        let mut cmd = Command::new(BIN);
        cmd.args(["register", "--label", "p013-test3"])
            .arg("--config")
            .arg(&cfg);
        with_mock_path(&mut cmd, &mock_dir);
        let out = cmd.output().expect("spawn");
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let written = fs::read_to_string(&state).expect("state written");
    let tagged: Vec<&str> = written
        .lines()
        .filter(|l| l.contains("# advisory-cron: p013-test3"))
        .collect();
    assert_eq!(
        tagged.len(),
        1,
        "expected exactly 1 tagged line after 2 registers, got: {written}"
    );
}

#[test]
fn status_reports_loaded_after_register() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test4");

    // Register first.
    let mut reg = Command::new(BIN);
    reg.args(["register", "--label", "p013-test4"])
        .arg("--config")
        .arg(&cfg);
    with_mock_path(&mut reg, &mock_dir);
    reg.output().expect("register");

    // Confirm state file has the line (sanity).
    assert!(
        fs::read_to_string(&state)
            .unwrap()
            .contains("# advisory-cron: p013-test4")
    );

    // Now run status --json.
    let mut st = Command::new(BIN);
    st.args(["status", "--label", "p013-test4", "--config"])
        .arg(&cfg)
        .arg("--json");
    with_mock_path(&mut st, &mock_dir);
    let out = st.output().expect("status");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(
        parsed["plist_loaded"],
        serde_json::json!(true),
        "got: {stdout}"
    );
}

#[test]
fn status_reports_unloaded_when_crontab_empty() {
    let td = TempDir::new().unwrap();
    let (mock_dir, _state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test5");

    // State file does NOT exist → mock returns "no crontab for user" stderr + exit 1.

    let mut st = Command::new(BIN);
    st.args(["status", "--label", "p013-test5", "--config"])
        .arg(&cfg)
        .arg("--json");
    with_mock_path(&mut st, &mock_dir);
    let out = st.output().expect("status");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(
        parsed["plist_loaded"],
        serde_json::json!(false),
        "got: {stdout}"
    );
}

#[test]
fn invalid_label_rejected_preflight_before_crontab_spawned() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "valid-cfg-label");

    // Mock state initially empty — if mock IS invoked, state would change.
    // We assert state is still absent (= mock never invoked) = pre-flight rejected before shell-out.

    let mut cmd = Command::new(BIN);
    cmd.args(["register", "--label", "foo;evil"])
        .arg("--config")
        .arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    let out = cmd.output().expect("spawn advisory-cron");
    assert!(
        !out.status.success(),
        "expected non-zero exit for invalid label"
    );
    assert!(
        !state.exists(),
        "mock crontab was invoked despite invalid label — INV-22 point 1 violated"
    );
}

#[test]
fn preserves_unrelated_user_cron_lines() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test7");

    // Pre-populate with user's own cron entries.
    fs::write(
        &state,
        "# user comment\n0 12 * * * /usr/bin/backup\n30 * * * * /usr/local/bin/poll\n",
    )
    .unwrap();

    let mut cmd = Command::new(BIN);
    cmd.args(["register", "--label", "p013-test7"])
        .arg("--config")
        .arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    cmd.output().expect("register");

    let after = fs::read_to_string(&state).unwrap();
    assert!(after.contains("# user comment"), "user comment lost");
    assert!(after.contains("/usr/bin/backup"), "user backup line lost");
    assert!(after.contains("/usr/local/bin/poll"), "user poll line lost");
    assert!(
        after.contains("# advisory-cron: p013-test7"),
        "managed line missing"
    );
}
