use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::cli::SetupArgs;
use crate::locator::RepoLocator;
use crate::repository::Repository;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupTarget {
    CreateNew,
    AdoptExisting,
}

/// Runs the repository setup flow and returns the validated repo path.
///
/// # Errors
///
/// Returns an error when the repository path cannot be determined, setup
/// cannot complete, or the repository gate rejects the final repo state.
pub fn run_setup(locator: &RepoLocator, args: &SetupArgs) -> Result<PathBuf> {
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;
    let target = classify_setup_target(&repo)?;
    let remote = resolve_remote(args.remote.as_deref(), interactive, &repo, target)?;
    prepare_repository(locator, repo, remote.as_deref())
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
            }
        }
    }

    let validated = Repository::open(&repo).with_context(|| {
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

fn prompt_for_repo_path() -> Result<PathBuf> {
    let suggested = default_repo_path();
    let input = prompt_line(&format!(
        "Budget repository path [{}]: ",
        suggested.display()
    ))?;
    if input.trim().is_empty() {
        return Ok(suggested);
    }
    expand_home_path(input.trim())
}

fn prompt_for_optional_remote() -> Result<Option<String>> {
    if !prompt_yes_no("Configure an origin remote now? [y/N]: ", false)? {
        return Ok(None);
    }

    loop {
        let input = prompt_line("Origin remote URL/path: ")?;
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_owned()));
        }
        println!("Remote cannot be blank.");
    }
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    loop {
        let input = prompt_line(prompt)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default);
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please answer y or n."),
        }
    }
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().context("flushing setup prompt")?;

    let mut input = String::new();
    let read = io::stdin()
        .read_line(&mut input)
        .context("reading setup input")?;
    if read == 0 {
        return Err(anyhow!("setup was cancelled before input completed"));
    }
    Ok(input)
}

fn default_repo_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("budget")
}

fn expand_home_path(input: &str) -> Result<PathBuf> {
    if input == "~" {
        return dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"));
    }

    if let Some(rest) = input.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
        return Ok(home.join(rest));
    }

    Ok(PathBuf::from(input))
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

    use tempfile::tempdir;

    use super::prepare_repository;
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
}
