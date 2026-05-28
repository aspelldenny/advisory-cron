//! Core unregister logic — delegate to Scheduler.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::unregister` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.
//!
//! V2 (per Architect Turn 1 RESPOND [O1.3] ACCEPT):
//! - Resolves `home`, `launch_agents_dir` internally (now inside Scheduler impl).
//! - ONLY `&S: Scheduler` is injected.
//!
//! Phase 3.1 (P012): generic `<L: LaunchctlClient>` → `<S: Scheduler>`.
//! `UnregisterOutput` keeps both `plist_existed` + `was_loaded` fields populated from
//! `was_registered` (minimum-disruption refactor — MCP JSON schema stable).

use crate::scheduler::Scheduler;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct UnregisterArgs {
    /// Label suffix (full label = com.advisorycron.<label>).
    pub label: String,
    /// Reserved for future use; currently unused (CLI parity with register).
    #[allow(dead_code)]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnregisterOutput {
    pub label: String,
    pub plist_existed: bool,
    pub was_loaded: bool,
}

/// Validate label allowlist (INV-12 enforcement point in core).
pub fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// V2 (per Architect Turn 1 RESPOND [O1.3] ACCEPT):
/// - Resolves home + OS-specific unregister logic inside scheduler.
/// - Idempotent: "not loaded" + "plist not found" are NOT errors (scheduler handles).
///
/// Phase 3.1: delegates to `scheduler.unregister()`.
/// `UnregisterOutput.plist_existed` + `was_loaded` both populated from `was_registered`
/// for backwards-compat: cli/unregister.rs:44,:49 warning render still triggers.
pub fn run<S: Scheduler>(args: UnregisterArgs, scheduler: &S) -> Result<UnregisterOutput> {
    // 1. Validate label (INV-12).
    if !is_valid_label(&args.label) {
        anyhow::bail!(
            "invalid label {:?} — must be ASCII alphanumeric + '-' + '_'",
            args.label
        );
    }

    // 2. Delegate to scheduler.
    let report = scheduler
        .unregister(&args.label)
        .context("scheduler unregister failed")?;

    Ok(UnregisterOutput {
        label: args.label,
        // Phase 3.1: collapse `plist_existed` + `was_loaded` → `was_registered`.
        // Backwards-compat: populate both old fields from `was_registered` so CLI render
        // (cli/unregister.rs:44, :49 — warning messages) still triggers.
        plist_existed: report.was_registered,
        was_loaded: report.was_registered,
    })
}

// Pull in Context trait for `.context()` usage above.
use anyhow::Context;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::NoopScheduler;
    use tempfile::TempDir;

    #[test]
    fn run_idempotent_when_plist_absent() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let scheduler = NoopScheduler::default();
        let result = run(
            UnregisterArgs {
                label: "test-label".to_string(),
                config_path: None,
            },
            &scheduler,
        );
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let output = result.unwrap();
        // NoopScheduler::unregister returns was_registered=false → plist_existed=false
        assert!(!output.plist_existed);
    }

    #[test]
    fn run_rejects_invalid_label() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let scheduler = NoopScheduler::default();
        let result = run(
            UnregisterArgs {
                label: "bad label!".to_string(),
                config_path: None,
            },
            &scheduler,
        );
        assert!(result.is_err());
    }
}
