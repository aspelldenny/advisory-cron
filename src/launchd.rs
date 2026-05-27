//! Phase 1.3 — launchd plist generation + `launchctl` bootstrap/bootout wrappers.
//!
//! macOS-specific. Linux deferred to Phase 3 (systemd timer / cron-tab).
//!
//! Public surface:
//! - `generate_plist(config, label, self_exe)` — pure XML string builder
//! - `plist_path_for(label, launch_agents_dir)` — compose absolute plist path
//! - `default_launch_agents_dir(home)` — `<home>/Library/LaunchAgents`
//! - `LaunchctlClient` trait — `bootstrap`/`bootout` abstraction
//! - `RealLaunchctl` — production impl using `std::process::Command`
//! - `NoopLaunchctl` — test impl recording calls (pub; integration tests may import)
//! - `current_uid()` — POSIX `id -u` shell-out (zero-unsafe, zero-dep)

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{Config, ScheduleConfig};

/// Compose absolute plist path: `<launch_agents_dir>/com.advisorycron.<label>.plist`.
pub fn plist_path_for(label: &str, launch_agents_dir: &Path) -> PathBuf {
    launch_agents_dir.join(format!("com.advisorycron.{label}.plist"))
}

/// Default user LaunchAgents directory: `<home>/Library/LaunchAgents/`.
pub fn default_launch_agents_dir(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
}

/// Generate launchd plist XML for a configured task.
///
/// `config` provides `task.working_dir` + `schedule`.
/// `label` becomes the `Label` key suffix (full label = `com.advisorycron.<label>`).
/// `self_exe` is the absolute path to the `advisory-cron` binary launchd will fire
/// (it invokes `<self_exe> run`).
///
/// Returns: UTF-8 plist XML string matching `docs/ARCHITECTURE.md` §Cron mechanism spec.
///
/// Errors: if `config.schedule` is `Cron` variant with an expression not parseable as
/// `"M H * * *"` form (launchd has no native crontab support — only `StartCalendarInterval`).
pub fn generate_plist(config: &Config, label: &str, self_exe: &Path) -> Result<String> {
    let (hour, minute) = match &config.schedule {
        ScheduleConfig::Calendar { hour, minute } => (*hour, *minute),
        ScheduleConfig::Cron { cron } => parse_simple_cron(cron)?,
    };

    // Sanitize: label MUST be safe for filesystem + reverse-DNS.
    // register::run also validates upstream — defense-in-depth here.
    if !label
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("label must contain only ASCII alphanumeric / '-' / '_' (got {label:?})");
    }

    let full_label = format!("com.advisorycron.{label}");
    let stdout_path = format!("/tmp/advisory-cron-{label}.stdout.log");
    let stderr_path = format!("/tmp/advisory-cron-{label}.stderr.log");

    // XML escape WorkingDirectory + self_exe (paths may contain `&`, `<`, `>` though rare on macOS).
    let working_dir_xml = xml_escape(&config.task.working_dir.display().to_string());
    let self_exe_xml = xml_escape(&self_exe.display().to_string());

    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{full_label}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{self_exe_xml}</string>
        <string>run</string>
    </array>

    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key><integer>{hour}</integer>
        <key>Minute</key><integer>{minute}</integer>
    </dict>

    <key>StandardOutPath</key>
    <string>{stdout_path}</string>

    <key>StandardErrorPath</key>
    <string>{stderr_path}</string>

    <key>WorkingDirectory</key>
    <string>{working_dir_xml}</string>

    <key>RunAtLoad</key>
    <false/>
</dict>
</plist>
"#,
    ))
}

/// Parse cron expression in simple `M H * * *` form (Minute, Hour, daily) → (hour, minute) tuple.
///
/// launchd has no native crontab — only `StartCalendarInterval` (Hour/Minute/Day/etc). For Phase 1
/// we support ONLY the daily-fire simple form. Complex cron (ranges, lists, day-of-week) → error.
fn parse_simple_cron(expr: &str) -> Result<(u8, u8)> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        bail!(
            "cron expression must be 5 fields (got {n}): {expr:?}. \
             launchd Phase 1 supports only `M H * * *` daily form; \
             use [schedule] hour/minute for arbitrary times.",
            n = parts.len()
        );
    }
    // Enforce daily form: minute and hour numeric, day/month/dow all `*`.
    if parts[2] != "*" || parts[3] != "*" || parts[4] != "*" {
        bail!(
            "Phase 1 launchd cron support requires day/month/dow all `*` (daily fire). \
             Got: {expr:?}. Use [schedule] hour/minute in config for arbitrary schedules."
        );
    }
    let minute: u8 = parts[0].parse().with_context(|| {
        format!(
            "cron minute field must be 0..=59 numeric (got {:?})",
            parts[0]
        )
    })?;
    let hour: u8 = parts[1].parse().with_context(|| {
        format!(
            "cron hour field must be 0..=23 numeric (got {:?})",
            parts[1]
        )
    })?;
    if hour > 23 {
        bail!("cron hour must be 0..=23 (got {hour})");
    }
    if minute > 59 {
        bail!("cron minute must be 0..=59 (got {minute})");
    }
    Ok((hour, minute))
}

