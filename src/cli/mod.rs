//! CLI subcommand definitions and dispatcher.
//!
//! Each subcommand module exposes `pub async fn run(args: <SubArgs>) -> anyhow::Result<u8>`
//! returning the exit code on success. Phase 1.1 stubs return `bail!()` (exit 1).

use clap::Subcommand;

pub mod init;
pub mod mcp;
pub mod register;
pub mod run;
pub mod status;
pub mod unregister;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Write default config to ~/.config/advisory-cron/config.toml.
    Init(init::Args),
    /// Generate launchd plist + register with user session.
    Register(register::Args),
    /// Remove launchd plist + bootout from user session.
    Unregister(unregister::Args),
    /// Fire the configured task once and write heartbeat.
    Run(run::Args),
    /// Show next scheduled fire time + last heartbeat.
    Status(status::Args),
    /// Start MCP server on stdin/stdout (JSON-RPC 2.0, 5 tools).
    Mcp(mcp::Args),
}

pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8> {
    match cmd {
        Commands::Init(args) => init::run(args).await,
        Commands::Register(args) => register::run(args).await,
        Commands::Unregister(args) => unregister::run(args).await,
        Commands::Run(args) => run::run(args).await,
        Commands::Status(args) => status::run(args).await,
        Commands::Mcp(args) => mcp::run(args).await,
    }
}
