//! Core unregister logic — bootout launchd job + remove plist.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::unregister` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.
//!
//! V2 (per Architect Turn 1 RESPOND [O1.3] ACCEPT):
//! - Resolves `home`, `launch_agents_dir` internally.
//! - ONLY `&L: LaunchctlClient` is injected.

use crate::core::config_path::home_dir;
use crate::launchd::{LaunchctlClient, default_launch_agents_dir, plist_path_for};
use anyhow::{Context, Result};
use std::{fs, io, path::PathBuf};

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
/// - Resolves home + launch_agents_dir internally.
/// - Idempotent: "not loaded" + "plist not found" are NOT errors.
pub fn run<L: LaunchctlClient>(args: UnregisterArgs, client: &L) -> Result<UnregisterOutput> {
    // 1. Validate label (INV-12).
    if !is_valid_label(&args.label) {
        anyhow::bail!(
            "invalid label {:?} — must be ASCII alphanumeric + '-' + '_'",
            args.label
        );
    }

    // 2. Resolve home + launch_agents_dir internally.
    let home = home_dir().context("failed to resolve $HOME")?;
    let launch_agents_dir = default_launch_agents_dir(&home);

    // 3. Check plist existence before attempting removal.
    let plist_path = plist_path_for(&args.label, &launch_agents_dir);
    let plist_existed = plist_path.exists();

    // 4. Attempt bootout. Any Err is treated as warn-continue (idempotency).
    let was_loaded = match client.bootout(&args.label) {
        Ok(()) => true,
        Err(_) => {
            // Label not loaded or already unloaded — continue to plist removal.
            false
        }
    };

    // 5. Remove plist. NotFound → warn, continue. Other IO → Err.
    match fs::remove_file(&plist_path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // File already absent — idempotent, not an error.
        }
        Err(e) => {
            return Err(e)
                .with_context(|| format!("failed to remove plist at {}", plist_path.display()));
        }
    }

    Ok(UnregisterOutput {
        label: args.label,
        plist_existed,
        was_loaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launchd::NoopLaunchctl;
    use tempfile::TempDir;

    #[test]
    fn run_idempotent_when_plist_absent() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let client = NoopLaunchctl::default();
        let result = run(
            UnregisterArgs {
                label: "test-label".to_string(),
                config_path: None,
            },
            &client,
        );
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let output = result.unwrap();
        assert!(!output.plist_existed);
    }

    #[test]
    fn run_rejects_invalid_label() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let client = NoopLaunchctl::default();
        let result = run(
            UnregisterArgs {
                label: "bad label!".to_string(),
                config_path: None,
            },
            &client,
        );
        assert!(result.is_err());
    }
}
