mod command;
mod github;
mod prompts;

use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::cli::SetupArgs;
use crate::locator::RepoLocator;
use crate::repository::Repository;

use self::github::{
    GithubRepoRef, authenticated_github_login, connect_github_remote_with_retry,
    create_github_repository, default_github_repo_name, ensure_github_tooling_ready,
    parse_github_repo_ref, verify_github_repository_accessible,
};
use self::prompts::{
    prompt_for_github_repo, prompt_for_optional_remote, prompt_for_repo_path, prompt_for_setup_mode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupTarget {
    CreateNew,
    AdoptExisting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupMode {
    GithubCreate,
    GithubConnect,
    LocalOnly,
    AdoptLocal,
}

/// Runs the repository setup flow and returns the validated repo path.
///
/// # Errors
///
/// Returns an error when the repository path cannot be determined, setup
/// cannot complete, or the repository gate rejects the final repo state.
pub fn run_setup(locator: &RepoLocator, args: &SetupArgs) -> Result<PathBuf> {
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    match resolve_setup_mode(args, interactive)? {
        SetupMode::GithubCreate => run_github_create_setup(locator, args, interactive),
        SetupMode::GithubConnect => run_github_connect_setup(locator, args, interactive),
        SetupMode::LocalOnly => run_local_only_setup(locator, args, interactive),
        SetupMode::AdoptLocal => run_adopt_local_setup(locator, args, interactive),
    }
}

/// Creates or adopts a repository, validates it through the normal gate, and
/// persists the locator only after that validation succeeds.
///
/// # Errors
///
/// Returns an error when the target path is unsuitable, repository creation or
/// adoption fails, or locator persistence cannot be completed.
pub fn prepare_repository(
    locator: &RepoLocator,
    repo: PathBuf,
    remote: Option<&str>,
) -> Result<PathBuf> {
    match classify_setup_target(&repo)? {
        SetupTarget::CreateNew => {
            Repository::init(&repo, remote)
                .with_context(|| format!("initialising repository `{}`", repo.display()))?;
        }
        SetupTarget::AdoptExisting => {
            if let Some(remote) = remote {
                Repository::connect_remote(&repo, remote).with_context(|| {
                    format!("configuring origin remote for `{}`", repo.display())
                })?;
            } else if let Some(existing_remote) = Repository::origin_remote_url(&repo)? {
                Repository::connect_remote(&repo, &existing_remote).with_context(|| {
                    format!(
                        "repairing origin tracking for existing repository `{}`",
                        repo.display()
                    )
                })?;
            }
        }
    }

    validate_and_save_repository(locator, &repo)
}

fn resolve_setup_mode(args: &SetupArgs, interactive: bool) -> Result<SetupMode> {
    if args.github_repo.is_some() && !args.github_create && !args.github_connect {
        bail!("`--github-repo` requires `--github-create` or `--github-connect`");
    }

    if args.github_create {
        return Ok(SetupMode::GithubCreate);
    }
    if args.github_connect {
        return Ok(SetupMode::GithubConnect);
    }
    if args.local_only {
        return Ok(SetupMode::LocalOnly);
    }
    if args.adopt_local {
        return Ok(SetupMode::AdoptLocal);
    }

    if args.repo.is_some() || args.remote.is_some() {
        return Ok(SetupMode::AdoptLocal);
    }

    if !interactive {
        bail!(
            "setup requires an explicit mode when not running interactively; try `--github-create`, `--github-connect`, `--local-only`, or `--adopt-local`"
        );
    }

    prompt_for_setup_mode()
}

fn run_github_create_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    ensure_github_tooling_ready(interactive)?;

    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;
    let github_login = authenticated_github_login()?;
    let github_repo = resolve_github_create_repo(
        args.github_repo.as_deref(),
        interactive,
        &github_login,
        &repo,
    )?;

    prepare_github_created_repository(locator, repo, &github_repo)
}

fn run_github_connect_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    ensure_github_tooling_ready(interactive)?;

    let github_login = authenticated_github_login()?;
    let github_repo =
        resolve_github_connect_repo(args.github_repo.as_deref(), interactive, &github_login)?;
    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;

    prepare_github_cloned_repository(locator, repo, &github_repo)
}

fn run_local_only_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;
    prepare_repository(locator, repo, None)
}

fn run_adopt_local_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;
    let target = classify_setup_target(&repo)?;
    let remote = resolve_remote(args.remote.as_deref(), interactive, &repo, target)?;
    prepare_repository(locator, repo, remote.as_deref())
}

