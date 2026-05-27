//! Shared default config path resolver — bails loud on $HOME unset.
//! Used by all core::* run functions to honor PROJECT.md hard line #3
//! ("No magic config discovery beyond 2 paths").

use anyhow::{Result, bail};
use std::path::PathBuf;

/// Resolve default config path. Bails! when `$HOME` is unset or empty — never
/// silently falls back to `/`. Per P004 Constraint #16.
pub(crate) fn default_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("$HOME environment variable is not set"))?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home)
        .join(".config")
        .join("advisory-cron")
        .join("config.toml"))
}

/// Resolve $HOME as a PathBuf. Bails! when `$HOME` is unset or empty.
/// Used by core::* fns that need home for non-config purposes
/// (e.g., launch_agents_dir resolution per V2 [O1.3] ACCEPT).
pub(crate) fn home_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("$HOME environment variable is not set"))?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_path_bails_on_missing_home() {
        let saved = std::env::var("HOME").ok();
        // SAFETY: test-only env mutation — reverts after test, no production unsafe.
        // Rust 2024 mandates `unsafe` wrap on `set_var`/`remove_var` per recent stdlib.
        unsafe {
            std::env::remove_var("HOME");
        }
        let result = default_config_path();
        if let Some(h) = saved {
            unsafe {
                std::env::set_var("HOME", h);
            }
        }
        assert!(result.is_err());
    }

    #[test]
    fn default_config_path_returns_expected_subpath() {
        unsafe {
            std::env::set_var("HOME", "/tmp/probe-home");
        }
        let p = default_config_path().unwrap();
        assert!(p.ends_with(".config/advisory-cron/config.toml"));
    }

    #[test]
    fn home_dir_bails_on_missing_home() {
        let saved = std::env::var("HOME").ok();
        unsafe {
            std::env::remove_var("HOME");
        }
        let result = home_dir();
        if let Some(h) = saved {
            unsafe {
                std::env::set_var("HOME", h);
            }
        }
        assert!(result.is_err());
    }
}
