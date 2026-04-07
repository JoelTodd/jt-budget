use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Command-line interface for the budgeting app.
#[derive(Debug, Parser)]
#[command(name = "jt-budget", version, about = "Terminal budgeting app")]
pub struct Cli {
    /// Run the app against this budget repository.
    #[arg(long, value_name = "PATH")]
    pub repo: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Initialise a budget data repository.
    Init(InitArgs),
    /// Run the TUI against an existing budget repository.
    Run(RunArgs),
    /// Set up a default budget repository for normal launches.
    Setup(SetupArgs),
}

/// Arguments for initialising a budget data repository.
#[derive(Clone, Debug, Args, PartialEq, Eq)]
pub struct InitArgs {
    /// Path to the budget repository to create.
    pub repo: PathBuf,
    /// Remote URL or bare-repo path to publish as `origin`.
    #[arg(long, value_name = "REMOTE")]
    pub remote: Option<String>,
}

/// Arguments for running the TUI against an explicit repository path.
#[derive(Clone, Debug, Args, PartialEq, Eq)]
pub struct RunArgs {
    /// Path to the budget repository to open.
    #[arg(long)]
    pub repo: PathBuf,
}

/// Arguments for first-run repository setup.
#[derive(Clone, Debug, Args, PartialEq, Eq)]
pub struct SetupArgs {
    /// Path to create or adopt as the default budget repository.
    #[arg(long, value_name = "PATH")]
    pub repo: Option<PathBuf>,
    /// Remote URL or bare-repo path to configure as `origin` in advanced mode.
    #[arg(long, value_name = "REMOTE")]
    pub remote: Option<String>,
    /// Create a new synced budget on GitHub.
    #[arg(long, conflicts_with_all = ["github_connect", "local_only", "adopt_local"])]
    pub github_create: bool,
    /// Connect this machine to an existing GitHub-backed budget.
    #[arg(long, conflicts_with_all = ["github_create", "local_only", "adopt_local"])]
    pub github_connect: bool,
    /// Create a local-only budget without configuring a remote.
    #[arg(long, conflicts_with_all = ["github_create", "github_connect", "adopt_local"])]
    pub local_only: bool,
    /// Use an existing local budget folder or the legacy local setup flow.
    #[arg(long, conflicts_with_all = ["github_create", "github_connect", "local_only"])]
    pub adopt_local: bool,
    /// GitHub repository in `OWNER/NAME` form.
    #[arg(long, value_name = "OWNER/NAME")]
    pub github_repo: Option<String>,
    /// Finish setup without opening the TUI afterwards.
    #[arg(long)]
    pub no_open: bool,
}
