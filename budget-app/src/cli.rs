use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Command-line interface for the budgeting app.
#[derive(Debug, Parser)]
#[command(
    name = "jt-budget",
    version,
    about = "Terminal budgeting app",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialise a budget data repository.
    Init {
        repo: PathBuf,
        #[arg(long)]
        remote: Option<String>,
    },
    /// Run the TUI against an existing budget repository.
    Run {
        #[arg(long)]
        repo: PathBuf,
    },
}
