mod cli;
mod locator;
mod logging;
mod repository;
mod setup;
mod state;

pub mod app;
pub mod ui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, InitArgs, RunArgs, SetupArgs};
use locator::RepoLocator;
use logging::init_logging;
use repository::Repository;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

pub use repository::{LoadedMonth, Repository as BudgetRepository};
pub use state::{
    CreateDialogue, DeleteDialogue, EditorState, FailureState, FieldId, GuidedCreationState,
    InteractionState, MoneyInput, MonthEntry, NavigationDialogue, NavigationState,
    PersistenceState, RenameDialogue, RetryTarget, Route, SectionId, SyncState,
};

/// Parses CLI arguments and dispatches the selected command.
pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let locator = RepoLocator::for_current_user()?;
    run_cli_with(cli, &locator)
}

fn run_cli_with(cli: Cli, locator: &RepoLocator) -> Result<()> {
    if cli.repo.is_some() && cli.command.is_some() {
        anyhow::bail!("`--repo` can only be used with the default launch command");
    }

    match cli.command {
        Some(Command::Init(args)) => initialise_repo(args),
        Some(Command::Run(args)) => run_explicit_repo(args),
        Some(Command::Setup(args)) => run_setup_command(locator, &args),
        None => run_default_launch(cli.repo, locator),
    }
}

fn initialise_repo(args: InitArgs) -> Result<()> {
    Repository::init(&args.repo, args.remote.as_deref())?;
    init_logging(&args.repo.join("meta/app.log"));
    Ok(())
}

fn run_explicit_repo(args: RunArgs) -> Result<()> {
    open_app(args.repo)
}

fn run_setup_command(locator: &RepoLocator, args: &SetupArgs) -> Result<()> {
    let repo = setup::run_setup(locator, args)?;
    if args.no_open {
        return Ok(());
    }
    open_app(repo)
}

fn run_default_launch(cli_repo: Option<PathBuf>, locator: &RepoLocator) -> Result<()> {
    match decide_default_launch(cli_repo, env_repo_path()?, locator, terminals_available())? {
        DefaultLaunchAction::Open(repo) => open_app(repo),
        DefaultLaunchAction::RunSetup => {
            let repo = setup::run_setup(
                locator,
                &SetupArgs {
                    repo: None,
                    remote: None,
                    github_create: false,
                    github_connect: false,
                    local_only: false,
                    adopt_local: false,
                    github_repo: None,
                    no_open: false,
                },
            )?;
            open_app(repo)
        }
    }
}

fn open_app(repo: PathBuf) -> Result<()> {
    init_logging(&repo.join("meta/app.log"));
    app::run(repo)
}

fn env_repo_path() -> Result<Option<PathBuf>> {
    match std::env::var_os("JT_BUDGET_REPO") {
        Some(repo) if repo.is_empty() => anyhow::bail!("`JT_BUDGET_REPO` cannot be empty"),
        Some(repo) => Ok(Some(PathBuf::from(repo))),
        None => Ok(None),
    }
}

