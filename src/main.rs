//! advisory-cron CLI entry point.
//!
//! Parses subcommand via clap derive, dispatches to handler in `cli::*`,
//! returns appropriate exit code per ARCHITECTURE.md §CLI surface exit codes.

mod cli;
mod config;
mod heartbeat;
mod launchd;
mod runner;

use clap::Parser;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "advisory-cron",
    version,
    about = "Local cron wrapper for periodic Claude Code tasks (launchd-backed on macOS).",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: cli::Commands,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli::dispatch(cli.command).await {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}
