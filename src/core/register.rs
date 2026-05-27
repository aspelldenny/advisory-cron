//! Core register logic — generate launchd plist + bootstrap.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::register` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.
//!
//! V2 (per Architect Turn 1 RESPOND [O1.3] ACCEPT):
//! - Resolves `home`, `launch_agents_dir`, `self_exe` ALL internally.
//! - ONLY `&L: LaunchctlClient` is injected (preserves testability via NoopLaunchctl).

use crate::config::{Config, ScheduleConfig};
use crate::core::config_path::home_dir;
use crate::launchd::{LaunchctlClient, default_launch_agents_dir, generate_plist, plist_path_for};
use anyhow::{Context, Result};
use std::{env, fs, path::PathBuf};

#[derive(Debug, Clone)]
pub struct RegisterArgs {
    /// Label suffix (full label = com.advisorycron.<label>).
    pub label: String,
    /// Cron expression (`M H * * *` daily form) — overrides config.schedule when present.
    pub schedule: Option<String>,
    /// Override default config path.
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegisterOutput {
    pub plist_path: PathBuf,
    pub label: String,
    pub bootstrapped: bool,
}

/// Validate label allowlist (INV-12 enforcement point in core).
pub fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// V2 (per Architect Turn 1 RESPOND [O1.3] ACCEPT):
/// - Resolves home internally.
/// - Resolves launch_agents_dir internally.
/// - Resolves self_exe internally.
/// - Returns typed error; CLI/MCP layer maps to exit codes.
pub fn run<L: LaunchctlClient>(args: RegisterArgs, client: &L) -> Result<RegisterOutput> {
    // 1. Validate label (INV-12 first enforcement in core).
    if !is_valid_label(&args.label) {
        anyhow::bail!(
            "invalid label {:?} — must be ASCII alphanumeric + '-' + '_'",
            args.label
        );
    }

    // 2. Resolve home + launch_agents_dir internally (V2 [O1.3]).
    let home = home_dir().context("failed to resolve $HOME")?;
    let launch_agents_dir = default_launch_agents_dir(&home);

    // 3. Resolve config path.
    let config_path = args
        .config_path
        .unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"));

    // 4. Load config.
    let mut config = Config::load(&config_path)
        .with_context(|| format!("failed to load config at {}", config_path.display()))?;

    // 5. Apply --schedule CLI override.
    if let Some(cron_expr) = args.schedule {
        config.schedule = ScheduleConfig::Cron { cron: cron_expr };
    }

    // 6. Resolve self_exe internally (V2 [O1.3]).
    let self_exe = env::current_exe().context("failed to resolve current executable path")?;

    // 7. Generate plist XML.
    let plist_xml =
        generate_plist(&config, &args.label, &self_exe).context("failed to generate plist")?;

    // 8. Write plist file.
    fs::create_dir_all(&launch_agents_dir)
        .with_context(|| format!("failed to create {}", launch_agents_dir.display()))?;
    let plist_path = plist_path_for(&args.label, &launch_agents_dir);
    fs::write(&plist_path, &plist_xml)
        .with_context(|| format!("failed to write plist to {}", plist_path.display()))?;

    // 9. Bootstrap via launchctl.
    client
        .bootstrap(&plist_path)
        .context("launchctl bootstrap failed")?;

    Ok(RegisterOutput {
        plist_path,
        label: args.label,
        bootstrapped: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launchd::NoopLaunchctl;
    use tempfile::TempDir;

    /// Helper: write a minimal valid config to a temp path.
    fn write_minimal_config(path: &std::path::Path, home: &std::path::Path) {
        use crate::config::Config;
        Config::write_default(path, home, false).unwrap();
    }

    #[test]
    fn run_registers_successfully_with_noop_launchctl() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        unsafe {
            std::env::set_var("HOME", home);
        }

        // Write config to expected location.
        let config_path = home.join(".config/advisory-cron/config.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        write_minimal_config(&config_path, home);

        let client = NoopLaunchctl::default();
        let result = run(
            RegisterArgs {
                label: "test-label".to_string(),
                schedule: None,
                config_path: Some(config_path),
            },
            &client,
        );
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let output = result.unwrap();
        assert_eq!(output.label, "test-label");
        assert!(output.bootstrapped);
        // Verify plist was bootstrapped once.
        assert_eq!(client.bootstrap_calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn run_rejects_invalid_label() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let client = NoopLaunchctl::default();
        let result = run(
            RegisterArgs {
                label: "bad label!".to_string(),
                schedule: None,
                config_path: None,
            },
            &client,
        );
        assert!(result.is_err());
    }

    // INV-12 specific attack-class tests — verify pre-flight rejection BEFORE
    // LaunchctlClient invocation (bootstrap_calls must be 0 on rejection).

    #[test]
    fn register_rejects_label_with_whitespace() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let client = NoopLaunchctl::default();
        let result = run(
            RegisterArgs {
                label: "foo bar".to_string(),
                schedule: None,
                config_path: None,
            },
            &client,
        );
        assert!(
            result.is_err(),
            "label with whitespace must be rejected at pre-flight (INV-12)"
        );
        assert_eq!(
            client.bootstrap_calls.lock().unwrap().len(),
            0,
            "pre-flight rejection must occur before LaunchctlClient invocation"
        );
    }

    #[test]
    fn register_rejects_label_with_path_separator() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let client = NoopLaunchctl::default();
        let result = run(
            RegisterArgs {
                label: "foo/bar".to_string(),
                schedule: None,
                config_path: None,
            },
            &client,
        );
        assert!(
            result.is_err(),
            "label with `/` must be rejected — path traversal vector (INV-12)"
        );
        assert_eq!(
            client.bootstrap_calls.lock().unwrap().len(),
            0,
            "pre-flight rejection must occur before LaunchctlClient invocation"
        );
    }

    #[test]
    fn register_rejects_label_with_shell_metachar() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let client = NoopLaunchctl::default();
        let result = run(
            RegisterArgs {
                label: "foo;rm".to_string(),
                schedule: None,
                config_path: None,
            },
            &client,
        );
        assert!(
            result.is_err(),
            "label with `;` must be rejected — shell metachar (INV-12)"
        );
        assert_eq!(
            client.bootstrap_calls.lock().unwrap().len(),
            0,
            "pre-flight rejection must occur before LaunchctlClient invocation"
        );
    }
}
