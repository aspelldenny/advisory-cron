//! MCP server (stdio JSON-RPC 2.0) — exposes 5 tools mirroring CLI subcommands.
//! Thin adapter over crate::core::* — see ARCHITECTURE.md §MCP surface.

pub mod server;
pub mod tools;
