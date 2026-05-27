//! rmcp server bootstrap — stdio transport.
//!
//! Starts an MCP JSON-RPC 2.0 server on stdin/stdout.
//! Awaits stdin EOF (Claude Desktop closes the pipe on disconnect).

use anyhow::Result;
use rmcp::ServiceExt;
use rmcp::transport::io::stdio;

/// Start the MCP server on stdio. Blocks until the client disconnects or
/// the process receives a signal. Returns `Ok(())` on clean disconnect,
/// `Err(e)` on transport-level failure (caller maps to exit code 5).
pub async fn serve_stdio() -> Result<()> {
    let handler = crate::mcp::tools::AdvisoryCronHandler;
    let service = handler
        .serve(stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP server initialization failed: {e}"))?;
    service
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("MCP server task panicked: {e}"))?;
    Ok(())
}
