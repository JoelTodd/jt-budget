use std::fs;
use std::path::Path;

use anyhow::anyhow;
use tempfile::tempdir;

use super::github::{
    GithubRepoRef, default_github_repo_name, github_remote_name_with_owner, github_repo_not_ready,
    parse_github_repo_ref,
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
    let repo = parse_github_repo_ref("https://github.com/openai/budget.git", "fallback").unwrap();
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
        owner: "example-user".to_owned(),
        name: "budget".to_owned(),
    };

    assert!(repo.matches_remote("https://github.com/example-user/budget.git"));
    assert!(repo.matches_remote("git@github.com:example-user/budget.git"));
    assert!(!repo.matches_remote("https://github.com/example-user/other.git"));
}

#[test]
fn github_remote_name_with_owner_rejects_non_github_paths() {
    assert_eq!(
        github_remote_name_with_owner("https://github.com/example-user/budget.git"),
        Some("example-user/budget".to_owned())
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