/// Minimal XML escape for `&`, `<`, `>`, `"`. Plist content typically file paths.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Output of `launchctl print gui/<uid>/com.advisorycron.<label>`.
/// Returned by `LaunchctlClient::print`.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchctlPrintOutput {
    /// Full stdout captured from launchctl. Caller parses for "Hour"/"Minute"
    /// keys inside the `descriptor` block (V2 spec — macOS 15 launchctl does
    /// not expose a "next fire" timestamp, only the configured recurrence).
    pub raw_stdout: String,
    /// True when stderr indicated "Could not find service" — label is not currently loaded.
    /// Caller renders "not loaded" status instead of attempting to parse `raw_stdout`.
    pub not_loaded: bool,
}

/// Abstraction over `launchctl` shell-out — production uses real launchctl, tests inject NoopLaunchctl.
pub trait LaunchctlClient {
    /// `launchctl bootstrap gui/<uid> <plist_path>`.
    // existing methods — DO NOT change signatures (V2 fix: bootstrap is 1-arg)
    fn bootstrap(&self, plist_path: &Path) -> Result<()>; // 1 arg — domain computed internally via current_uid() per P003 V2

    /// `launchctl bootout gui/<uid>/<label>`.
    ///
    /// Returns Ok even if launchctl reports "not loaded" — caller decides idempotency.
    /// Errors only on hard launchctl failures (binary missing, spawn fail).
    /// Worker MUST capture stdout+stderr in returned error for diagnostics.
    fn bootout(&self, label: &str) -> Result<()>; // unchanged from P003

    /// Query launchd for the loaded job's status. Returns raw stdout for the caller
    /// to parse (parse format is system-version dependent — see P005 V2 parse_next_fire).
    /// `label` is the bare label (no `com.advisorycron.` prefix and no `gui/<uid>/`).
    /// Per INV-12, `label` MUST be ASCII alphanumeric + `-` + `_` only (caller validates;
    /// implementation re-validates as defense-in-depth).
    fn print(&self, label: &str) -> Result<LaunchctlPrintOutput>;
}

/// Production impl — shells out real `launchctl`.
pub struct RealLaunchctl;

impl LaunchctlClient for RealLaunchctl {
    fn bootstrap(&self, plist_path: &Path) -> Result<()> {
        let uid = current_uid()?;
        let domain = format!("gui/{uid}");
        let out = Command::new("launchctl")
            .arg("bootstrap")
            .arg(&domain)
            .arg(plist_path)
            .output()
            .context("failed to spawn `launchctl bootstrap`")?;
        if !out.status.success() {
            bail!(
                "launchctl bootstrap failed (exit {}): stdout={:?} stderr={:?}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    fn bootout(&self, label: &str) -> Result<()> {
        let uid = current_uid()?;
        let target = format!("gui/{uid}/com.advisorycron.{label}");
        let out = Command::new("launchctl")
            .arg("bootout")
            .arg(&target)
            .output()
            .context("failed to spawn `launchctl bootout`")?;
        if !out.status.success() {
            // V2 note (Anchor #17 empirical): expected stdout when label-not-loaded is
            // "Boot-out failed: 3: No such process" (exit=3). Do NOT branch behavior on
            // substring — caller (unregister::run) treats any Err as warn-continue.
            bail!(
                "launchctl bootout failed (exit {}): stdout={:?} stderr={:?}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    fn print(&self, label: &str) -> Result<LaunchctlPrintOutput> {
        // Defense-in-depth label sanitization (INV-12). Caller in src/cli/status.rs
        // also validates — this is the second of 2 enforcement points.
        if label.is_empty() {
            anyhow::bail!("invalid label — empty string");
        }
        if !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        }

        let uid = current_uid()?;
        let target = format!("gui/{uid}/com.advisorycron.{label}");

        let output = std::process::Command::new("launchctl")
            .arg("print")
            .arg(&target)
            .output()
            .with_context(|| format!("failed to spawn launchctl print {target}"))?;

        // launchctl exits non-zero when service not loaded.
        // Sample stderr (macOS 14+): "Could not find service \"com.advisorycron.<label>\" in domain for ..."
        // Sample stderr (older): "No such process"
        // Treat either substring as "not loaded" — render status accordingly, do NOT bubble error.
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Could not find service") || stderr.contains("No such process") {
                return Ok(LaunchctlPrintOutput {
                    raw_stdout: String::new(),
                    not_loaded: true,
                });
            }
            // Real launchctl error (permission denied, etc.) — bubble up.
            anyhow::bail!(
                "launchctl print {target} failed: exit={:?} stderr={}",
                output.status.code(),
                stderr
            );
        }

        Ok(LaunchctlPrintOutput {
            raw_stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            not_loaded: false,
        })
    }
}

/// Test impl — records calls; never invokes real launchctl.
/// `pub` to allow future lib-target integration tests to import directly.
/// Silenced: pub API not yet constructed in production binary by design.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct NoopLaunchctl {
    pub bootstrap_calls: std::sync::Mutex<Vec<PathBuf>>,
    pub bootout_calls: std::sync::Mutex<Vec<String>>,
}