fn prepare_github_created_repository(
    locator: &RepoLocator,
    repo: PathBuf,
    github_repo: &GithubRepoRef,
) -> Result<PathBuf> {
    match classify_setup_target(&repo)? {
        SetupTarget::CreateNew => {
            Repository::init(&repo, None)
                .with_context(|| format!("initialising repository `{}`", repo.display()))?;
        }
        SetupTarget::AdoptExisting => {
            if let Some(existing_remote) = Repository::origin_remote_url(&repo)? {
                ensure!(
                    github_repo.matches_remote(&existing_remote),
                    "existing budget repo `{}` already has origin remote `{}`",
                    repo.display(),
                    existing_remote
                );
            }
        }
    }

    create_github_repository(github_repo).with_context(|| {
        format!(
            "creating private GitHub repo `{}`",
            github_repo.name_with_owner()
        )
    })?;
    connect_github_remote_with_retry(&repo, github_repo).with_context(|| {
        format!(
            "connecting `{}` to GitHub repo `{}`",
            repo.display(),
            github_repo.name_with_owner()
        )
    })?;

    validate_and_save_repository(locator, &repo)
}

fn prepare_github_cloned_repository(
    locator: &RepoLocator,
    repo: PathBuf,
    github_repo: &GithubRepoRef,
) -> Result<PathBuf> {
    verify_github_repository_accessible(github_repo).with_context(|| {
        format!(
            "checking access to GitHub repo `{}`",
            github_repo.name_with_owner()
        )
    })?;
    Repository::clone_from_remote(&github_repo.https_remote(), &repo).with_context(|| {
        format!(
            "cloning GitHub repo `{}` into `{}`",
            github_repo.name_with_owner(),
            repo.display()
        )
    })?;

    validate_and_save_repository(locator, &repo)
}

fn validate_and_save_repository(locator: &RepoLocator, repo: &Path) -> Result<PathBuf> {
    let validated = Repository::open(repo).with_context(|| {
        format!(
            "validating repository `{}` through the repository gate",
            repo.display()
        )
    })?;
    let canonical_repo = validated.root().to_path_buf();
    drop(validated);

    locator.save(&canonical_repo)?;
    Ok(canonical_repo)
}

fn resolve_repo_path(explicit_repo: Option<&Path>, interactive: bool) -> Result<PathBuf> {
    if let Some(repo) = explicit_repo {
        ensure!(
            !repo.as_os_str().is_empty(),
            "repository path cannot be empty"
        );
        return Ok(repo.to_path_buf());
    }

    if !interactive {
        bail!("setup requires `--repo` when not running interactively");
    }

    prompt_for_repo_path()
}

fn resolve_github_create_repo(
    explicit_repo: Option<&str>,
    interactive: bool,
    default_owner: &str,
    local_repo: &Path,
) -> Result<GithubRepoRef> {
    if let Some(repo) = explicit_repo {
        return parse_github_repo_ref(repo, default_owner);
    }

    let default = GithubRepoRef {
        owner: default_owner.to_owned(),
        name: default_github_repo_name(local_repo),
    };

    if !interactive {
        return Ok(default);
    }

    prompt_for_github_repo("GitHub repository", &default, default_owner)
}

fn resolve_github_connect_repo(
    explicit_repo: Option<&str>,
    interactive: bool,
    default_owner: &str,
) -> Result<GithubRepoRef> {
    if let Some(repo) = explicit_repo {
        return parse_github_repo_ref(repo, default_owner);
    }

    if !interactive {
        bail!("`--github-connect` requires `--github-repo OWNER/NAME`");
    }

    let default = GithubRepoRef {
        owner: default_owner.to_owned(),
        name: "budget".to_owned(),
    };
    prompt_for_github_repo("GitHub repository or URL", &default, default_owner)
}

fn resolve_remote(
    explicit_remote: Option<&str>,
    interactive: bool,
    repo: &Path,
    target: SetupTarget,
) -> Result<Option<String>> {
    if let Some(remote) = explicit_remote {
        let trimmed = remote.trim();
        ensure!(!trimmed.is_empty(), "remote cannot be empty");
        return Ok(Some(trimmed.to_owned()));
    }

    if !interactive {
        return Ok(None);
    }

    if matches!(target, SetupTarget::AdoptExisting) && Repository::has_origin_remote(repo)? {
        return Ok(None);
    }

    prompt_for_optional_remote()
}

