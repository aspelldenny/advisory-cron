//! Core business logic modules — CLI and MCP agnostic.
//!
//! Each module exposes a `run()` function that implements the pure business
//! logic for one advisory-cron operation. Both `cli::*` and `mcp::tools`
//! delegate here. Satisfies ARCHITECTURE.md §Layering invariant.
//!
//! V2 design principle: every `core::*::run` resolves its own env dependencies
//! (home dir, launch_agents_dir, self_exe) internally via stdlib calls.
//! The ONLY injected dependency is `&L: LaunchctlClient` (for testability).

pub mod config_path;
pub mod init;
pub mod register;
pub mod run;
pub mod status;
pub mod unregister;
