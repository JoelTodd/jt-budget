use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};

use crate::repository::Repository;

use super::command::{ensure_command_available, run_command, run_command_inherit};

const DEFAULT_GITHUB_HOST: &str = "github.com";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GithubRepoRef {
    pub(super) owner: String,
    pub(super) name: String,
}

impl GithubRepoRef {
    pub(super) fn name_with_owner(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    pub(super) fn https_remote(&self) -> String {
        format!(
            "https://{}/{}/{}.git",
            DEFAULT_GITHUB_HOST, self.owner, self.name
        )
    }

    pub(super) fn matches_remote(&self, remote: &str) -> bool {
        github_remote_name_with_owner(remote)
            .is_some_and(|name_with_owner| name_with_owner == self.name_with_owner())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GithubRepoParts<'a> {
    owner: Option<&'a str>,
    name: &'a str,
}

pub(super) fn default_github_repo_name(local_repo: &Path) -> String {
    let raw = local_repo
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("budget");

    let mut name = String::new();
    let mut last_dash = false;
    for character in raw.chars() {
        let mapped = if character.is_ascii_alphanumeric() {
            character.to_ascii_lowercase()
        } else if matches!(character, '-' | '_' | '.') {
            character
        } else {
            '-'
        };

        if mapped == '-' {
            if name.is_empty() || last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }

        name.push(mapped);
    }

    let trimmed = name.trim_matches(|character| matches!(character, '-' | '_' | '.'));
    if trimmed.is_empty() {
        "budget".to_owned()
    } else {
        trimmed.to_owned()
    }
}

pub(super) fn parse_github_repo_ref(input: &str, default_owner: &str) -> Result<GithubRepoRef> {
    let trimmed = input.trim();
    ensure!(!trimmed.is_empty(), "GitHub repository cannot be blank");

    let parts = if let Some(path) = strip_supported_github_path(trimmed)? {
        parse_repo_parts(path)?
    } else {
        ensure!(
            !looks_like_local_path(trimmed),
            "GitHub repository must be in `OWNER/NAME` form"
        );
        parse_repo_parts(trimmed)?
    };

    let owner = parts.owner.unwrap_or(default_owner);
    ensure!(
        !owner.trim().is_empty(),
        "GitHub repository owner cannot be blank"
    );
    ensure!(
        !parts.name.trim().is_empty(),
        "GitHub repository name cannot be blank"
    );

    Ok(GithubRepoRef {
        owner: owner.to_owned(),
        name: parts.name.to_owned(),
    })
}

pub(super) fn ensure_github_tooling_ready(interactive: bool) -> Result<()> {
    ensure_command_available("git")?;
    ensure_command_available("gh")?;

    if !github_authenticated()? {
        ensure!(
            interactive,
            "GitHub CLI is not authenticated; run `gh auth login` or use interactive setup"
        );
        println!("GitHub CLI is not authenticated. Starting `gh auth login`...");
        run_command_inherit("gh", &["auth", "login"])?;
    }

    run_gh(&["auth", "setup-git", "--hostname", DEFAULT_GITHUB_HOST])
        .context("configuring git to use GitHub CLI credentials")?;
    Ok(())
}

pub(super) fn authenticated_github_login() -> Result<String> {
    let login = run_gh(&["api", "user", "--jq", ".login"]).context("reading GitHub login")?;
    ensure!(!login.trim().is_empty(), "GitHub login cannot be blank");
    Ok(login)
}

pub(super) fn create_github_repository(repo: &GithubRepoRef) -> Result<()> {
    let name_with_owner = repo.name_with_owner();
    match run_gh(&[
        "repo",
        "create",
        &name_with_owner,
        "--private",
        "--disable-issues",
        "--disable-wiki",
    ]) {
        Ok(_) => Ok(()),
        Err(error) if github_repo_already_exists(&error) => {
            verify_github_repository_accessible(repo)
                .with_context(|| format!("verifying existing GitHub repo `{name_with_owner}`"))?;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub(super) fn verify_github_repository_accessible(repo: &GithubRepoRef) -> Result<()> {
    let name_with_owner = repo.name_with_owner();
    run_gh(&["repo", "view", &name_with_owner, "--json", "nameWithOwner"]).map(|_| ())
}

pub(super) fn connect_github_remote_with_retry(
    repo: &Path,
    github_repo: &GithubRepoRef,
) -> Result<()> {
    const MAX_ATTEMPTS: usize = 5;

    let remote = github_remote_for_connection(repo, github_repo)?;
    let mut last_error = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match Repository::connect_remote(repo, &remote) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < MAX_ATTEMPTS && github_repo_not_ready(&error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_secs(1));
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.expect("GitHub connection retry should preserve the last error"))
}

pub(super) fn github_repo_not_ready(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains("Repository not found"))
}

pub(super) fn github_remote_name_with_owner(remote: &str) -> Option<String> {
    let path = strip_supported_github_path(remote).ok().flatten()?;
    let parts = parse_repo_parts(path).ok()?;
    let owner = parts.owner?;
    Some(format!("{owner}/{}", parts.name))
}

fn github_repo_already_exists(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("Name already exists on this account") || text.contains("already exists")
    })
}

fn github_remote_for_connection(repo: &Path, github_repo: &GithubRepoRef) -> Result<String> {
    if let Some(existing_remote) = Repository::origin_remote_url(repo)? {
        if github_repo.matches_remote(&existing_remote) {
            return Ok(existing_remote);
        }
    }
    Ok(github_repo.https_remote())
}

fn parse_repo_parts(path: &str) -> Result<GithubRepoParts<'_>> {
    let parts: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
    match parts.as_slice() {
        [name] => Ok(GithubRepoParts { owner: None, name }),
        [owner, name] => Ok(GithubRepoParts {
            owner: Some(owner),
            name,
        }),
        _ => bail!("GitHub repository must be in `OWNER/NAME` form"),
    }
}

fn strip_supported_github_path(input: &str) -> Result<Option<&str>> {
    if (input.contains("://") || input.starts_with("git@")) && !input.contains(DEFAULT_GITHUB_HOST)
    {
        bail!("only github.com repositories are supported in the GitHub setup flow");
    }

    Ok(input
        .strip_prefix("https://github.com/")
        .or_else(|| input.strip_prefix("http://github.com/"))
        .or_else(|| input.strip_prefix("ssh://git@github.com/"))
        .or_else(|| input.strip_prefix("git@github.com:"))
        .or_else(|| input.strip_prefix("github.com/"))
        .map(|path| path.trim_end_matches('/').trim_end_matches(".git")))
}

fn looks_like_local_path(input: &str) -> bool {
    input.starts_with('/')
        || input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with("~/")
        || input.contains('\\')
}

fn github_authenticated() -> Result<bool> {
    let status = std::process::Command::new("gh")
        .args(["auth", "status", "--hostname", DEFAULT_GITHUB_HOST])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("checking GitHub CLI authentication state")?;
    Ok(status.success())
}

fn run_gh(args: &[&str]) -> Result<String> {
    run_command("gh", args)
}
