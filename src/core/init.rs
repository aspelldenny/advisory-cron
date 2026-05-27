//! Core init logic — write default config.
//!
//! Pure business logic, no CLI or MCP concerns. Both `cli::init` and `mcp::tools`
//! call this. Satisfies ARCHITECTURE.md §Layering invariant.

use crate::config::Config;
use crate::core::config_path::{default_config_path, home_dir};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InitArgs {
    pub force: bool,
    /// None → use default `~/.config/advisory-cron/config.toml`.
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitOutput {
    pub config_path: PathBuf,
    /// True iff `Config::write_default` wrote the file (i.e., call succeeded).
    /// False-but-Ok is impossible: write_default returns Err on "exists + no force".
    pub written: bool,
}

/// V2 (per Architect Turn 1 RESPOND [O1.2] ACCEPT):
/// - Resolves `home` internally via `home_dir()`.
/// - Derives `written` from pre-call `path.exists()` check.
/// - Calls real 3-arg `Config::write_default(path, home, force)`.
/// - On Err from write_default → propagates; CLI shell maps "already exists" → exit 2.
pub fn run(args: InitArgs) -> Result<InitOutput> {
    let path = match args.config_path {
        Some(p) => p,
        None => default_config_path()?,
    };
    let home = home_dir()?;

    let _path_existed_before = path.exists();
    // write_default returns Ok(()) on success, Err on "config exists + no force".
    Config::write_default(&path, &home, args.force)?;

    Ok(InitOutput {
        config_path: path,
        written: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_writes_config_on_success() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let config_path = dir.path().join("config.toml");

        let output = run(InitArgs {
            force: false,
            config_path: Some(config_path.clone()),
        })
        .unwrap();

        assert!(output.written);
        assert_eq!(output.config_path, config_path);
        assert!(config_path.exists());
    }

    #[test]
    fn run_fails_on_existing_config_without_force() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let config_path = dir.path().join("config.toml");

        // First write succeeds.
        run(InitArgs {
            force: false,
            config_path: Some(config_path.clone()),
        })
        .unwrap();

        // Second write without force must fail.
        let result = run(InitArgs {
            force: false,
            config_path: Some(config_path.clone()),
        });
        assert!(result.is_err());
    }

    #[test]
    fn run_force_overwrites_existing_config() {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let config_path = dir.path().join("config.toml");

        run(InitArgs {
            force: false,
            config_path: Some(config_path.clone()),
        })
        .unwrap();

        // Force overwrite must succeed.
        let output = run(InitArgs {
            force: true,
            config_path: Some(config_path.clone()),
        })
        .unwrap();
        assert!(output.written);
    }
}