fn classify_setup_target(repo: &Path) -> Result<SetupTarget> {
    if !repo.exists() {
        return Ok(SetupTarget::CreateNew);
    }

    ensure!(
        repo.is_dir(),
        "repository path `{}` must be a directory",
        repo.display()
    );

    if fs::read_dir(repo)
        .with_context(|| format!("reading `{}`", repo.display()))?
        .next()
        .transpose()?
        .is_none()
    {
        return Ok(SetupTarget::CreateNew);
    }

    if Repository::looks_like_budget_repo(repo)? {
        return Ok(SetupTarget::AdoptExisting);
    }

    bail!(
        "repository path `{}` must be missing, empty, or an existing jt-budget repository",
        repo.display()
    );
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use anyhow::anyhow;
    use tempfile::tempdir;

    use super::github::{
        GithubRepoRef, default_github_repo_name, github_remote_name_with_owner,
        github_repo_not_ready, parse_github_repo_ref,
    };
    use super::{SetupMode, prepare_repository, resolve_setup_mode};
    use crate::cli::SetupArgs;
    use crate::locator::RepoLocator;
    use crate::repository::Repository;

    #[test]
    fn prepare_repository_creates_new_repo_and_saves_locator() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));

        let canonical_repo = prepare_repository(&locator, repo.clone(), None).unwrap();

        assert_eq!(canonical_repo, repo.canonicalize().unwrap());
        assert_eq!(locator.load().unwrap(), Some(canonical_repo));
        assert!(repo.join("config.toml").exists());
    }

    #[test]
    fn prepare_repository_adopts_existing_repo_and_saves_locator() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        Repository::init(&repo, None).unwrap();

        let canonical_repo = prepare_repository(&locator, repo.clone(), None).unwrap();

        assert_eq!(canonical_repo, repo.canonicalize().unwrap());
        assert_eq!(locator.load().unwrap(), Some(canonical_repo));
    }

    #[test]
    fn prepare_repository_rejects_occupied_non_budget_path() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        fs::create_dir_all(&repo).unwrap();
        fs::write(repo.join("notes.txt"), "not a budget repo").unwrap();

        let error = prepare_repository(&locator, repo, None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must be missing, empty, or an existing jt-budget repository")
        );
    }

    #[test]
    fn parse_github_repo_ref_accepts_owner_name() {
        let repo = parse_github_repo_ref("openai/budget", "fallback").unwrap();
        assert_eq!(
            repo,
            GithubRepoRef {
                owner: "openai".to_owned(),
                name: "budget".to_owned(),
            }
        );
    }

    #[test]
    fn parse_github_repo_ref_accepts_url_and_git_suffix() {
        let repo =
            parse_github_repo_ref("https://github.com/openai/budget.git", "fallback").unwrap();
        assert_eq!(repo.owner, "openai");
        assert_eq!(repo.name, "budget");
    }

    #[test]
    fn parse_github_repo_ref_uses_default_owner_for_name_only() {
        let repo = parse_github_repo_ref("budget", "fallback").unwrap();
        assert_eq!(repo.owner, "fallback");
        assert_eq!(repo.name, "budget");
    }

    #[test]
    fn default_github_repo_name_sanitises_local_path_name() {
        let repo = Path::new("/tmp/My Budget!!!");
        assert_eq!(default_github_repo_name(repo), "my-budget");
    }

    #[test]
    fn github_repo_matches_https_and_ssh_remotes() {
        let repo = GithubRepoRef {
            owner: "JoelTodd".to_owned(),
            name: "budget".to_owned(),
        };

        assert!(repo.matches_remote("https://github.com/JoelTodd/budget.git"));
        assert!(repo.matches_remote("git@github.com:JoelTodd/budget.git"));
        assert!(!repo.matches_remote("https://github.com/JoelTodd/other.git"));
    }

    #[test]
    fn github_remote_name_with_owner_rejects_non_github_paths() {
        assert_eq!(
            github_remote_name_with_owner("https://github.com/JoelTodd/budget.git"),
            Some("JoelTodd/budget".to_owned())
        );
        assert_eq!(github_remote_name_with_owner("/tmp/budget.git"), None);
    }

    #[test]
    fn resolve_setup_mode_defaults_to_legacy_local_flow_when_repo_is_explicit() {
        let mode = resolve_setup_mode(
            &SetupArgs {
                repo: Some("/tmp/budget".into()),
                remote: None,
                github_create: false,
                github_connect: false,
                local_only: false,
                adopt_local: false,
                github_repo: None,
                no_open: false,
            },
            false,
        )
        .unwrap();
        assert_eq!(mode, SetupMode::AdoptLocal);
    }

    #[test]
    fn resolve_setup_mode_rejects_github_repo_without_mode() {
        let error = resolve_setup_mode(
            &SetupArgs {
                repo: None,
                remote: None,
                github_create: false,
                github_connect: false,
                local_only: false,
                adopt_local: false,
                github_repo: Some("owner/name".to_owned()),
                no_open: false,
            },
            false,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("`--github-repo` requires `--github-create` or `--github-connect`")
        );
    }

    #[test]
    fn github_repo_not_ready_checks_wrapped_error_chain() {
        let error = anyhow!("git push -u origin main failed: remote: Repository not found.")
            .context("publishing repository state");

        assert!(github_repo_not_ready(&error));
    }

    #[test]
    fn prepare_repository_repairs_upstream_when_origin_exists() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));

        std::process::Command::new("git")
            .args([
                "init",
                "--bare",
                "--initial-branch=main",
                remote.to_str().unwrap(),
            ])
            .current_dir(temp.path())
            .status()
            .unwrap();
        Repository::init(&repo, None).unwrap();
        std::process::Command::new("git")
            .args(["remote", "add", "origin", remote.to_str().unwrap()])
            .current_dir(&repo)
            .status()
            .unwrap();

        let canonical_repo = prepare_repository(&locator, repo.clone(), None).unwrap();

        assert_eq!(canonical_repo, repo.canonicalize().unwrap());
        let upstream = std::process::Command::new("git")
            .args([
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(upstream.status.success());
        assert_eq!(
            String::from_utf8_lossy(&upstream.stdout).trim(),
            "origin/main"
        );
    }
}