fn terminals_available() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RepoSource {
    Cli,
    Environment,
    SavedDefault,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LaunchTarget {
    path: PathBuf,
    source: RepoSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DefaultLaunchAction {
    Open(PathBuf),
    RunSetup,
}

fn resolve_launch_target(
    cli_repo: Option<PathBuf>,
    env_repo: Option<PathBuf>,
    locator: &RepoLocator,
) -> Result<Option<LaunchTarget>> {
    if let Some(repo) = cli_repo {
        return Ok(Some(LaunchTarget {
            path: repo,
            source: RepoSource::Cli,
        }));
    }

    if let Some(repo) = env_repo {
        return Ok(Some(LaunchTarget {
            path: repo,
            source: RepoSource::Environment,
        }));
    }

    Ok(locator.load()?.map(|repo| LaunchTarget {
        path: repo,
        source: RepoSource::SavedDefault,
    }))
}

fn decide_default_launch(
    cli_repo: Option<PathBuf>,
    env_repo: Option<PathBuf>,
    locator: &RepoLocator,
    interactive: bool,
) -> Result<DefaultLaunchAction> {
    if let Some(target) = resolve_launch_target(cli_repo, env_repo, locator)? {
        return Ok(DefaultLaunchAction::Open(target.path));
    }

    if interactive {
        return Ok(DefaultLaunchAction::RunSetup);
    }

    anyhow::bail!(
        "no budget repository configured; run `jt-budget setup` or pass `--repo /path/to/repo`"
    );
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use tempfile::tempdir;

    use super::{
        BudgetRepository, Cli, Command, DefaultLaunchAction, InitArgs, RepoLocator, RepoSource,
        RunArgs, SetupArgs, decide_default_launch, resolve_launch_target, run_cli_with,
    };

    #[test]
    fn init_command_creates_repo_in_missing_directory() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        run_cli_with(
            Cli {
                repo: None,
                command: Some(Command::Init(InitArgs {
                    repo: repo.clone(),
                    remote: None,
                })),
            },
            &locator,
        )
        .unwrap();

        assert!(repo.join("config.toml").exists());
        assert!(repo.join("months").is_dir());
        assert!(BudgetRepository::open(&repo).is_ok());
    }

    #[test]
    fn cli_accepts_default_launch_without_subcommand() {
        let cli = Cli::try_parse_from(["jt-budget"]).unwrap();
        assert!(cli.command.is_none());
        assert!(cli.repo.is_none());
    }

    #[test]
    fn run_subcommand_requires_repo() {
        assert!(Cli::try_parse_from(["jt-budget", "run"]).is_err());
    }

    #[test]
    fn cli_parses_default_launch_repo_override() {
        let cli = Cli::try_parse_from(["jt-budget", "--repo", "/tmp/budget"]).unwrap();
        assert_eq!(cli.repo, Some("/tmp/budget".into()));
        assert!(cli.command.is_none());
    }

    #[test]
    fn cli_parses_setup_subcommand() {
        let cli = Cli::try_parse_from(["jt-budget", "setup", "--repo", "/tmp/budget"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Command::Setup(SetupArgs {
                repo: Some("/tmp/budget".into()),
                remote: None,
                github_create: false,
                github_connect: false,
                local_only: false,
                adopt_local: false,
                github_repo: None,
                no_open: false,
            }))
        );
    }

    #[test]
    fn cli_parses_github_connect_setup_subcommand() {
        let cli = Cli::try_parse_from([
            "jt-budget",
            "setup",
            "--github-connect",
            "--github-repo",
            "openai/budget",
            "--repo",
            "/tmp/budget",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Some(Command::Setup(SetupArgs {
                repo: Some("/tmp/budget".into()),
                remote: None,
                github_create: false,
                github_connect: true,
                local_only: false,
                adopt_local: false,
                github_repo: Some("openai/budget".to_owned()),
                no_open: false,
            }))
        );
    }

    #[test]
    fn resolve_launch_target_prefers_cli_repo() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        let saved_repo = temp.path().join("saved");
        std::fs::create_dir_all(&saved_repo).unwrap();
        locator.save(&saved_repo).unwrap();

        let target = resolve_launch_target(
            Some(temp.path().join("cli")),
            Some(temp.path().join("env")),
            &locator,
        )
        .unwrap()
        .unwrap();

        assert_eq!(target.source, RepoSource::Cli);
        assert_eq!(target.path, temp.path().join("cli"));
    }

    #[test]
    fn resolve_launch_target_prefers_env_over_saved_default() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        let saved_repo = temp.path().join("saved");
        std::fs::create_dir_all(&saved_repo).unwrap();
        locator.save(&saved_repo).unwrap();

        let target = resolve_launch_target(None, Some(temp.path().join("env")), &locator)
            .unwrap()
            .unwrap();

        assert_eq!(target.source, RepoSource::Environment);
        assert_eq!(target.path, temp.path().join("env"));
    }

    #[test]
    fn decide_default_launch_requires_repo_when_non_interactive() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));

        let error = decide_default_launch(None, None, &locator, false).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no budget repository configured")
        );
    }

    #[test]
    fn decide_default_launch_runs_setup_when_interactive_and_unconfigured() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));

        let action = decide_default_launch(None, None, &locator, true).unwrap();
        assert_eq!(action, DefaultLaunchAction::RunSetup);
    }

    #[test]
    fn decide_default_launch_uses_saved_default_path() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        let saved_repo = temp.path().join("saved");
        std::fs::create_dir_all(&saved_repo).unwrap();
        locator.save(&saved_repo).unwrap();

        let action = decide_default_launch(None, None, &locator, false).unwrap();
        assert_eq!(
            action,
            DefaultLaunchAction::Open(saved_repo.canonicalize().unwrap())
        );
    }

    #[test]
    fn decide_default_launch_keeps_broken_saved_path_instead_of_falling_back_to_setup() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        let saved_repo = temp.path().join("saved");
        std::fs::create_dir_all(&saved_repo).unwrap();
        locator.save(&saved_repo).unwrap();
        let saved_repo = saved_repo.canonicalize().unwrap();
        std::fs::remove_dir_all(&saved_repo).unwrap();

        let action = decide_default_launch(None, None, &locator, true).unwrap();
        assert_eq!(action, DefaultLaunchAction::Open(saved_repo));
    }

    #[test]
    fn run_cli_rejects_top_level_repo_with_subcommand() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        let error = run_cli_with(
            Cli {
                repo: Some("/tmp/budget".into()),
                command: Some(Command::Run(RunArgs {
                    repo: "/tmp/other".into(),
                })),
            },
            &locator,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("`--repo` can only be used with the default launch command")
        );
    }
}
