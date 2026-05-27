//! `advisory-cron mcp` — thin CLI shell for MCP server subcommand.
//!
//! Starts rmcp server on stdio (JSON-RPC 2.0). Phase 1.7 (P006).
//!
//! V2 (per Architect Turn 1 RESPOND [O1.1] ACCEPT):
//! - Returns `Result<u8>` matching `dispatch()` contract.
//! - Transport errors → `return Ok(5)` (NOT `process::exit(5)`).
//!   Exit code 5 reserved per ARCHITECTURE.md:77 for "MCP transport error".

use anyhow::Result;
use clap::Args as ClapArgs;

#[derive(Debug, ClapArgs)]
pub struct Args {
    // Intentionally empty — stdio MCP has no flags in Phase 1.
}

/// Returns `Result<u8>` matching dispatch contract.
/// Exit codes:
/// - 0: server ran and exited cleanly (stdin EOF).
/// - 5: MCP transport error (stdio closed unexpectedly, malformed JSON-RPC frame).
pub async fn run(_args: Args) -> Result<u8> {
    match crate::mcp::server::serve_stdio().await {
        Ok(()) => Ok(0),
        Err(e) => {
            // Map transport errors to exit code 5 per ARCHITECTURE.md:77.
            // Use Ok(5) not Err(e) — Err would bubble through main and produce a
            // generic exit code 1; we want explicit MCP-transport exit 5.
            eprintln!("MCP transport error: {e:#}");
            Ok(5)
        }
    }
}
