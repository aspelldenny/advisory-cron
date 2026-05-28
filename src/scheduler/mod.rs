//! Phase 3.1 — Cross-OS scheduler abstraction.
//!
//! Replaces Phase 1.3 `src/launchd.rs::LaunchctlClient`. macOS impl in `macos.rs`
//! (launchd via launchctl); Linux impl in `linux.rs` (crontab — stub P012, real P013).
//!
//! Compile-time dispatch: `PlatformScheduler` alias resolves to `MacosScheduler` on
//! macOS targets, `CrontabScheduler` on Linux. Other OSes do not compile (Phase 3 = macOS + Linux only).

use anyhow::Result;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

/// High-level intent passed to `Scheduler::register`. Abstracts plist-vs-crontab.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RegisterIntent {
    /// Bare label (full launchd label / crontab tag = `com.advisorycron.<label>` / `# advisory-cron: <label>`).
    pub label: String,
    /// Hour 0..=23 + minute 0..=59 — daily form only (Phase 1 / Phase 3.1 constraint).
    /// Phase 3.2 P013 may extend with full cron expression for Linux only; macOS stays daily.
    pub hour: u8,
    pub minute: u8,
    /// Absolute path to `advisory-cron` binary (resolved by core via `env::current_exe()`).
    pub self_exe: PathBuf,
    /// Working directory for the fired task.
    pub working_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RegisterReport {
    /// macOS: path to written plist file. Linux (P013): None. Surfaced for CLI render.
    pub plist_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct UnregisterReport {
    /// True if the scheduler had a registration matching `label` before this call.
    /// macOS: true if launchctl had the job loaded OR the plist file existed.
    /// Linux (P013): true if a tagged crontab line existed.
    pub was_registered: bool,
}

#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    /// True if the scheduler currently has a registration for `label`.
    pub is_registered: bool,
    /// Raw scheduler-specific descriptor for downstream parsing.
    /// macOS: `launchctl print` stdout (parsed by `core::status::parse_next_fire`).
    /// Linux (P013): matched crontab line (parsed by future `parse_cron_next_fire`).
    pub raw_descriptor: Option<String>,
}

/// Cross-OS label allowlist — ASCII alphanumeric + `-` + `_`, non-empty.
///
/// Used by:
/// - `scheduler::macos::MacosScheduler::unregister` (INV-12 defense-in-depth point 2 — same allowlist as `generate_plist`).
/// - `scheduler::linux::CrontabScheduler::{register, unregister, status}` (INV-22 defense-in-depth point 2 — pre-flight at `core::*` is point 1).
///
/// **Why a tight allowlist instead of a metachar blacklist:** the allowlist excludes ALL whitespace,
/// path separators (`.`, `/`, `~`), shell meta-chars (`$`, `` ` ``, `&`, `;`, `|`, `#`), quote chars (`'`, `"`),
/// AND newlines — covers both launchd domain-target injection (INV-10/12/17) AND crontab tag-line injection
/// (INV-22) without enumeration. Single source of truth; less to forget.
pub fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Cross-OS scheduling abstraction. macOS = launchd; Linux = crontab.
pub trait Scheduler {
    /// Register a recurring task. Idempotent on re-register (overwrites existing registration).
    fn register(&self, intent: &RegisterIntent) -> Result<RegisterReport>;

    /// Unregister by label. Idempotent: returns `was_registered=false` if no prior registration.
    fn unregister(&self, label: &str) -> Result<UnregisterReport>;

    /// Query registration state + raw descriptor for next-fire parsing.
    fn status(&self, label: &str) -> Result<SchedulerStatus>;
}

#[cfg(target_os = "macos")]
pub use macos::MacosScheduler as PlatformScheduler;

#[cfg(target_os = "linux")]
pub use linux::CrontabScheduler as PlatformScheduler;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("advisory-cron Phase 3 supports macOS + Linux only");

#[cfg(test)]
mod tests {
    use super::is_valid_label;

    #[test]
    fn accepts_alphanumeric_hyphen_underscore() {
        assert!(is_valid_label("advisory-scan_daily"));
        assert!(is_valid_label("foo"));
        assert!(is_valid_label("F00"));
        assert!(is_valid_label("a-b_c-d"));
    }

    #[test]
    fn rejects_empty() {
        assert!(!is_valid_label(""));
    }

    #[test]
    fn rejects_shell_metacharacters() {
        for label in [
            "foo;bar", "foo$bar", "foo|bar", "foo&bar", "foo`bar`", "foo'bar", "foo\"bar",
            "foo#bar", "foo bar", "foo\nbar", "foo/bar", "foo.bar", "foo~bar", "../etc",
        ] {
            assert!(!is_valid_label(label), "expected rejection for {label:?}");
        }
    }

    #[test]
    fn rejects_unicode() {
        assert!(!is_valid_label("café"));
        assert!(!is_valid_label("日本語"));
    }
}

// ---- NoopScheduler (test impl — replaces NoopLaunchctl) ----

/// Test impl that records calls. Used by `core::*::tests` + (future) lib tests.
/// `pub` to allow integration test crate to import directly.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct NoopScheduler {
    pub register_calls: std::sync::Mutex<Vec<RegisterIntent>>,
    pub unregister_calls: std::sync::Mutex<Vec<String>>,
    pub status_calls: std::sync::Mutex<Vec<String>>,
}

impl Scheduler for NoopScheduler {
    fn register(&self, intent: &RegisterIntent) -> Result<RegisterReport> {
        self.register_calls.lock().unwrap().push(intent.clone());
        Ok(RegisterReport { plist_path: None })
    }

    fn unregister(&self, label: &str) -> Result<UnregisterReport> {
        self.unregister_calls
            .lock()
            .unwrap()
            .push(label.to_string());
        Ok(UnregisterReport {
            was_registered: false,
        })
    }

    fn status(&self, label: &str) -> Result<SchedulerStatus> {
        self.status_calls.lock().unwrap().push(label.to_string());
        // Canned descriptor matches macOS 15 launchctl format (preserve test compat).
        Ok(SchedulerStatus {
            is_registered: true,
            raw_descriptor: Some(
                "descriptor = {\n\t\"Minute\" => 0\n\t\"Hour\" => 9\n}".to_string(),
            ),
        })
    }
}