impl LaunchctlClient for NoopLaunchctl {
    fn bootstrap(&self, plist_path: &Path) -> Result<()> {
        self.bootstrap_calls
            .lock()
            .unwrap()
            .push(plist_path.to_path_buf());
        Ok(())
    }

    fn bootout(&self, label: &str) -> Result<()> {
        self.bootout_calls.lock().unwrap().push(label.to_string());
        Ok(())
    }

    fn print(&self, _label: &str) -> Result<LaunchctlPrintOutput> {
        // Canned output matches macOS 15 launchctl format (Worker Turn 1 captured fixture).
        Ok(LaunchctlPrintOutput {
            raw_stdout: "gui/501/com.advisorycron.test = {\n\
                \tstate = not running\n\
                \tevent triggers = {\n\
                \t\tcom.advisorycron.test.268435522 => {\n\
                \t\t\tstream = com.apple.launchd.calendarinterval\n\
                \t\t\tdescriptor = {\n\
                \t\t\t\t\"Minute\" => 0\n\
                \t\t\t\t\"Hour\" => 9\n\
                \t\t\t}\n\
                \t\t}\n\
                \t}\n\
                }"
            .to_string(),
            not_loaded: false,
        })
    }
}

/// Resolve current UID via POSIX `id -u`. Zero-unsafe alternative to `libc::getuid()`.
/// Sub-100ms cost — acceptable for one-shot register/unregister CLI ops.
pub fn current_uid() -> Result<u32> {
    let out = Command::new("id")
        .arg("-u")
        .output()
        .context("failed to spawn `id -u`")?;
    if !out.status.success() {
        bail!(
            "`id -u` exited non-zero: stderr={:?}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    s.parse::<u32>()
        .with_context(|| format!("failed to parse UID from `id -u` output: {s:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HeartbeatConfig, TaskConfig};
    use std::path::PathBuf;

    fn sample_config_calendar() -> Config {
        Config {
            task: TaskConfig {
                command: "claude".into(),
                args: vec!["-p".into(), "/advisory-scan".into()],
                working_dir: PathBuf::from("/Users/test"),
                label: None,
            },
            schedule: ScheduleConfig::Calendar { hour: 9, minute: 0 },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/Users/test/.local/state/advisory-cron/heartbeat.jsonl"),
            },
        }
    }

    fn sample_config_cron(expr: &str) -> Config {
        let mut c = sample_config_calendar();
        c.schedule = ScheduleConfig::Cron { cron: expr.into() };
        c
    }

    #[test]
    fn generate_plist_calendar_contains_all_required_keys() {
        let cfg = sample_config_calendar();
        let xml = generate_plist(&cfg, "test", Path::new("/usr/local/bin/advisory-cron"))
            .expect("calendar schedule should generate");
        for needle in [
            "<key>Label</key>",
            "<string>com.advisorycron.test</string>",
            "<key>ProgramArguments</key>",
            "<string>/usr/local/bin/advisory-cron</string>",
            "<string>run</string>",
            "<key>StartCalendarInterval</key>",
            "<key>Hour</key><integer>9</integer>",
            "<key>Minute</key><integer>0</integer>",
            "<key>StandardOutPath</key>",
            "<string>/tmp/advisory-cron-test.stdout.log</string>",
            "<key>StandardErrorPath</key>",
            "<string>/tmp/advisory-cron-test.stderr.log</string>",
            "<key>WorkingDirectory</key>",
            "<string>/Users/test</string>",
            "<key>RunAtLoad</key>",
            "<false/>",
        ] {
            assert!(
                xml.contains(needle),
                "plist missing required substring {needle:?}:\n{xml}"
            );
        }
        // DOCTYPE present
        assert!(xml.contains("<!DOCTYPE plist PUBLIC"));
    }

    #[test]
    fn generate_plist_cron_simple_daily_form_works() {
        let cfg = sample_config_cron("30 14 * * *");
        let xml = generate_plist(&cfg, "test", Path::new("/bin/x")).unwrap();
        assert!(xml.contains("<key>Hour</key><integer>14</integer>"));
        assert!(xml.contains("<key>Minute</key><integer>30</integer>"));
    }

    #[test]
    fn generate_plist_cron_complex_expression_errors() {
        let cfg = sample_config_cron("*/15 9-17 * * 1-5");
        let err = generate_plist(&cfg, "test", Path::new("/bin/x")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("daily") || msg.contains("numeric") || msg.contains("`*`"),
            "unexpected error msg: {msg}"
        );
    }

    #[test]
    fn generate_plist_cron_wrong_field_count_errors() {
        let cfg = sample_config_cron("0 9 *");
        assert!(generate_plist(&cfg, "test", Path::new("/bin/x")).is_err());
    }

    #[test]
    fn generate_plist_rejects_invalid_label() {
        let cfg = sample_config_calendar();
        for bad in [
            "bad label",
            "bad.label",
            "bad/label",
            "bad@label",
            "bad$label",
        ] {
            assert!(
                generate_plist(&cfg, bad, Path::new("/bin/x")).is_err(),
                "label {bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn generate_plist_accepts_valid_labels() {
        let cfg = sample_config_calendar();
        for good in ["test", "advisory-scan", "daily_report", "Test123"] {
            assert!(
                generate_plist(&cfg, good, Path::new("/bin/x")).is_ok(),
                "label {good:?} should be accepted"
            );
        }
    }

    #[test]
    fn plist_path_for_composes_label_correctly() {
        let p = plist_path_for("scan", Path::new("/tmp/LaunchAgents"));
        assert_eq!(
            p,
            PathBuf::from("/tmp/LaunchAgents/com.advisorycron.scan.plist")
        );
    }

    #[test]
    fn default_launch_agents_dir_computes_user_path() {
        let p = default_launch_agents_dir(Path::new("/Users/x"));
        assert_eq!(p, PathBuf::from("/Users/x/Library/LaunchAgents"));
    }

    #[test]
    fn noop_launchctl_records_calls() {
        let n = NoopLaunchctl::default();
        n.bootstrap(Path::new("/tmp/foo.plist")).unwrap();
        n.bootout("scan").unwrap();
        assert_eq!(n.bootstrap_calls.lock().unwrap().len(), 1);
        assert_eq!(n.bootout_calls.lock().unwrap()[0], "scan");
    }

    #[test]
    fn xml_escape_handles_special_chars() {
        assert_eq!(xml_escape("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d&quot;e");
    }

    #[test]
    fn current_uid_returns_nonzero() {
        // dev machine UID is non-zero (501 on macOS user, 1000 on Linux user typically).
        // Test confirms shell-out works; doesn't assert exact value.
        let uid = current_uid().expect("id -u must work in test env");
        assert!(uid > 0, "expected non-root UID in test env, got {uid}");
    }

    #[test]
    fn noop_launchctl_print_returns_canned_descriptor_output() {
        let client = NoopLaunchctl::default(); // V2 fix: unit struct uses Default
        let result = client.print("test-label").expect("noop never fails");
        assert!(!result.not_loaded);
        // V2: assert descriptor Hour/Minute keys present (macOS 15 format)
        assert!(result.raw_stdout.contains("\"Hour\" => 9"));
        assert!(result.raw_stdout.contains("\"Minute\" => 0"));
    }

    #[test]
    fn real_launchctl_print_rejects_invalid_label() {
        let client = RealLaunchctl; // V2 fix: unit struct, no ::new()
        let result = client.print("../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err:#}").contains("invalid label"));
    }

    #[test]
    fn real_launchctl_print_rejects_empty_label() {
        let client = RealLaunchctl; // V2 fix: unit struct, no ::new()
        let result = client.print("");
        assert!(result.is_err());
    }
}
