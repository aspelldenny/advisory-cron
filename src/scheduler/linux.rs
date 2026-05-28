//! Phase 3.2 — Linux crontab scheduler (P013).
//!
//! `register`: `crontab -l` (tolerate "no crontab for <user>" stderr) → filter existing tagged line →
//! append `<minute> <hour> * * * <self_exe> run # advisory-cron: <label>` →
//! pipe back via `crontab -` (stdin).
//!
//! `unregister`: same flow, omit append; report whether tag was found.
//!
//! `status`: `crontab -l` → grep for tagged line → return raw line in `raw_descriptor`.
//!
//! **INV-22 defense-in-depth point 2**: each method validates `label` via `super::is_valid_label` first.
//! Point 1 lives at `core::*::run` (pre-flight per INV-12 — same allowlist; covers INV-22 transitively).
//!
//! **Cron form constraint**: P013 builds only daily form `<min> <hour> * * *` (mirrors macOS Phase 1 `M H * * *`
//! constraint per ARCHITECTURE.md §Cron mechanism). Full 5-field cron deferred to P014 INV-23.
//!
//! **V2 sync stdlib (Debate Log Turn 1+2)**: `Scheduler` trait methods are sync; this module uses
//! `std::process::Command` (blocking) for `crontab -l` and `crontab -` shell-outs. No tokio runtime,
//! no `io-util` feature, no nested-runtime panic. Blocking I/O is acceptable for ~3 crontab calls/day (~10ms each).

use anyhow::{Context, Result, bail};
use std::io::Write;
use std::process::{Command, Stdio};

use super::{
    RegisterIntent, RegisterReport, Scheduler, SchedulerStatus, UnregisterReport, is_valid_label,
};

/// Tag prefix used to mark advisory-cron-managed lines in user crontab.
/// Format: `<cron_expr> <command> # advisory-cron: <label>`.
const TAG_PREFIX: &str = "# advisory-cron: ";

#[derive(Debug, Default)]
pub struct CrontabScheduler;

impl Scheduler for CrontabScheduler {
    fn register(&self, intent: &RegisterIntent) -> Result<RegisterReport> {
        // INV-22 defense-in-depth point 2.
        if !is_valid_label(&intent.label) {
            bail!(
                "invalid label {:?} — must be ASCII alphanumeric + '-' + '_'",
                intent.label
            );
        }

        // Read existing crontab (tolerate "no crontab for <user>" stderr).
        let existing = read_user_crontab()?;

        // Filter out any prior tagged line for this label (idempotent re-register).
        let tag = format!("{TAG_PREFIX}{}", intent.label);
        let mut lines: Vec<&str> = existing
            .lines()
            .filter(|line| !line.contains(&tag))
            .collect();

        // Build new managed line.
        let new_line = format!(
            "{} {} * * * {} run # advisory-cron: {}",
            intent.minute,
            intent.hour,
            intent.self_exe.display(),
            intent.label,
        );
        lines.push(&new_line);

        // Pipe combined output back via `crontab -`.
        let combined = format!("{}\n", lines.join("\n"));
        write_user_crontab(&combined)?;

        Ok(RegisterReport { plist_path: None })
    }

    fn unregister(&self, label: &str) -> Result<UnregisterReport> {
        // INV-22 defense-in-depth point 2.
        if !is_valid_label(label) {
            bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        }

        let existing = read_user_crontab()?;

        let tag = format!("{TAG_PREFIX}{label}");
        let mut found = false;
        let kept: Vec<&str> = existing
            .lines()
            .filter(|line| {
                let is_tagged = line.contains(&tag);
                if is_tagged {
                    found = true;
                }
                !is_tagged
            })
            .collect();

        if found {
            let combined = if kept.is_empty() {
                String::new()
            } else {
                format!("{}\n", kept.join("\n"))
            };
            write_user_crontab(&combined)?;
        }

        Ok(UnregisterReport {
            was_registered: found,
        })
    }

    fn status(&self, label: &str) -> Result<SchedulerStatus> {
        // INV-22 defense-in-depth point 2.
        if !is_valid_label(label) {
            bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        }

        let existing = match read_user_crontab() {
            Ok(s) => s,
            Err(_) => {
                // "no crontab for user" or other crontab read failure → silent fallback.
                // Mirrors macOS bootout idempotent "Boot-out failed: 3: No such process" pattern.
                return Ok(SchedulerStatus {
                    is_registered: false,
                    raw_descriptor: None,
                });
            }
        };

        let tag = format!("{TAG_PREFIX}{label}");
        let matched = existing.lines().find(|line| line.contains(&tag));

        Ok(SchedulerStatus {
            is_registered: matched.is_some(),
            raw_descriptor: matched.map(String::from),
        })
    }
}

/// Read user crontab via `crontab -l` (sync). Tolerates "no crontab for <user>" stderr by returning empty string.
///
/// **Worker verify in Task 0**: exact substring observed on the dev host is `"no crontab for sep"`.
/// Lowercase substring `"no crontab"` covers variants across distros.
fn read_user_crontab() -> Result<String> {
    let output = Command::new("crontab")
        .arg("-l")
        .output()
        .context("failed to invoke `crontab -l`")?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    // Non-zero exit: check if it's the benign "no crontab" case.
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("no crontab") {
        // User has no crontab — treat as empty input for register flow.
        return Ok(String::new());
    }

    // Other non-zero exit: surface the error.
    bail!(
        "`crontab -l` failed (exit {:?}): {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Write user crontab via `crontab -` (sync stdin pipe).
///
/// **Race condition note (Architect-acknowledged)**: between `read_user_crontab` and `write_user_crontab`,
/// another process could modify the user's crontab. P013 accepts last-writer-wins. Future hardening
/// (advisory locking via `flock(2)` on a sentinel file) deferred — out of scope per BACKLOG Phase 3 acceptance.
fn write_user_crontab(content: &str) -> Result<()> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn `crontab -` for stdin write")?;

    {
        let mut stdin = child
            .stdin
            .take()
            .context("failed to acquire stdin handle for `crontab -`")?;
        stdin
            .write_all(content.as_bytes())
            .context("failed to write content to `crontab -` stdin")?;
        // Drop stdin to close pipe → signal EOF to crontab.
    }

    let output = child
        .wait_with_output()
        .context("failed to wait for `crontab -` to complete")?;

    if !output.status.success() {
        bail!(
            "`crontab -` failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for `CrontabScheduler` — invalid-label rejection. Full integration
    //! (mock `crontab` binary in PATH + happy-path flows) lives in `tests/cli_register_linux.rs`.

    use super::*;

    fn make_intent(label: &str) -> RegisterIntent {
        RegisterIntent {
            label: label.into(),
            hour: 9,
            minute: 0,
            self_exe: std::path::PathBuf::from("/usr/local/bin/advisory-cron"),
            working_dir: std::path::PathBuf::from("/tmp"),
        }
    }

    #[test]
    fn register_rejects_empty_label() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn register_rejects_label_with_semicolon() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("foo;evil")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn register_rejects_label_with_hash() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("foo#bar")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn register_rejects_label_with_newline() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("foo\nbar")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn unregister_rejects_invalid_label() {
        let s = CrontabScheduler;
        let err = s.unregister("foo$bar").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn status_rejects_invalid_label() {
        let s = CrontabScheduler;
        let err = s.status("foo|bar").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }
}
