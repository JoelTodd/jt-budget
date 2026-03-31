mod cli;
mod logging;
mod repository;
mod state;

pub mod app;
pub mod ui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use logging::init_logging;
use repository::Repository;

pub use repository::{LoadedMonth, Repository as BudgetRepository};
pub use state::{
    CreateDialog, DeleteDialog, EditorState, FailureState, FieldId, GuidedCreationState,
    InteractionState, MoneyInput, MonthEntry, NavigationDialog, NavigationState, PersistenceState,
    RenameDialog, RetryTarget, Route, SectionId, SyncState,
};

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    run_command(cli.command)
}

fn run_command(command: Command) -> Result<()> {
    match command {
        Command::Init { repo, remote } => {
            Repository::init(&repo, remote.as_deref())?;
            init_logging(&repo.join("meta/app.log"));
            Ok(())
        }
        Command::Run { repo } => {
            init_logging(&repo.join("meta/app.log"));
            app::run(repo)
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use tempfile::tempdir;

    use super::{BudgetRepository, Cli, Command, run_command};

    #[test]
    fn init_command_creates_repo_in_missing_directory() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        run_command(Command::Init {
            repo: repo.clone(),
            remote: None,
        })
        .unwrap();

        assert!(repo.join("config.toml").exists());
        assert!(repo.join("months").is_dir());
        assert!(BudgetRepository::open(&repo).is_ok());
    }

    #[test]
    fn cli_requires_subcommand() {
        assert!(Cli::try_parse_from(["jt-budget"]).is_err());
    }

    #[test]
    fn run_subcommand_requires_repo() {
        assert!(Cli::try_parse_from(["jt-budget", "run"]).is_err());
    }
}
